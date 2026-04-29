//! v0.6 Phase A.5.5 — Privacy tier 检测
//!
//! 端点：
//! - `GET /api/v1/privacy/tier` — 返硬件支持的脱敏层 + 推荐选择
//!
//! 决策（用户 2026-04-28）：
//! - L1 正则脱敏 → OSS 免费层，所有 tier 都有
//! - L2 ONNX NER → OSS 免费层，Tier T1+ 可选下载
//! - L3 LLM 脱敏 → 仅 Tier T3 + T4 + K3 一体机解锁
//!
//! UI 用途：Settings → Privacy 页面根据该 endpoint 渲染 toggle 状态 + 升级提示。

use attune_core::platform::{classify_hardware, Tier};
use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::state::SharedState;

/// 返当前硬件可用的脱敏层 + 推荐 LLM 脱敏模型（如适用）。
pub async fn tier(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let hw = &state.hardware;
    let tier = classify_hardware(hw);

    // 各 tier 解锁的层级
    // L1 正则 = 所有 tier（即使 T0 也提供，但 T0 通常进不了应用）
    // L2 NER = T1+（约 300MB 模型）
    // L3 LLM = T3+ 才有意义
    let layers: Vec<&str> = match tier {
        Tier::Unsupported => vec!["L0", "L1"],
        Tier::Low => vec!["L0", "L1"],
        Tier::Mid => vec!["L0", "L1", "L2"],
        Tier::High => vec!["L0", "L1", "L2", "L3"],
        Tier::Flagship => vec!["L0", "L1", "L2", "L3"],
    };

    // L3 默认模型（按 tier）
    let l3_model: Option<&'static str> = match tier {
        Tier::High => Some("qwen2.5:3b-instruct-q4_K_M"),
        Tier::Flagship => Some("qwen2.5:7b-instruct-q4_K_M"),
        _ => None,
    };

    // 升级提示
    let upgrade_hint: Option<&'static str> = match tier {
        Tier::Unsupported | Tier::Low => Some(
            "你的硬件仅支持 L1 正则脱敏（OSS 免费）。如需 L2 NER / L3 LLM 脱敏，建议升级硬件或选购 K3 一体机。",
        ),
        Tier::Mid => Some(
            "你的硬件支持 L1 + L2 NER 脱敏（OSS 免费）。如需 L3 LLM 语义脱敏，建议升级到 16GB+ RAM / 高性能 CPU。",
        ),
        Tier::High | Tier::Flagship => None, // 已是最高，无升级提示
    };

    let l3_available = matches!(tier, Tier::High | Tier::Flagship);

    Json(json!({
        "hardware_tier": tier.label(),
        "available_layers": layers,
        "l1_regex_available": true,           // 所有 tier 必有
        "l2_ner_available": tier as u8 >= Tier::Mid as u8,
        "l3_llm_available": l3_available,
        "l3_model_suggestion": l3_model,
        "upgrade_hint": upgrade_hint,
        // 默认推荐：L1 已开（强制），L2 / L3 由用户在 Settings 主动切
        "default_active_layers": ["L1"],
    }))
}
