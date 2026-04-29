# Sprint 1 Phase B: REST API + AI 推荐归类 + WebSocket 推送

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Phase A 已建的 Project / entities 基础上，落 spec §2.3 的"AI 推荐归类"机制：用户上传文件或在 chat 提到关键词时，attune 自动算出"是否归到某个已有 Project"建议，通过 WebSocket 推送到前端供用户确认。

**Architecture:**
- 实体重叠度（Jaccard 相似度）由 `entity_overlap_score()` 计算（attune-core 扩展 `entities.rs`）
- `ProjectRecommender`（attune-core 新增）封装两种触发：`recommend_for_file` / `recommend_for_chat`
- REST API `/api/v1/projects/*`（attune-server `routes/projects.rs` 新增）暴露 CRUD
- 触发点：(1) `routes/upload.rs` 文件上传成功后 spawn 异步 task 跑 recommender；(2) `routes/chat.rs` 用户消息进入时检测关键词
- 推送通道：扩展现有 `ws/scan-progress` 的 JSON payload 加 `recommendations` 字段（最小动作，复用前端已有连接）

**Tech Stack:**
- axum 0.8 routes
- tokio broadcast channel（推荐推送）
- 共享 `Arc<Store>` 通过 `state.vault.lock().store()` 访问
- 前端 vanilla JS（Phase B 只 console.log 验证；完整 UI 留 Phase D）

**Spec source:** [`docs/superpowers/specs/2026-04-25-industry-attune-design.md`](../specs/2026-04-25-industry-attune-design.md) §2.3

---

## File Structure

**Create:**
- `rust/crates/attune-core/src/project_recommender.rs` — 推荐引擎
- `rust/crates/attune-server/src/routes/projects.rs` — Project REST API
- `rust/crates/attune-core/tests/project_recommender_test.rs` — 集成测试

**Modify:**
- `rust/crates/attune-core/src/entities.rs` — 扩展 `entity_overlap_score()` + `EntitySet` 类型
- `rust/crates/attune-core/src/lib.rs` — `pub mod project_recommender;`
- `rust/crates/attune-server/src/state.rs` — `AppState` 加 `recommendation_tx: broadcast::Sender<...>`
- `rust/crates/attune-server/src/routes/mod.rs` — 加 `pub mod projects;`
- `rust/crates/attune-server/src/lib.rs` — `build_router` 加 `/api/v1/projects/*` 路由 + 注入 broadcast channel 到 AppState
- `rust/crates/attune-server/src/routes/upload.rs` — 文件上传成功后调 recommender + send 到 channel
- `rust/crates/attune-server/src/routes/chat.rs` — 用户消息进入时调 recommend_for_chat
- `rust/crates/attune-server/src/routes/ws.rs` — payload 加 `recommendations` 字段
- `rust/crates/attune-core/src/store/middleware.rs` — vault_guard / bearer_auth_guard 不需要白名单 `/api/v1/projects/*`（属业务 endpoint，要 vault 已解锁 + auth）

---

## Progress Tracking

每 Task 完成后回到本文件勾 checkbox。每 Task 一个独立 commit。中间确保 `cargo test --workspace` 维持 ≥ 392 passed。

---

### Task 1: entity_overlap_score（Jaccard 相似度）

在 `entities.rs` 扩展，加 `EntitySet` 类型 + `entity_overlap_score(a, b) -> f32`。

**Files:**
- Modify: `rust/crates/attune-core/src/entities.rs`

- [ ] **Step 1: 写失败测试 — 在 entities.rs 内联 mod tests 末尾加**

打开 `rust/crates/attune-core/src/entities.rs`，找到 `#[cfg(test)] mod unit_tests { ... }` 块（Phase A 加的），在末尾追加：

```rust
    #[test]
    fn overlap_score_identical() {
        let a = extract_entities("张三借款 ¥10000，2024-03-15 到期");
        let b = extract_entities("张三借款 ¥10000，2024-03-15 到期");
        let s = entity_overlap_score(&a, &b);
        assert!((s - 1.0).abs() < 1e-6, "完全相同应 1.0，got {s}");
    }

    #[test]
    fn overlap_score_disjoint() {
        let a = extract_entities("张三借款 ¥10000");
        let b = extract_entities("李四签约 ¥50000");
        let s = entity_overlap_score(&a, &b);
        // 张三 vs 李四 / 10000 vs 50000 完全不重叠 → 0
        assert!(s < 0.01, "无重叠应 ~0，got {s}");
    }

    #[test]
    fn overlap_score_partial() {
        let a = extract_entities("张三借款 ¥10000 (2024)京02民终123号");
        let b = extract_entities("张三起诉 ¥20000 (2024)京02民终123号");
        let s = entity_overlap_score(&a, &b);
        // 共享：张三 + (2024)京02民终123号；不共享：10000 / 20000
        // a = {张三, ¥10000, (2024)京02民终123号}
        // b = {张三, ¥20000, (2024)京02民终123号}
        // intersect = 2, union = 4 → 0.5
        assert!((s - 0.5).abs() < 0.05, "应 ~0.5（Jaccard），got {s}");
    }

    #[test]
    fn overlap_score_empty_inputs() {
        assert_eq!(entity_overlap_score(&[], &[]), 0.0);
        let a = extract_entities("张三");
        assert_eq!(entity_overlap_score(&a, &[]), 0.0);
        assert_eq!(entity_overlap_score(&[], &a), 0.0);
    }
```

