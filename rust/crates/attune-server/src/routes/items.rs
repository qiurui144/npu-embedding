use attune_core::store::audit::PrivacyTier;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize { 20 }

pub async fn list_items(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vault lock poisoned"}))))?;
    let _ = vault.dek_db().map_err(|e| {
        (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    let limit = params.limit.min(200);
    let items = vault.store().list_items(limit, params.offset).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    Ok(Json(serde_json::json!({"items": items, "count": items.len()})))
}

pub async fn get_item(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vault lock poisoned"}))))?;
    let dek = vault.dek_db().map_err(|e| {
        (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    match vault.store().get_item(&dek, &id) {
        Ok(Some(item)) => Ok(Json(serde_json::json!(item))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))),
    }
}

#[derive(Deserialize)]
pub struct UpdateRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}

pub async fn update_item(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vault lock poisoned"}))))?;
    let dek = vault.dek_db().map_err(|e| {
        (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
    })?;

    match vault.store().update_item(&dek, &id, body.title.as_deref(), body.content.as_deref()) {
        Ok(true) => Ok(Json(serde_json::json!({"status": "ok"}))),
        Ok(false) => Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))),
    }
}

pub async fn delete_item(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vault lock poisoned"}))))?;
    match vault.store().delete_item(&id) {
        Ok(true) => Ok(Json(serde_json::json!({"status": "ok"}))),
        Ok(false) => Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))),
    }
}

#[derive(serde::Deserialize)]
pub struct StaleQuery {
    #[serde(default = "default_stale_days")]
    pub days: i64,
    #[serde(default = "default_stale_limit")]
    pub limit: i64,
}

fn default_stale_days() -> i64 { 30 }
fn default_stale_limit() -> i64 { 50 }

pub async fn list_stale_items(
    State(state): State<SharedState>,
    Query(params): Query<StaleQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vault lock poisoned"}))))?;
    let _ = vault.dek_db().map_err(|e| {
        (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    let limit = params.limit.min(200);
    let items = vault.store().list_stale_items(params.days, limit).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    let count = items.len();
    Ok(Json(serde_json::json!({"items": items, "count": count, "days": params.days})))
}

pub async fn get_item_stats(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vault lock poisoned"}))))?;
    let _ = vault.dek_db().map_err(|e| {
        (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    match vault.store().get_item_stats(&id) {
        Ok(Some(stats)) => Ok(Json(serde_json::json!(stats))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))),
    }
}

// ============================================================
// v0.6 Phase A.5.4 — per-file 隐私分级
// ============================================================

#[derive(Deserialize)]
pub struct PrivacyTierBody {
    /// "L0" (🔒 永不出网) | "L1" (脱敏→云，默认) | "L3" (LLM 脱敏→云)
    pub tier: String,
}

fn parse_tier(s: &str) -> Result<PrivacyTier, (StatusCode, Json<serde_json::Value>)> {
    match s.to_uppercase().as_str() {
        "L0" => Ok(PrivacyTier::L0),
        "L1" => Ok(PrivacyTier::L1),
        "L3" => Ok(PrivacyTier::L3),
        other => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("invalid tier '{other}'; expected L0|L1|L3")
            })),
        )),
    }
}

fn tier_str(t: PrivacyTier) -> &'static str {
    match t {
        PrivacyTier::L0 => "L0",
        PrivacyTier::L1 => "L1",
        PrivacyTier::L3 => "L3",
    }
}

/// PATCH /api/v1/items/{id}/privacy_tier  body: { "tier": "L0"|"L1"|"L3" }
pub async fn set_item_privacy(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(body): Json<PrivacyTierBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let tier = parse_tier(&body.tier)?;
    let vault = state.vault.lock().map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vault lock poisoned"})))
    })?;
    let _ = vault.dek_db().map_err(|e| {
        (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    vault.store().set_item_privacy_tier(&id, tier).map_err(|e| {
        let code = if e.to_string().contains("not found") {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        (code, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    Ok(Json(serde_json::json!({"id": id, "privacy_tier": tier_str(tier)})))
}

/// GET /api/v1/items/{id}/privacy_tier
pub async fn get_item_privacy(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock().map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vault lock poisoned"})))
    })?;
    let _ = vault.dek_db().map_err(|e| {
        (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    let tier = vault.store().get_item_privacy_tier(&id).map_err(|e| {
        let code = if e.to_string().contains("not found") {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        (code, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    Ok(Json(serde_json::json!({"id": id, "privacy_tier": tier_str(tier)})))
}

/// GET /api/v1/items/protected — 列出所有标记为 L0 的 item id（Settings UI "受保护文件"）
pub async fn list_protected_items(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock().map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "vault lock poisoned"})))
    })?;
    let _ = vault.dek_db().map_err(|e| {
        (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    let ids = vault.store().list_l0_item_ids().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
    })?;
    let count = ids.len();
    Ok(Json(serde_json::json!({"items": ids, "count": count})))
}
