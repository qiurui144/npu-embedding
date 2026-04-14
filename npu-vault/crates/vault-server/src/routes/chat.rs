use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use vault_core::llm::ChatMessage;

use crate::state::SharedState;

type ApiError = (StatusCode, Json<serde_json::Value>);

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub history: Vec<HistoryMessage>,
}

#[derive(Deserialize, Clone)]
pub struct HistoryMessage {
    pub role: String,
    pub content: String,
}

/// POST /api/v1/chat -- RAG 对话（非流式）
pub async fn chat(
    State(state): State<SharedState>,
    Json(body): Json<ChatRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Check LLM availability
    let llm = state.llm.lock().unwrap().as_ref().cloned();
    let llm = match llm {
        Some(l) => l,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "AI 后端不可用",
                    "hint": "请安装 Ollama 并下载 chat 模型: ollama pull qwen2.5:3b"
                })),
            ))
        }
    };

    let dek = {
        let vault = state.vault.lock().unwrap();
        vault.dek_db().map_err(|e| {
            (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
    };

    // 1. Search knowledge base via three-stage pipeline (initial_k → rerank → top_k)
    let search_params = vault_core::search::SearchParams::with_defaults(5);
    let reranker = state.reranker.lock().map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "reranker lock"})))
    })?.clone();
    let emb = state.embedding.lock().map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "emb lock"})))
    })?.clone();

    let search_results = {
        let ft_guard = state.fulltext.lock().map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "ft lock"})))
        })?;
        let vec_guard = state.vectors.lock().map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vec lock"})))
        })?;
        let vault_guard = state.vault.lock().map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vault lock"})))
        })?;

        let ctx = vault_core::search::SearchContext {
            fulltext: ft_guard.as_ref(),
            vectors: vec_guard.as_ref(),
            embedding: emb,
            reranker,
            store: vault_guard.store(),
            dek: &dek,
        };
        vault_core::search::search_with_context(&ctx, &body.message, &search_params)
            .unwrap_or_default()
    };

    let knowledge: Vec<serde_json::Value> = search_results
        .iter()
        .map(|r| serde_json::json!({
            "item_id": r.item_id,
            "title": r.title,
            "content": r.content,
            "score": r.score,
            "source_type": r.source_type,
        }))
        .collect();

    // 2. Build RAG system prompt
    let mut system_prompt = String::from(
        "你是用户的个人知识助手。以下是从用户本地知识库中检索到的相关文档。\n\
         请基于这些知识回答用户的问题。如果引用了某个文档，请标注 [文档标题]。\n\
         如果知识库中没有相关信息，正常回答即可，不要编造引用。\n\n",
    );

    if !knowledge.is_empty() {
        system_prompt.push_str("=== 知识库相关文档 ===\n\n");
        for (i, k) in knowledge.iter().enumerate() {
            let title = k.get("title").and_then(|v| v.as_str()).unwrap_or("?");
            let content = k.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let score = k.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            system_prompt.push_str(&format!(
                "[{}] 《{}》(相关度: {:.0}%)\n{}\n\n",
                i + 1,
                title,
                score * 100.0,
                content
            ));
        }
        system_prompt.push_str("=== 知识库结束 ===\n");
    }

    // 3. Build messages with history
    let mut messages: Vec<ChatMessage> = vec![ChatMessage::system(&system_prompt)];
    for h in &body.history {
        messages.push(ChatMessage {
            role: h.role.clone(),
            content: h.content.clone(),
        });
    }
    messages.push(ChatMessage::user(&body.message));

    // 4. Call LLM (blocking via spawn_blocking)
    let response = tokio::task::spawn_blocking(move || llm.chat_with_history(&messages))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;

    // 5. Auto-save conversation
    {
        let vault = state.vault.lock().unwrap();
        let title: String = body.message.chars().take(50).collect();
        let content = format!("用户: {}\n\n助手: {}", body.message, response);
        let _ = vault
            .store()
            .insert_item(&dek, &title, &content, None, "ai_chat", None, None);
    }

    // 6. Build citations
    let citations: Vec<serde_json::Value> = knowledge
        .iter()
        .map(|k| {
            serde_json::json!({
                "item_id": k.get("item_id"),
                "title": k.get("title"),
                "relevance": k.get("score"),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "content": response,
        "citations": citations,
        "knowledge_count": knowledge.len(),
    })))
}

/// GET /api/v1/chat/history -- 对话历史（source_type=ai_chat 的 items）
pub async fn chat_history(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let vault = state.vault.lock().unwrap();
    let _ = vault.dek_db().map_err(|e| {
        (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    let items = vault.store().list_items(50, 0).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    let chat_items: Vec<serde_json::Value> = items
        .iter()
        .filter(|i| i.source_type == "ai_chat")
        .map(|i| {
            serde_json::json!({
                "id": i.id,
                "title": i.title,
                "created_at": i.created_at,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({"conversations": chat_items})))
}