- [ ] **Step 2: 跑测试，验证 fail**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo test --release -p attune-core entities::unit_tests::overlap 2>&1 | tail -10
```

预期：`unresolved name entity_overlap_score`。

- [ ] **Step 3: 实现 entity_overlap_score**

在 `rust/crates/attune-core/src/entities.rs` 的 `extract_entities` fn 之**后**、`#[cfg(test)] mod unit_tests` 之前，追加：

```rust
/// 计算两组实体的 Jaccard 相似度：|A ∩ B| / |A ∪ B|。
///
/// 用 (kind, value) 二元组作为去重 key — 同字面值不同 kind 视为不同实体。
/// 空输入返回 0.0。Sprint 1 Phase B 用 0.6 阈值判断"是否推荐归类"（spec §2.3）。
pub fn entity_overlap_score(a: &[Entity], b: &[Entity]) -> f32 {
    use std::collections::HashSet;

    if a.is_empty() && b.is_empty() {
        return 0.0;
    }

    let set_a: HashSet<(EntityKind, &str)> =
        a.iter().map(|e| (e.kind, e.value.as_str())).collect();
    let set_b: HashSet<(EntityKind, &str)> =
        b.iter().map(|e| (e.kind, e.value.as_str())).collect();

    let inter = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    if union == 0 {
        0.0
    } else {
        inter as f32 / union as f32
    }
}
```

- [ ] **Step 4: 跑测试验证 pass**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo test --release -p attune-core entities 2>&1 | tail -15
```

预期：所有 entities 测试 pass（unit 8 个 + integration 7 个 = 15 个），含新加的 4 个 overlap 测试。

- [ ] **Step 5: 跑全工作区**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**396 passed**（392 baseline + 4 overlap unit tests = 396）。

- [ ] **Step 6: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add rust/crates/attune-core/src/entities.rs && \
git commit -m "feat(entities): entity_overlap_score (Jaccard similarity)

(kind, value) tuple dedup; empty inputs → 0.0.
Sprint 1 Phase B will use 0.6 threshold for Project recommendation
(spec §2.3).

Tests: 396 passed (392 baseline + 4 overlap)."
```

---

### Task 2: ProjectRecommender 引擎（attune-core）

封装两种触发：`recommend_for_file` / `recommend_for_chat`。

**Files:**
- Create: `rust/crates/attune-core/src/project_recommender.rs`
- Create: `rust/crates/attune-core/tests/project_recommender_test.rs`
- Modify: `rust/crates/attune-core/src/lib.rs`

- [ ] **Step 1: 写失败测试**

`rust/crates/attune-core/tests/project_recommender_test.rs`:

```rust
//! ProjectRecommender 集成测试 — 用 in-memory Store 模拟现实推荐。

use attune_core::project_recommender::{recommend_for_file, recommend_for_chat, RecommendationCandidate, ChatTriggerHint};
use attune_core::store::{ProjectKind, Store};

/// 工具：在 in-memory store 上建一个 project 并放入若干文件实体（mock：文件实体由参数提供）。
fn setup_store_with_project(
    project_title: &str,
    files_with_entities: Vec<(&str, &str)>, // (file_id, file_text — 用于实体抽取)
) -> (Store, String) {
    let store = Store::open_memory().expect("open memory store");
    let p = store
        .create_project(project_title, ProjectKind::Case)
        .expect("create project");
    for (file_id, _file_text) in &files_with_entities {
        store
            .add_file_to_project(&p.id, file_id, "evidence")
            .expect("add file");
        // 注：实际 Store 没存 file_text；recommender 走 "传入新文件 entities + 已有 project 的 entities 集合"
        // 由调用方负责（route handler 会从 items 表 join 出来）。本 test 直接验证 fn 接口。
    }
    (store, p.id)
}

#[test]
fn recommend_for_file_match_high_overlap() {
    // 已有 project 的 entity 集（从 file_id="ev-1" 等 ingest 时算出来；recommender 不重抽）
    let project_entities_per_file: Vec<Vec<attune_core::entities::Entity>> = vec![
        attune_core::entities::extract_entities("张三借款 ¥10000 (2024)京02民终123号"),
    ];
    // 新上传文件
    let new_file_entities =
        attune_core::entities::extract_entities("张三签合同 ¥10000 (2024)京02民终123号 履行");

    let (store, pid) = setup_store_with_project("民间借贷案", vec![("ev-1", "")]);
    // 测试 fn 签名：(store, new_file_entities, given_project_id_to_entities) -> Vec<Candidate>
    // 但实际签名让 recommender 自己 join 实体... 简化：本测试只测纯函数 score_against_project_entities
    let cand = recommend_for_file(
        &store,
        "new-file-1",
        &new_file_entities,
        // 实测时 recommender 会扫所有 project，每个 project 收集 entities；
        // 这里 mock：直接传 project entities map
        Some(vec![(&pid, project_entities_per_file.iter().flatten().cloned().collect())]),
    )
    .expect("recommend");

    assert!(!cand.is_empty(), "高重叠应推荐至少 1 个 project");
    assert_eq!(cand[0].project_id, pid);
    assert!(cand[0].score >= 0.6, "应过 0.6 阈值，got {}", cand[0].score);
}

#[test]
fn recommend_for_file_no_match_low_overlap() {
    let new_entities = attune_core::entities::extract_entities("李四签约 ¥50000");
    let other_entities = attune_core::entities::extract_entities("张三借款 ¥10000");

    let (store, pid) = setup_store_with_project("无关案件", vec![("ev-2", "")]);
    let cand = recommend_for_file(
        &store,
        "new-file-2",
        &new_entities,
        Some(vec![(&pid, other_entities)]),
    )
    .expect("recommend");

    assert!(
        cand.iter().all(|c| c.score < 0.6),
        "无重叠不应推荐过阈值，got {:?}",
        cand
    );
}

#[test]
fn recommend_for_chat_keyword_hit() {
    let hit = recommend_for_chat("我现在的案件，王某 vs 李某，有几个证据要整理。");
    assert!(hit.is_some(), "含'案件'应触发 hint");
    let h = hit.unwrap();
    assert!(h.matched_keywords.contains(&"案件".to_string()));
}

#[test]
fn recommend_for_chat_no_keyword() {
    let hit = recommend_for_chat("今天天气真好啊");
    assert!(hit.is_none(), "无关键词不应触发 hint");
}
```

