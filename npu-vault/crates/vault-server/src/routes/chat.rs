use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use vault_core::llm::ChatMessage;
use vault_core::search::rrf_fuse;

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

    // 1. Search knowledge base (fulltext)
    let ft_results = {
        let ft_guard = state.fulltext.lock().unwrap();
        match ft_guard.as_ref() {
            Some(ft) => ft.search(&body.message, 5).unwrap_or_default(),
            None => vec![],
        }
    };

    // Vector search (via spawn_blocking to allow nested tokio runtime in OllamaProvider)
    let vec_results = {
        let emb_opt = state.embedding.lock().ok().and_then(|g| g.clone());
        let vec_exists = state
            .vectors
            .lock()
            .ok()
            .map(|g| g.is_some())
            .unwrap_or(false);

        match (emb_opt, vec_exists) {
            (Some(emb), true) => {
                let query_owned = body.message.clone();
                let state_clone = state.clone();
                tokio::task::spawn_blocking(move || {
                    let embeddings = match emb.embed(&[&query_owned]) {
                        Ok(e) if !e.is_empty() => e,
                        _ => return vec![],
                    };
                    let vec_guard = match state_clone.vectors.lock() {
                        Ok(g) => g,
                        Err(_) => return vec![],
                    };
                    match vec_guard.as_ref() {
                        Some(vecs) => vecs
                            .search(&embeddings[0], 5)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(meta, score)| (meta.item_id, score))
                            .collect(),
                        None => vec![],
                    }
                })
                .await
                .unwrap_or_default()
            }
            _ => vec![],
        }
    };

    // RRF fuse
    let fused = rrf_fuse(&vec_results, &ft_results, 0.6, 0.4, 5);

    // Fetch items for context
    let knowledge: Vec<serde_json::Value> = {
        let vault = state.vault.lock().unwrap();
        let mut results = Vec::new();
        for (item_id, score) in &fused {
            if let Ok(Some(item)) = vault.store().get_item(&dek, item_id) {
                results.push(serde_json::json!({
                    "item_id": item.id,
                    "title": item.title,
                    "content": item.content,
                    "score": score,
                    "source_type": item.source_type,
                }));
            }
        }
        results
    };

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
