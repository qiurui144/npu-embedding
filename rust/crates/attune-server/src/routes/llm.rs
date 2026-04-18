//! LLM 运维端点 —— 为 Wizard / Settings 提供的 utility 路由
//!
//! - `POST /api/v1/llm/test`：测试云端 LLM 连接（ping 一次，验证 endpoint + api_key + model）
//! - `POST /api/v1/models/pull`：后台拉 Ollama 模型（异步；进度通过 WebSocket 推送）
//!
//! 见 spec `2026-04-19-frontend-redesign-design.md §6`。

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::state::SharedState;
use attune_core::llm::{ChatMessage, LlmProvider, OpenAiLlmProvider};

/// 同一时间最多 2 个 ollama pull 进程（防资源耗尽，见 CRITICAL 1.2）
static PULL_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
const MAX_CONCURRENT_PULLS: usize = 2;

type ApiError = (StatusCode, Json<serde_json::Value>);

// ── POST /api/v1/llm/test ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LlmTestRequest {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Serialize)]
pub struct LlmTestResponse {
    pub ok: bool,
    pub latency_ms: Option<u64>,
    pub reply: Option<String>,
    pub error: Option<String>,
}

pub async fn test_llm(
    Json(body): Json<LlmTestRequest>,
) -> Result<Json<LlmTestResponse>, ApiError> {
    // 输入校验（防 javascript: 注入到"endpoint"）
    let ep = body.endpoint.trim();
    if !(ep.starts_with("http://") || ep.starts_with("https://")) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "endpoint must start with http(s)://"})),
        ));
    }
    if body.model.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "model required"})),
        ));
    }

    let provider = OpenAiLlmProvider::new(ep, &body.api_key, body.model.trim());
    let messages = vec![ChatMessage::user("ping")];

    let start = std::time::Instant::now();

    // 阻塞 LLM 调用通过 spawn_blocking 跑
    let result = tokio::task::spawn_blocking(move || provider.chat_with_history(&messages))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("task join: {e}")})),
            )
        })?;

    let latency_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(reply) => Ok(Json(LlmTestResponse {
            ok: true,
            latency_ms: Some(latency_ms),
            reply: Some(reply.chars().take(100).collect()),
            error: None,
        })),
        Err(e) => Ok(Json(LlmTestResponse {
            ok: false,
            latency_ms: Some(latency_ms),
            reply: None,
            error: Some(e.to_string()),
        })),
    }
}

// ── POST /api/v1/models/pull ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ModelPullRequest {
    pub model: String,
}

#[derive(Serialize)]
pub struct ModelPullResponse {
    pub task_id: String,
    pub status: String,
}

pub async fn pull_model(
    State(_state): State<SharedState>,
    Json(body): Json<ModelPullRequest>,
) -> Result<Json<ModelPullResponse>, ApiError> {
    let model = body.model.trim().to_string();
    if model.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "model required"})),
        ));
    }
    // 基本校验防止 shell 注入（只允许常见 ollama 模型名字符）
    if !model
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || ":-.".contains(c))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid model name"})),
        ));
    }

    // 并发上限守卫（Critical 1.2 修复）
    let inflight = PULL_IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
    if inflight >= MAX_CONCURRENT_PULLS {
        PULL_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": format!("too many concurrent pulls (max {MAX_CONCURRENT_PULLS})"),
            })),
        ));
    }

    let task_id = format!("pull-{}", uuid::Uuid::new_v4());
    let task_id_ret = task_id.clone();

    // 后台跑 `ollama pull <model>`（不等待；进度推送由 WS 侧实现）
    tokio::spawn(async move {
        let out = tokio::process::Command::new("ollama")
            .arg("pull")
            .arg(&model)
            .output()
            .await;
        match out {
            Ok(o) if o.status.success() => {
                tracing::info!("model pull done: {model} (task={task_id})");
            }
            Ok(o) => {
                tracing::warn!(
                    "model pull failed: {model} (task={task_id}) status={} stderr={}",
                    o.status,
                    String::from_utf8_lossy(&o.stderr)
                );
            }
            Err(e) => {
                tracing::warn!("model pull spawn error: {model} (task={task_id}) err={e}");
            }
        }
        // 无论成功失败都释放计数
        PULL_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    });

    Ok(Json(ModelPullResponse {
        task_id: task_id_ret,
        status: "queued".to_string(),
    }))
}