- [ ] **Step 2: lib.rs 注册新 mod**

`rust/crates/attune-core/src/lib.rs`：在已有 `pub mod entities;` 后加：

```rust
pub mod project_recommender;
```

- [ ] **Step 3: 跑测试验证 fail**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo test --release -p attune-core --test project_recommender_test 2>&1 | tail -10
```

预期：编译失败，`unresolved import attune_core::project_recommender` 或缺类型。

- [ ] **Step 4: 实现 project_recommender.rs**

`rust/crates/attune-core/src/project_recommender.rs`:

```rust
//! Project 推荐归类引擎（spec §2.3）
//!
//! 两种触发：
//! - 文件上传成功 → recommend_for_file 算实体重叠度，> 0.6 推荐
//! - chat 用户消息含触发关键词 → recommend_for_chat 提示用户"是否要找/建 Project"
//!
//! 推荐结果**不持久化**：通过 WebSocket 推送给前端；前端如果错过，下次同样路径再算即可。

use crate::entities::{entity_overlap_score, Entity};
use crate::error::Result;
use crate::store::Store;
use serde::{Deserialize, Serialize};

/// 单条推荐候选（一个 Project 是否值得归到该 Project）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationCandidate {
    pub project_id: String,
    pub project_title: String,
    /// Jaccard 相似度（0.0-1.0）
    pub score: f32,
    /// 触发的实体重叠（最相关的 top 5）
    pub overlapping_entities: Vec<String>,
}

/// chat 关键词触发结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTriggerHint {
    /// 命中的关键词
    pub matched_keywords: Vec<String>,
    /// 提示文案（前端可显示气泡）
    pub suggestion: String,
}

/// spec §2.3 阈值
pub const RECOMMEND_THRESHOLD: f32 = 0.6;

/// 触发关键词（中文常见关于"案件 / 客户 / 项目"语义）。
const CHAT_TRIGGER_KEYWORDS: &[&str] = &["案件", "案号", "客户", "项目", "诉讼", "案子"];

/// 给一份新文件（或新 chunk）算应该归到哪个 Project。
///
/// 参数 `project_entities` 是为了避免 recommender 在调用方代价巨大的 join：
/// route handler 调用前先从 items 表 + project_file 表组装好每个 active project 的
/// entities Vec，然后传入。如为 None，recommender fall back 走简化路径返回空。
///
/// 返回的 candidates 已按 score 降序排列，仅包含 score >= 阈值的项。
pub fn recommend_for_file(
    _store: &Store,
    _new_file_id: &str,
    new_file_entities: &[Entity],
    project_entities: Option<Vec<(&String, Vec<Entity>)>>,
) -> Result<Vec<RecommendationCandidate>> {
    let projects = match project_entities {
        Some(v) => v,
        None => return Ok(Vec::new()),
    };

    let mut out = Vec::new();
    for (pid, ents) in projects {
        let score = entity_overlap_score(new_file_entities, &ents);
        if score >= RECOMMEND_THRESHOLD {
            // 计算重叠的实体（最多 5 个，方便前端显示）
            use std::collections::HashSet;
            let new_set: HashSet<_> = new_file_entities
                .iter()
                .map(|e| (e.kind, e.value.clone()))
                .collect();
            let overlap: Vec<String> = ents
                .iter()
                .filter(|e| new_set.contains(&(e.kind, e.value.clone())))
                .take(5)
                .map(|e| format!("{:?}: {}", e.kind, e.value))
                .collect();

            // project_title 由调用方在 route 层补；这里给空，因为 recommender 不持有 store query 责任
            out.push(RecommendationCandidate {
                project_id: pid.clone(),
                project_title: String::new(),
                score,
                overlapping_entities: overlap,
            });
        }
    }

    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}

/// 给一段 chat 用户消息检测是否含 Project 触发关键词。
///
/// 不调 LLM，纯关键词匹配。命中即返回 ChatTriggerHint。无命中返回 None。
pub fn recommend_for_chat(message: &str) -> Option<ChatTriggerHint> {
    let mut matched = Vec::new();
    for kw in CHAT_TRIGGER_KEYWORDS {
        if message.contains(kw) {
            matched.push(kw.to_string());
        }
    }
    if matched.is_empty() {
        None
    } else {
        Some(ChatTriggerHint {
            matched_keywords: matched.clone(),
            suggestion: format!(
                "看起来你提到了 {} — 是否要把当前对话或最近上传的文件归到一个 Project？",
                matched.join(" / ")
            ),
        })
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::entities::extract_entities;

    #[test]
    fn threshold_constant() {
        assert!((RECOMMEND_THRESHOLD - 0.6).abs() < 1e-6);
    }

    #[test]
    fn chat_keyword_basic() {
        let h = recommend_for_chat("帮我整理这个案件的证据").expect("hit");
        assert!(h.matched_keywords.contains(&"案件".to_string()));
    }

    #[test]
    fn chat_keyword_multiple() {
        let h = recommend_for_chat("这个客户的项目我们整理一下").expect("hit");
        assert!(h.matched_keywords.contains(&"客户".to_string()));
        assert!(h.matched_keywords.contains(&"项目".to_string()));
    }

    #[test]
    fn chat_no_keyword() {
        assert!(recommend_for_chat("今天天气怎样").is_none());
    }

    #[test]
    fn recommend_for_file_empty_projects() {
        let store = Store::open_memory().expect("open");
        let new_ents = extract_entities("test");
        let r = recommend_for_file(&store, "f1", &new_ents, None).expect("ok");
        assert!(r.is_empty());

        let r = recommend_for_file(&store, "f1", &new_ents, Some(vec![])).expect("ok");
        assert!(r.is_empty());
    }
}
```

- [ ] **Step 5: 跑测试验证 pass**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo test --release -p attune-core project_recommender 2>&1 | tail -15
```

预期：8 个测试 pass（unit_tests 4 个 + integration 4 个 = 8 个）。

如果 fail：检查 RecommendationCandidate / ChatTriggerHint 字段名、recommend_for_file 签名是否与 test 一致（test 用了 `Some(vec![(&pid, ...)])`，fn 签名要 `Option<Vec<(&String, Vec<Entity>)>>`）。

- [ ] **Step 6: 跑全工作区**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**404 passed**（396 baseline + 8 = 404）。

- [ ] **Step 7: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add rust/crates/attune-core/src/project_recommender.rs \
        rust/crates/attune-core/tests/project_recommender_test.rs \
        rust/crates/attune-core/src/lib.rs && \
git commit -m "feat(recommender): ProjectRecommender — recommend_for_file / recommend_for_chat

Spec §2.3 — 0.6 Jaccard threshold; chat keyword triggers (案件/客户/项目/...).
Pure functions, no I/O — caller assembles project entities + decides UI.
Tests: 404 passed (396 baseline + 8 recommender)."
```

---

### Task 3: routes/projects.rs — Project REST API

REST 端点：CRUD project + 文件归属 + timeline。

**Files:**
- Create: `rust/crates/attune-server/src/routes/projects.rs`
- Modify: `rust/crates/attune-server/src/routes/mod.rs`
- Modify: `rust/crates/attune-server/src/lib.rs`

- [ ] **Step 1: routes/mod.rs 注册新 mod**

读 `rust/crates/attune-server/src/routes/mod.rs`，在末尾追加（按字母序合适位置）：

```rust
pub mod projects;
```

- [ ] **Step 2: 创建 projects.rs**

`rust/crates/attune-server/src/routes/projects.rs`:

```rust
//! Project / Case 卷宗 REST API（spec §2.3）

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use attune_core::store::{Project, ProjectFile, ProjectKind, ProjectTimelineEntry};
use attune_core::vault::VaultState;
use serde::{Deserialize, Serialize};

use crate::state::SharedState;

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub title: String,
    #[serde(default)]
    pub kind: Option<String>, // 'case' / 'deal' / 'topic' / 'generic'
}

#[derive(Debug, Deserialize)]
pub struct AddFileRequest {
    pub file_id: String,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectListResponse {
    pub projects: Vec<Project>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct FilesListResponse {
    pub files: Vec<ProjectFile>,
}

#[derive(Debug, Serialize)]
pub struct TimelineResponse {
    pub entries: Vec<ProjectTimelineEntry>,
}

/// POST /api/v1/projects
pub async fn create_project(
    State(state): State<SharedState>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<Project>), (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(vault.state(), VaultState::Unlocked) {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "vault locked"}))));
    }
    let kind = match req.kind.as_deref() {
        Some(s) => ProjectKind::from_str(s),
        None => ProjectKind::Generic,
    };
    let title = req.title.trim();
    if title.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "title required"})),
        ));
    }
    let p = vault
        .store()
        .create_project(title, kind)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;
    Ok((StatusCode::CREATED, Json(p)))
}

/// GET /api/v1/projects?include_archived=false
pub async fn list_projects(
    State(state): State<SharedState>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ProjectListResponse>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(vault.state(), VaultState::Unlocked) {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "vault locked"}))));
    }
    let include_archived = q
        .get("include_archived")
        .map(|s| s == "true" || s == "1")
        .unwrap_or(false);
    let projects = vault
        .store()
        .list_projects(include_archived)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;
    let total = projects.len();
    Ok(Json(ProjectListResponse { projects, total }))
}

/// GET /api/v1/projects/:id
pub async fn get_project(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Project>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(vault.state(), VaultState::Unlocked) {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "vault locked"}))));
    }
    let p = vault
        .store()
        .get_project(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;
    match p {
        Some(p) => Ok(Json(p)),
        None => Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "project not found"})))),
    }
}

/// POST /api/v1/projects/:id/files
pub async fn add_file_to_project(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<AddFileRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(vault.state(), VaultState::Unlocked) {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "vault locked"}))));
    }
    if vault.store().get_project(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?.is_none() {
        return Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "project not found"}))));
    }
    let role = req.role.as_deref().unwrap_or("");
    vault
        .store()
        .add_file_to_project(&id, &req.file_id, role)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({"status": "ok"}))))
}

/// GET /api/v1/projects/:id/files
pub async fn list_project_files(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<FilesListResponse>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(vault.state(), VaultState::Unlocked) {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "vault locked"}))));
    }
    let files = vault
        .store()
        .list_files_for_project(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;
    Ok(Json(FilesListResponse { files }))
}

/// GET /api/v1/projects/:id/timeline
pub async fn list_project_timeline(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<TimelineResponse>, (StatusCode, Json<serde_json::Value>)> {
    let vault = state.vault.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(vault.state(), VaultState::Unlocked) {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "vault locked"}))));
    }
    let entries = vault
        .store()
        .list_timeline(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;
    Ok(Json(TimelineResponse { entries }))
}
```

- [ ] **Step 3: lib.rs build_router 加路由**

打开 `rust/crates/attune-server/src/lib.rs`，找到现有 `.route("/api/v1/items", ...)` 之类（约 L77）。在合适位置追加：

```rust
        // Projects (Sprint 1 Phase B)
        .route("/api/v1/projects",
            get(routes::projects::list_projects)
            .post(routes::projects::create_project))
        .route("/api/v1/projects/{id}", get(routes::projects::get_project))
        .route("/api/v1/projects/{id}/files",
            get(routes::projects::list_project_files)
            .post(routes::projects::add_file_to_project))
        .route("/api/v1/projects/{id}/timeline", get(routes::projects::list_project_timeline))
```

按 axum 0.8 的语法 — `{id}` 不是 `:id`（attune 项目已用 `{id}` 风格，看 chat_sessions 路由）。

- [ ] **Step 4: 写 integration test**

`rust/crates/attune-server/tests/projects_routes_test.rs`:

```rust
//! Project REST API integration test。

use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn projects_crud_e2e() {
    // 自由 port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let config = attune_server::ServerConfig {
        host: "127.0.0.1".to_string(),
        port,
        tls_cert: None,
        tls_key: None,
        no_auth: true, // 测试模式
    };
    let handle = tokio::spawn(async move { attune_server::run_in_runtime(config).await });
    tokio::time::sleep(Duration::from_millis(200)).await;

    let base = format!("http://127.0.0.1:{}/api/v1/projects", port);

    // 1. unlock vault first
    let unlock_url = format!("http://127.0.0.1:{}/api/v1/vault/setup", port);
    let _ = reqwest::Client::new()
        .post(&unlock_url)
        .json(&serde_json::json!({"password": "test123"}))
        .send()
        .await;
    let unlock_url = format!("http://127.0.0.1:{}/api/v1/vault/unlock", port);
    let _ = reqwest::Client::new()
        .post(&unlock_url)
        .json(&serde_json::json!({"password": "test123"}))
        .send()
        .await;

    // 2. create project
    let resp = reqwest::Client::new()
        .post(&base)
        .json(&serde_json::json!({"title": "案件 A", "kind": "case"}))
        .send()
        .await
        .expect("create");
    assert_eq!(resp.status().as_u16(), 201);
    let project: serde_json::Value = resp.json().await.expect("json");
    let pid = project["id"].as_str().expect("id").to_string();

    // 3. list
    let resp = reqwest::Client::new().get(&base).send().await.expect("list");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("body");
    assert!(body["total"].as_u64().unwrap() >= 1);

    // 4. get by id
    let resp = reqwest::Client::new()
        .get(&format!("{}/{}", base, pid))
        .send()
        .await
        .expect("get");
    assert_eq!(resp.status(), 200);

    // 5. add file
    let resp = reqwest::Client::new()
        .post(&format!("{}/{}/files", base, pid))
        .json(&serde_json::json!({"file_id": "file-001", "role": "evidence"}))
        .send()
        .await
        .expect("add file");
    assert_eq!(resp.status().as_u16(), 201);

    // 6. list files
    let resp = reqwest::Client::new()
        .get(&format!("{}/{}/files", base, pid))
        .send()
        .await
        .expect("list files");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("body");
    assert_eq!(body["files"].as_array().unwrap().len(), 1);

    handle.abort();
}
```

- [ ] **Step 5: 跑测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo test --release -p attune-server --test projects_routes_test 2>&1 | tail -15
```

预期：1 测试 pass。

如 fail：常见原因是 vault unlock 流程（test 模式可能不需要 unlock，no_auth=true 时部分 endpoint 仍要求 vault 解锁）。如果 unlock 测试代码写错，调整或简化为直接 setup 一个测试 vault。

- [ ] **Step 6: 跑全工作区**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
timeout 300 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**405 passed**（404 baseline + 1 integration = 405）。

- [ ] **Step 7: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add rust/crates/attune-server/src/routes/projects.rs \
        rust/crates/attune-server/src/routes/mod.rs \
        rust/crates/attune-server/src/lib.rs \
        rust/crates/attune-server/tests/projects_routes_test.rs && \
git commit -m "feat(api): /api/v1/projects/* CRUD + files + timeline

axum routes wrap Phase A Store::project methods.
Vault locked → 403; missing project → 404; bad input → 400.
Integration test: vault setup → unlock → create → list → get → add_file → list_files.
Tests: 405 passed."
```

---

### Task 4: file_added → 自动跑 recommender + WebSocket 推送

文件上传完成后（routes/upload.rs），spawn 异步任务跑 recommender，结果通过 broadcast channel 推到 ws.rs。

**Files:**
- Modify: `rust/crates/attune-server/src/state.rs`
- Modify: `rust/crates/attune-server/src/routes/upload.rs`
- Modify: `rust/crates/attune-server/src/routes/ws.rs`
- Modify: `rust/crates/attune-server/src/lib.rs`

- [ ] **Step 1: state.rs 加 recommendation broadcast channel**

打开 `rust/crates/attune-server/src/state.rs`，在 `pub struct AppState` 字段末尾追加（在 `pub queue_worker_running` 等之后）：

```rust
    /// Sprint 1 Phase B: project recommendation broadcast channel.
    /// upload.rs / chat.rs 收到信号后 send；ws.rs subscribe 推送给前端。
    pub recommendation_tx: tokio::sync::broadcast::Sender<serde_json::Value>,
```

`AppState::new` 构造时初始化（找到 `pub fn new(...)`）：

```rust
let (recommendation_tx, _rx) = tokio::sync::broadcast::channel(64);
```

并把它加到 Self 里。

- [ ] **Step 2: upload.rs 在文件上传完成后调 recommender**

`rust/crates/attune-server/src/routes/upload.rs` 末尾，在 `Ok(Json(serde_json::json!(...)))` **之前**追加：

```rust
    // Sprint 1 Phase B: 异步跑 ProjectRecommender，命中阈值通过 ws 推送
    let title_clone = title.clone();
    let item_id_clone = item_id.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        let vault_guard = state_clone.vault.lock();
        let vault_guard = vault_guard.unwrap_or_else(|e| e.into_inner());
        if !matches!(vault_guard.state(), attune_core::vault::VaultState::Unlocked) {
            return;
        }
        // 抽 entities — 这里简化：用 title 当样本（chunks 的 entities 可在 Phase D 优化）
        let new_ents = attune_core::entities::extract_entities(&title_clone);
        if new_ents.is_empty() {
            return;
        }
        // 收集所有 active project 的 entities
        let projects = match vault_guard.store().list_projects(false) {
            Ok(v) => v,
            Err(_) => return,
        };
        let mut project_entities: Vec<(&String, Vec<attune_core::entities::Entity>)> = Vec::new();
        // 注：当前简化，只算 project title 的 entities（避免 join file → text 的 query 复杂度）；
        // Phase B 后续可优化为聚合 file 内容。
        let project_titles: Vec<(String, String)> = projects
            .iter()
            .map(|p| (p.id.clone(), p.title.clone()))
            .collect();
        let project_ents_storage: Vec<(String, Vec<attune_core::entities::Entity>)> = project_titles
            .iter()
            .map(|(id, title)| (id.clone(), attune_core::entities::extract_entities(title)))
            .collect();
        // 借用 ref（避免 lifetime 冲突）
        for (id, ents) in &project_ents_storage {
            project_entities.push((id, ents.clone()));
        }
        let candidates = attune_core::project_recommender::recommend_for_file(
            vault_guard.store(),
            &item_id_clone,
            &new_ents,
            Some(project_entities),
        )
        .unwrap_or_default();
        if candidates.is_empty() {
            return;
        }
        // Push to ws
        let title_map: std::collections::HashMap<String, String> = projects
            .iter()
            .map(|p| (p.id.clone(), p.title.clone()))
            .collect();
        let payload = serde_json::json!({
            "type": "project_recommendation",
            "trigger": "file_uploaded",
            "file_id": item_id_clone,
            "candidates": candidates.iter().map(|c| serde_json::json!({
                "project_id": c.project_id,
                "project_title": title_map.get(&c.project_id).cloned().unwrap_or_default(),
                "score": c.score,
                "overlapping_entities": c.overlapping_entities,
            })).collect::<Vec<_>>(),
        });
        let _ = state_clone.recommendation_tx.send(payload);
    });
```

注意：这是个 best-effort spawn，失败 silently（不阻塞用户上传响应）。

- [ ] **Step 3: ws.rs 推荐推送扩展**

`rust/crates/attune-server/src/routes/ws.rs`，**完整替换** `handle_scan_progress` 函数：

```rust
async fn handle_scan_progress(mut socket: WebSocket, state: SharedState) {
    let interval = std::time::Duration::from_secs(2);
    let mut rx = state.recommendation_tx.subscribe();

    loop {
        // 1. 推 progress（原有）
        let payload = {
            let vault_guard = state.vault.lock().unwrap_or_else(|e| e.into_inner());
            let vault_state = vault_guard.state();
            if !matches!(vault_state, VaultState::Unlocked) {
                serde_json::json!({
                    "type": "progress",
                    "vault_state": "locked",
                    "pending_embeddings": 0,
                    "pending_classify": 0,
                    "bound_dirs": 0,
                })
            } else {
                let pending_embed = vault_guard
                    .store()
                    .pending_count_by_type("embed")
                    .unwrap_or(0);
                let pending_classify = vault_guard
                    .store()
                    .pending_count_by_type("classify")
                    .unwrap_or(0);
                let bound_dirs = vault_guard
                    .store()
                    .list_bound_directories()
                    .map(|v| v.len())
                    .unwrap_or(0);
                serde_json::json!({
                    "type": "progress",
                    "vault_state": "unlocked",
                    "pending_embeddings": pending_embed,
                    "pending_classify": pending_classify,
                    "bound_dirs": bound_dirs,
                })
            }
        };
        if socket.send(Message::Text(payload.to_string().into())).await.is_err() {
            break;
        }

        // 2. 非阻塞拉所有积压的 recommendation 一并推
        loop {
            match rx.try_recv() {
                Ok(rec_payload) => {
                    if socket.send(Message::Text(rec_payload.to_string().into())).await.is_err() {
                        return;
                    }
                }
                Err(_) => break,
            }
        }

        tokio::time::sleep(interval).await;
    }
}
```

注意 import：顶部加 `use tokio::sync::broadcast;` 之类（如果 broadcast 还没 import）。

- [ ] **Step 4: cargo build 验证编译**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo build --release --workspace 2>&1 | tail -8
```

预期：build OK。

- [ ] **Step 5: 跑全工作区测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
timeout 300 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**405 passed**（无新测试，本 task 主要是 wiring；行为变化通过 ws E2E 在 Sprint 1 Phase D 验证）。

如有测试退化（特别是涉及 AppState 构造的 test）：必须修 — 加 `recommendation_tx: broadcast::channel(64).0` 之类到 mock state。

- [ ] **Step 6: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add rust/crates/attune-server/src/state.rs \
        rust/crates/attune-server/src/routes/upload.rs \
        rust/crates/attune-server/src/routes/ws.rs && \
git commit -m "feat(recommender): file upload → spawn recommender → ws push

Spec §2.3 trigger: file_added.
- AppState gains broadcast::Sender<Value> for recommendations
- routes/upload.rs spawns best-effort task post-upload
- routes/ws.rs interleaves progress ticks with non-blocking recommendation drain

Tests: 405 passed."
```

---

### Task 5: chat 关键词触发器

`routes/chat.rs` 在用户消息进入时检测关键词，命中即推 ws。

**Files:**
- Modify: `rust/crates/attune-server/src/routes/chat.rs`

- [ ] **Step 1: 找 chat handler 的入口**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
grep -nE 'pub async fn chat\(|pub async fn chat_handler' rust/crates/attune-server/src/routes/chat.rs | head
```

记录 chat handler 的开始行号。

- [ ] **Step 2: 在 chat handler 入口处加关键词检测**

打开 `rust/crates/attune-server/src/routes/chat.rs`，在 chat handler 内**接收到用户 message 后**（typically 是从 `req.message` 取值的位置）加：

```rust
    // Sprint 1 Phase B: chat keyword trigger for project recommendation
    if let Some(hint) = attune_core::project_recommender::recommend_for_chat(&req.message) {
        let payload = serde_json::json!({
            "type": "project_recommendation",
            "trigger": "chat_keyword",
            "matched_keywords": hint.matched_keywords,
            "suggestion": hint.suggestion,
        });
        let _ = state.recommendation_tx.send(payload);
    }
```

注意：变量名 `req.message` / `req` / `state` 按现有 chat handler 签名调整。Implementer 需要先 grep 看现有签名：

```bash
grep -A5 'pub async fn chat\b' rust/crates/attune-server/src/routes/chat.rs | head -10
```

- [ ] **Step 3: cargo build**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo build --release --workspace 2>&1 | tail -5
```

预期：build OK。

- [ ] **Step 4: 跑测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
timeout 300 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**405 passed**（无新测试，nudge 行为通过 Phase D E2E 验证）。

- [ ] **Step 5: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add rust/crates/attune-server/src/routes/chat.rs && \
git commit -m "feat(recommender): chat keyword trigger pushes recommendation hint

When user message contains 案件 / 客户 / 项目 / etc., emit ws hint.
Pure observer — no impact on chat handler flow."
```

---

### Task 6: 文档同步

更新 spec 状态 / README endpoint 列表。

**Files:**
- Modify: `docs/superpowers/specs/2026-04-25-industry-attune-design.md`（标记 Sprint 1 Phase B 完成）
- Modify: `rust/README.md`（API endpoints 段加 /api/v1/projects/*）

- [ ] **Step 1: spec 加 Sprint 1 Phase B 完成标记**

读 spec §9 sprint 节奏表。在 Sprint 1 行追加 "Phase A/B 已完成" 备注。

具体：

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
grep -n '^### .*Sprint 1\|^## 9' docs/superpowers/specs/2026-04-25-industry-attune-design.md | head
```

找到 §9 表格中 Sprint 1 行（应在 ~L656），在 "交付" 列后加 "（Phase A+B 完成 yyyy-mm-dd）"。

- [ ] **Step 2: README 加 endpoint**

`rust/README.md` 找 `/api/v1/*` 段，加：

```markdown
- `/api/v1/projects` — Project / Case 卷宗 CRUD（spec §2.1）
- `/api/v1/projects/{id}/files` — 文件归属
- `/api/v1/projects/{id}/timeline` — 案件时间线
```

- [ ] **Step 3: 同步 README.zh.md**

中文版同等改动。

- [ ] **Step 4: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add docs/superpowers/specs/2026-04-25-industry-attune-design.md \
        rust/README.md \
        rust/README.zh.md && \
git commit -m "docs(sprint-1-b): sync spec status + README endpoints

Mark Sprint 1 Phase A+B done in §9 timeline.
Add /api/v1/projects/* endpoints to README dual-language."
```

---

## Self-Review Notes

**Spec coverage:**
- ✅ §2.3 实体重叠度 0.6 阈值 → Task 1 (entity_overlap_score) + Task 2 (RECOMMEND_THRESHOLD)
- ✅ §2.3 触发 1：上传 N 份文件实体重叠检测 → Task 4 (upload.rs spawn recommender)
- ✅ §2.3 触发 2：chat 提到关键词 → Task 5 (recommend_for_chat in chat.rs)
- ✅ §2.3 触发 3：上传新文件实体重叠 ≥ 2 已有 → Task 4（每次 upload 都跑）
- ⏭ §2.3 用户三选一 [新建 / 加入 existing / 跳过] UI → Phase D（前端）
- ⏭ §2.2 attune-law Case 渲染层 → Phase D（前端 + plugin）

**Placeholder scan:** 完整代码 + 完整命令 + 完整预期。

**Type consistency:**
- `RecommendationCandidate { project_id, project_title, score, overlapping_entities }` — Task 2 定义 / Task 4 ws payload 一致
- `ChatTriggerHint { matched_keywords, suggestion }` — Task 2 / Task 5 一致
- `recommendation_tx: broadcast::Sender<serde_json::Value>` — Task 4 state.rs / upload.rs / chat.rs / ws.rs 一致

---

## 完成 Phase B 标志

6 个 Task 全部 checkbox 勾上：
- [ ] `cargo test --workspace`: ≥ **405 passed**
- [ ] `/api/v1/projects` 完整 CRUD 可调（curl test pass）
- [ ] 上传文件触发 recommender + ws 推送（log 可观察）
- [ ] chat 含关键词触发 hint + ws 推送
- [ ] 仍维持 Phase A 的 392 测试 + 添加的 13 个 Phase B 测试
- [ ] 文档（spec + README）同步

完成后：
- Phase C 写跨证据链 workflow（attune-law plugin 的 evidence_chain_inference workflow）
- Phase D 前端 Project tab + 推荐确认 UI + attune-law Case 渲染层
- Phase E E2E + finishing-branch
