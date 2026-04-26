# Sprint 2 Skills Router: 完整 skill 接入路由 + 用户管理 UI

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 让用户在 chat 里说"帮我审查合同"自动触发 contract_review skill 提示；通过 Settings → Skills tab 可视化启用/禁用、查看 keywords，**完全不用改 yaml**。免费版 + Pro 版共享同一加载机制，Pro 版仅多预装官方 skill 集。

**Architecture:**
- `PluginManifest` 加 `chat_trigger: Option<ChatTrigger>` 字段（patterns / keywords / min_keyword_match / exclude_patterns / requires_document / priority / needs_confirm）
- `IntentRouter::route(message, has_pending_doc, disabled_set)` 在 attune-core 加，扫所有已加载 plugin 的 chat_trigger 匹配
- `routes/chat.rs` 在 user message 进入时调 `IntentRouter::route` → ws push `skill_suggested` payload
- `Settings.skills.disabled` 持久化禁用集（`{disabled_skill_ids: [...]}`）
- UI: 新 Skills tab 列出 registry 加载的所有 type=skill plugins，toggle 启用/禁用、显示 keywords + description
- 写 skill 教程加到 README（plugin.yaml 示例 + 放在哪）

**Tech Stack:**
- 现有 PluginRegistry / serde_yaml / Settings persistence
- Preact + Signals UI
- 不动后端 LLM provider / workflow 引擎

**Spec source:** [`docs/superpowers/specs/2026-04-25-industry-attune-design.md`](../specs/2026-04-25-industry-attune-design.md) §3.1 §3.2

---

## File Structure

**Create:**
- `rust/crates/attune-core/src/intent_router.rs` — IntentRouter + ChatTrigger 抽出
- `rust/crates/attune-server/ui/src/views/SkillsView.tsx` — Skills 管理 tab

**Modify:**
- `rust/crates/attune-core/src/plugin_loader.rs` — PluginManifest 加 chat_trigger
- `rust/crates/attune-core/src/lib.rs` — `pub mod intent_router;`
- `rust/crates/attune-server/src/routes/chat.rs` — 调 IntentRouter
- `rust/crates/attune-server/src/routes/settings.rs` — 加 `skills.disabled` 字段处理
- `rust/crates/attune-server/ui/src/views/index.ts` + `App.tsx` + `Sidebar.tsx` + `signals.ts` — Skills view 注册
- `README.md` + `README.zh.md` — 加"如何写自己的 skill"段

---

## Progress Tracking

每 Task 一个独立 commit。中间维持 `cargo test --workspace` ≥ 423 passed。

---

### Task 1: PluginManifest 加 chat_trigger 字段

**Files:**
- Modify: `rust/crates/attune-core/src/plugin_loader.rs`

- [ ] **Step 1: 在 PluginManifest struct 加 chat_trigger 字段**

打开 `rust/crates/attune-core/src/plugin_loader.rs`，在 PluginManifest struct 末尾追加：

```rust
    // Sprint 2 Skills Router: chat 关键词路由（type=skill 时使用）
    #[serde(default)]
    pub chat_trigger: Option<ChatTrigger>,
}

/// chat_trigger 配置（参考 lawcontrol skill plugin.yaml）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatTrigger {
    /// 是否启用 chat 触发（plugin.yaml 默认 false）
    #[serde(default)]
    pub enabled: bool,

    /// 触发后是否需要用户显式确认才跑 skill（默认 true，安全优先）
    #[serde(default = "default_true")]
    pub needs_confirm: bool,

    /// 多个 skill 同时命中时优先级（数字越大越优先）
    #[serde(default)]
    pub priority: i32,

    /// 正则模式列表（任一命中算匹配）
    #[serde(default)]
    pub patterns: Vec<String>,

    /// 关键词列表（命中数 >= min_keyword_match 算匹配）
    #[serde(default)]
    pub keywords: Vec<String>,

    /// 关键词最小命中数（默认 1）
    #[serde(default = "default_one")]
    pub min_keyword_match: usize,

    /// 否决正则（任一命中即否决，即使 patterns/keywords 命中）
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// 是否要求 chat 上下文有 pending file（如 contract_review 需要文件）
    #[serde(default)]
    pub requires_document: bool,

    /// 短描述（UI 展示）
    #[serde(default)]
    pub description: String,
}

fn default_true() -> bool { true }
fn default_one() -> usize { 1 }
```

注：原 PluginManifest 末尾 `}` 要保留 — chat_trigger 字段插在 `}` 之前，新加的 `pub struct ChatTrigger` 在 PluginManifest 之后。

- [ ] **Step 2: 加 unit test**

PluginManifest 现有 `mod tests` 末尾追加：

```rust
    #[test]
    fn parses_skill_with_chat_trigger() {
        let yaml = r#"
id: law-pro/contract_review
name: 合同风险审查
type: skill
version: "0.1.0"
chat_trigger:
  enabled: true
  needs_confirm: true
  priority: 5
  patterns:
    - '帮我.*审查.*合同'
  keywords: ['审查合同', '合同风险']
  min_keyword_match: 1
  exclude_patterns: ['起草']
  requires_document: true
  description: AI 审查合同条款风险
"#;
        let m: PluginManifest = serde_yaml::from_str(yaml).expect("parse");
        let ct = m.chat_trigger.expect("should have chat_trigger");
        assert!(ct.enabled);
        assert!(ct.needs_confirm);
        assert_eq!(ct.priority, 5);
        assert_eq!(ct.keywords.len(), 2);
        assert_eq!(ct.min_keyword_match, 1);
        assert!(ct.requires_document);
    }

    #[test]
    fn parses_skill_without_chat_trigger() {
        let yaml = r#"
id: simple-skill
name: 简单 skill
type: skill
version: "1.0.0"
"#;
        let m: PluginManifest = serde_yaml::from_str(yaml).expect("parse");
        assert!(m.chat_trigger.is_none());
    }
```

- [ ] **Step 3: 跑测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills/rust && \
cargo test --release -p attune-core plugin_loader 2>&1 | tail -10
```

预期：所有 plugin_loader tests pass（含新加 2）。

- [ ] **Step 4: 全工作区**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills/rust && \
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**425 passed**（423 + 2）。

- [ ] **Step 5: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills && \
git add rust/crates/attune-core/src/plugin_loader.rs && \
git commit -m "feat(plugin): PluginManifest 加 chat_trigger 字段

ChatTrigger { enabled / needs_confirm / priority / patterns / keywords /
              min_keyword_match / exclude_patterns / requires_document / description }

Sprint 2 Skills Router 前置：让 IntentRouter 能从 plugin.yaml 读路由配置。

Tests: 425 passed (423 baseline + 2 chat_trigger parse)."
```

---

### Task 2: IntentRouter 实现

**Files:**
- Create: `rust/crates/attune-core/src/intent_router.rs`
- Modify: `rust/crates/attune-core/src/lib.rs`

- [ ] **Step 1: 创建 intent_router.rs**

```rust
//! IntentRouter — 把用户 chat 消息路由到 plugin 注册的 skill。
//!
//! 设计：纯函数 + 正则 + 关键词；不调 LLM。
//! 调用方传 PluginRegistry + 消息 + 上下文（是否含文件）+ 禁用集，
//! 返回排序好的候选 skill 列表（按 priority 降序）。

use crate::plugin_loader::{ChatTrigger, LoadedPlugin};
use crate::plugin_registry::PluginRegistry;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMatch {
    pub skill_id: String,
    pub skill_name: String,
    pub priority: i32,
    pub needs_confirm: bool,
    pub description: String,
    /// 命中时机：哪些 keyword / pattern 触发了
    pub matched_via: Vec<String>,
}

pub struct IntentRouter<'a> {
    registry: &'a PluginRegistry,
}

impl<'a> IntentRouter<'a> {
    pub fn new(registry: &'a PluginRegistry) -> Self {
        Self { registry }
    }

    /// 路由 chat 消息到候选 skill。
    ///
    /// - `message`: 用户消息
    /// - `has_pending_document`: chat 上下文是否含文件（影响 requires_document filter）
    /// - `disabled_skills`: 用户禁用的 skill_id 集合
    ///
    /// 返回按 priority 降序排列的候选。
    pub fn route(
        &self,
        message: &str,
        has_pending_document: bool,
        disabled_skills: &HashSet<String>,
    ) -> Vec<SkillMatch> {
        let mut matches = Vec::new();

        for plugin in self.registry.plugins() {
            // 仅 type=skill 且 chat_trigger.enabled
            if plugin.manifest.plugin_type != "skill" {
                continue;
            }
            if disabled_skills.contains(&plugin.manifest.id) {
                continue;
            }
            let Some(trigger) = &plugin.manifest.chat_trigger else {
                continue;
            };
            if !trigger.enabled {
                continue;
            }
            // requires_document 过滤
            if trigger.requires_document && !has_pending_document {
                continue;
            }
            // exclude_patterns 否决
            if Self::matches_any_regex(message, &trigger.exclude_patterns) {
                continue;
            }
            // 主匹配：patterns OR keywords
            let mut matched_via = Vec::new();
            if let Some(p) = Self::first_matching_pattern(message, &trigger.patterns) {
                matched_via.push(format!("pattern: {}", p));
            }
            let kw_hits = Self::keyword_hits(message, &trigger.keywords);
            if kw_hits.len() >= trigger.min_keyword_match.max(1) {
                for k in kw_hits.iter().take(3) {
                    matched_via.push(format!("keyword: {}", k));
                }
            }
            if matched_via.is_empty() {
                continue;
            }
            matches.push(SkillMatch {
                skill_id: plugin.manifest.id.clone(),
                skill_name: plugin.manifest.name.clone(),
                priority: trigger.priority,
                needs_confirm: trigger.needs_confirm,
                description: if trigger.description.is_empty() {
                    plugin.manifest.description.clone()
                } else {
                    trigger.description.clone()
                },
                matched_via,
            });
        }

        matches.sort_by(|a, b| b.priority.cmp(&a.priority));
        matches
    }

    fn matches_any_regex(message: &str, patterns: &[String]) -> bool {
        for p in patterns {
            if let Ok(re) = Regex::new(p) {
                if re.is_match(message) {
                    return true;
                }
            }
        }
        false
    }

    fn first_matching_pattern(message: &str, patterns: &[String]) -> Option<String> {
        for p in patterns {
            if let Ok(re) = Regex::new(p) {
                if re.is_match(message) {
                    return Some(p.clone());
                }
            }
        }
        None
    }

    fn keyword_hits(message: &str, keywords: &[String]) -> Vec<String> {
        keywords.iter().filter(|k| message.contains(k.as_str())).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_loader::{LoadedPlugin, PluginManifest};

    fn make_skill_plugin(id: &str, trigger: ChatTrigger) -> LoadedPlugin {
        let mut manifest = PluginManifest {
            id: id.into(),
            name: id.into(),
            plugin_type: "skill".into(),
            version: "1.0.0".into(),
            ..Default::default()
        };
        manifest.chat_trigger = Some(trigger);
        LoadedPlugin {
            manifest,
            prompt: String::new(),
        }
    }

    fn registry_with(plugins: Vec<LoadedPlugin>) -> PluginRegistry {
        let mut reg = PluginRegistry::new();
        // PluginRegistry 没有公开 insert API；用 hack 方式手动构造
        // 这里改用 scan with tempdir 方式，避免动 PluginRegistry 公开 API
        // 简化：用 tempdir 模拟（与现有 plugin_registry tests 一致）
        let _ = plugins;
        reg
    }

    #[test]
    fn route_skill_with_keyword_match() {
        // 测试用 tempdir 创建 skill plugin，避免改 PluginRegistry API
        use std::fs;
        use tempfile::TempDir;
        let tmp = TempDir::new().expect("tmp");
        let pdir = tmp.path().join("contract-skill");
        fs::create_dir_all(&pdir).expect("mkdir");
        fs::write(
            pdir.join("plugin.yaml"),
            r#"
id: contract-skill
name: 合同审查
type: skill
version: "1.0.0"
chat_trigger:
  enabled: true
  priority: 5
  keywords: ['合同', '审查']
  min_keyword_match: 1
  description: AI 合同风险审查
"#,
        ).expect("write yaml");

        let (reg, errs) = PluginRegistry::scan(tmp.path()).expect("scan");
        assert!(errs.is_empty());
        let router = IntentRouter::new(&reg);

        let disabled: HashSet<String> = HashSet::new();
        let matches = router.route("帮我审查这份合同", false, &disabled);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].skill_id, "contract-skill");
        assert_eq!(matches[0].priority, 5);
    }

    #[test]
    fn route_filters_disabled() {
        use std::fs;
        use tempfile::TempDir;
        let tmp = TempDir::new().expect("tmp");
        let pdir = tmp.path().join("disabled-skill");
        fs::create_dir_all(&pdir).expect("mkdir");
        fs::write(
            pdir.join("plugin.yaml"),
            r#"
id: disabled-skill
name: 已禁用
type: skill
version: "1.0.0"
chat_trigger:
  enabled: true
  keywords: ['hello']
"#,
        ).expect("write");

        let (reg, _) = PluginRegistry::scan(tmp.path()).expect("scan");
        let router = IntentRouter::new(&reg);
        let mut disabled = HashSet::new();
        disabled.insert("disabled-skill".into());

        let matches = router.route("hello", false, &disabled);
        assert!(matches.is_empty());
    }

    #[test]
    fn route_requires_document_filter() {
        use std::fs;
        use tempfile::TempDir;
        let tmp = TempDir::new().expect("tmp");
        let pdir = tmp.path().join("doc-skill");
        fs::create_dir_all(&pdir).expect("mkdir");
        fs::write(
            pdir.join("plugin.yaml"),
            r#"
id: doc-skill
name: 需要文件
type: skill
version: "1.0.0"
chat_trigger:
  enabled: true
  keywords: ['分析']
  requires_document: true
"#,
        ).expect("write");

        let (reg, _) = PluginRegistry::scan(tmp.path()).expect("scan");
        let router = IntentRouter::new(&reg);
        let disabled = HashSet::new();

        // 无文件 → 不命中
        let m1 = router.route("帮我分析", false, &disabled);
        assert!(m1.is_empty());

        // 有文件 → 命中
        let m2 = router.route("帮我分析", true, &disabled);
        assert_eq!(m2.len(), 1);
    }

    #[test]
    fn route_exclude_pattern_vetoes() {
        use std::fs;
        use tempfile::TempDir;
        let tmp = TempDir::new().expect("tmp");
        let pdir = tmp.path().join("draft-skill");
        fs::create_dir_all(&pdir).expect("mkdir");
        fs::write(
            pdir.join("plugin.yaml"),
            r#"
id: draft-skill
name: 起草
type: skill
version: "1.0.0"
chat_trigger:
  enabled: true
  keywords: ['合同']
  exclude_patterns: ['审查']
"#,
        ).expect("write");

        let (reg, _) = PluginRegistry::scan(tmp.path()).expect("scan");
        let router = IntentRouter::new(&reg);
        let disabled = HashSet::new();

        // 含 keyword "合同" 但触发 exclude "审查" → 不命中
        let m = router.route("帮我审查合同", false, &disabled);
        assert!(m.is_empty());
    }
}
```

注意：`LoadedPlugin / PluginManifest` 用了 Default — 如果实际未 derive Default，需要手动构造完整实例。先看 plugin_loader.rs 是否 derive Default：

```bash
grep -E '#\[derive.*Default\]\|impl Default' rust/crates/attune-core/src/plugin_loader.rs | head
```

如果 PluginManifest 没 derive Default，把 `..Default::default()` 替换成完整字段（id/name/plugin_type/version 已写，再补 description/author/category/label_prefix/default_color/constraints/prompt_file/output 全空字符串/None）。或者直接给 PluginManifest 加 `#[derive(Default)]`。

- [ ] **Step 2: lib.rs 注册**

```rust
pub mod intent_router;
```

按字母序合适位置（`pub mod entities;` 后 / `pub mod plugin_loader;` 前）。

- [ ] **Step 3: 跑测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills/rust && \
cargo test --release -p attune-core intent_router 2>&1 | tail -25
```

预期：4 tests pass（route_skill_with_keyword_match / route_filters_disabled / route_requires_document_filter / route_exclude_pattern_vetoes）。

- [ ] **Step 4: 全工作区**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills/rust && \
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**429 passed**（425 + 4）。

- [ ] **Step 5: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills && \
git add rust/crates/attune-core/src/intent_router.rs \
        rust/crates/attune-core/src/lib.rs \
        rust/crates/attune-core/src/plugin_loader.rs && \
git commit -m "feat(intent_router): chat 关键词路由到 plugin skill

IntentRouter::route(message, has_pending_doc, disabled_skills) → Vec<SkillMatch>
- 扫所有 type=skill plugin 的 chat_trigger
- patterns 正则 OR keywords (>= min_keyword_match) 触发
- exclude_patterns 否决 / requires_document 过滤 / disabled 集合屏蔽
- priority 降序排列

Tests: 429 passed (425 baseline + 4 router unit)."
```

---

### Task 3: chat handler 调 IntentRouter + ws push

**Files:**
- Modify: `rust/crates/attune-server/src/routes/chat.rs`

- [ ] **Step 1: 在 chat handler 入口加 IntentRouter 调用**

打开 `rust/crates/attune-server/src/routes/chat.rs`，找 Sprint 1 加的 `recommend_for_chat` 调用块。在它后面追加：

```rust
    // Sprint 2 Skills Router: 路由到 plugin skill
    {
        let registry = state.plugin_registry.clone();
        let disabled = std::collections::HashSet::new(); // Task 4 接 Settings.skills.disabled
        let has_pending_doc = false; // Task 5 可由 chat context 决定
        let router = attune_core::intent_router::IntentRouter::new(&registry);
        let matches = router.route(&body.message, has_pending_doc, &disabled);
        if !matches.is_empty() {
            let payload = serde_json::json!({
                "type": "skill_suggested",
                "matches": matches,
                "user_message": &body.message,
            });
            let _ = state.recommendation_tx.send(payload);
        }
    }
```

注意：变量名 `body.message` / `state` 按现有 chat handler 调整。`state.plugin_registry` 是 `Arc<PluginRegistry>`（Sprint 2 Phase A 已加），可 clone。

- [ ] **Step 2: cargo build + 测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills/rust && \
cargo build --release --workspace 2>&1 | tail -5
echo '---'
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：build OK + **429 passed**（无新测试）。

- [ ] **Step 3: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills && \
git add rust/crates/attune-server/src/routes/chat.rs && \
git commit -m "feat(chat): IntentRouter 路由后 ws 推 skill_suggested

Pure observer (与 Phase B project_recommendation 同构):
- IntentRouter::route(body.message, false, &empty_disabled)
- 命中 skills 通过 broadcast 推到前端
- Task 4 后接 Settings.skills.disabled，Task 5 后接 has_pending_doc

Tests: 429 passed."
```

---

### Task 4: Settings.skills.disabled + 路由过滤

**Files:**
- Modify: `rust/crates/attune-server/src/routes/settings.rs`
- Modify: `rust/crates/attune-server/src/routes/chat.rs`

- [ ] **Step 1: 加 skills 字段到 settings schema**

打开 `rust/crates/attune-server/src/routes/settings.rs`，找 default settings JSON 模板。在末尾加：

```json
"skills": { "disabled": [] }
```

并在 ALLOWED_KEYS 列表加 `"skills"`。如有 schema 校验代码，确保 skills.disabled 接受 array of strings。

- [ ] **Step 2: chat handler 读 disabled 集合**

修改 chat.rs 里 Task 3 加的 IntentRouter 块：

```rust
    // 读 settings.skills.disabled
    let disabled: std::collections::HashSet<String> = state
        .vault.lock().unwrap_or_else(|e| e.into_inner())
        .store()
        .get_meta("app_settings")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("skills").and_then(|s| s.get("disabled")).cloned())
        .and_then(|d| d.as_array().map(|a| {
            a.iter().filter_map(|x| x.as_str().map(String::from)).collect()
        }))
        .unwrap_or_default();
```

注意：vault.store().get_meta 是从 vault_meta 表读 settings — 实际 path 看 settings.rs 现有怎么读，按相同方式读。如果 settings 通过 state.cached_settings 之类内存缓存提供，用那个 path。

- [ ] **Step 3: cargo build + 测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills/rust && \
cargo build --release --workspace 2>&1 | tail -5
echo '---'
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：build OK + **429 passed**.

- [ ] **Step 4: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills && \
git add rust/crates/attune-server/src/routes/settings.rs \
        rust/crates/attune-server/src/routes/chat.rs && \
git commit -m "feat(settings): skills.disabled 持久化 + IntentRouter 读取

Settings schema 加 skills.disabled: string[] 字段。
chat handler 在调 IntentRouter 前从 settings 读 disabled 集合，
让用户禁用过的 skill 永不被路由命中。

Tests: 429 passed."
```

---

### Task 5: UI Skills tab — 列出 + 启用/禁用 toggle

**Files:**
- Create: `rust/crates/attune-server/ui/src/views/SkillsView.tsx`
- Modify: `rust/crates/attune-server/ui/src/views/index.ts`
- Modify: `rust/crates/attune-server/ui/src/store/signals.ts`（加 view 类型）
- Modify: `rust/crates/attune-server/ui/src/layout/Sidebar.tsx`（加 nav item）
- Modify: `rust/crates/attune-server/ui/src/layout/MainShell.tsx`（加 view dispatch）

- [ ] **Step 1: 后端 list 端点**（如不存在，加一个）

```bash
grep -n '/api/v1/skills\|/api/v1/plugins' rust/crates/attune-server/src/lib.rs
```

如已有 `/api/v1/plugins` 列出 plugins，扩展返回 chat_trigger 字段；否则创建 `/api/v1/skills`：

`rust/crates/attune-server/src/routes/skills.rs`（新建，如已有 plugins.rs 则在那里加）:

```rust
//! GET /api/v1/skills — 列出 type=skill 的所有 plugin（含 chat_trigger）

use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::SharedState;

#[derive(Serialize)]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub keywords: Vec<String>,
    pub patterns: Vec<String>,
    pub enabled_in_plugin: bool,  // chat_trigger.enabled
    pub disabled_by_user: bool,   // settings.skills.disabled.contains(id)
}

#[derive(Serialize)]
pub struct SkillsListResponse {
    pub skills: Vec<SkillSummary>,
}

pub async fn list_skills(
    State(state): State<SharedState>,
) -> Json<SkillsListResponse> {
    // 读 disabled set
    let vault_guard = state.vault.lock().unwrap_or_else(|e| e.into_inner());
    let disabled: std::collections::HashSet<String> = vault_guard
        .store()
        .get_meta("app_settings")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("skills").and_then(|s| s.get("disabled")).cloned())
        .and_then(|d| d.as_array().map(|a| {
            a.iter().filter_map(|x| x.as_str().map(String::from)).collect()
        }))
        .unwrap_or_default();
    drop(vault_guard);

    let registry = state.plugin_registry.clone();
    let mut skills = Vec::new();
    for plugin in registry.plugins_by_type("skill") {
        let trigger = &plugin.manifest.chat_trigger;
        skills.push(SkillSummary {
            id: plugin.manifest.id.clone(),
            name: plugin.manifest.name.clone(),
            description: plugin.manifest.description.clone(),
            version: plugin.manifest.version.clone(),
            keywords: trigger.as_ref().map(|t| t.keywords.clone()).unwrap_or_default(),
            patterns: trigger.as_ref().map(|t| t.patterns.clone()).unwrap_or_default(),
            enabled_in_plugin: trigger.as_ref().map(|t| t.enabled).unwrap_or(false),
            disabled_by_user: disabled.contains(&plugin.manifest.id),
        });
    }
    Json(SkillsListResponse { skills })
}
```

注册到 build_router：

```rust
.route("/api/v1/skills", get(routes::skills::list_skills))
```

routes/mod.rs 加：

```rust
pub mod skills;
```

- [ ] **Step 2: 创建 SkillsView.tsx**

`rust/crates/attune-server/ui/src/views/SkillsView.tsx`:

```tsx
//! Skills 管理 — 列出 plugin 注册的所有 skill，启用/禁用 toggle，查看 keywords。
//! 配置简单：用户从不需要改 yaml。

import { useState, useEffect, useMemo } from 'preact/hooks';
import { api } from '../store/api';

interface SkillSummary {
  id: string;
  name: string;
  description: string;
  version: string;
  keywords: string[];
  patterns: string[];
  enabled_in_plugin: boolean;
  disabled_by_user: boolean;
}

interface SkillsListResponse {
  skills: SkillSummary[];
}

export function SkillsView() {
  const [skills, setSkills] = useState<SkillSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const reload = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await api.get<SkillsListResponse>('/api/v1/skills');
      setSkills(res.skills);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    reload();
  }, []);

  const setDisabled = async (skill: SkillSummary, disabled: boolean) => {
    try {
      const cur = skills.find((s) => s.id === skill.id);
      if (!cur) return;
      // PATCH settings: skills.disabled
      const settings = await api.get<any>('/api/v1/settings');
      const cur_disabled: string[] = (settings.skills?.disabled ?? []) as string[];
      const next = disabled
        ? Array.from(new Set([...cur_disabled, skill.id]))
        : cur_disabled.filter((s) => s !== skill.id);
      await api.patch('/api/v1/settings', { skills: { disabled: next } });
      // 更新本地状态
      setSkills((arr) =>
        arr.map((s) =>
          s.id === skill.id ? { ...s, disabled_by_user: disabled } : s
        )
      );
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <div class="skills-view">
      <header class="view-header">
        <h2>Skills</h2>
        <button onClick={reload} disabled={loading}>
          {loading ? '加载中…' : '刷新'}
        </button>
      </header>

      {error && <div class="error-banner">⚠ {error}</div>}

      {skills.length === 0 && !loading && (
        <div class="empty">
          暂无已安装的 skill。把 .attunepkg 解压到{' '}
          <code>~/.local/share/attune/plugins/&lt;name&gt;/</code> 或者参考{' '}
          <a href="https://github.com/attune/attune/blob/main/README.md#skill-development">README</a>{' '}
          自己写一个。
        </div>
      )}

      <ul class="skills-list">
        {skills.map((s) => (
          <li key={s.id}>
            <div class="skill-row">
              <div class="skill-meta">
                <div class="skill-title">
                  {s.name}{' '}
                  <span class="skill-version">v{s.version}</span>
                </div>
                <div class="skill-id">
                  <code>{s.id}</code>
                </div>
                <div class="skill-desc">{s.description || '—'}</div>
                <div class="skill-keywords">
                  {s.keywords.length > 0 && (
                    <>
                      <span class="label">关键词触发：</span>
                      {s.keywords.map((k) => (
                        <span class="kw" key={k}>{k}</span>
                      ))}
                    </>
                  )}
                  {s.patterns.length > 0 && (
                    <span class="patterns" title={s.patterns.join('\n')}>
                      + {s.patterns.length} 正则模式
                    </span>
                  )}
                </div>
              </div>
              <div class="skill-toggle">
                <label class="switch">
                  <input
                    type="checkbox"
                    checked={!s.disabled_by_user && s.enabled_in_plugin}
                    disabled={!s.enabled_in_plugin}
                    onChange={(e) => setDisabled(s, !(e.target as HTMLInputElement).checked)}
                  />
                  <span>{s.disabled_by_user ? '已禁用' : (s.enabled_in_plugin ? '已启用' : '未启用 (yaml 内 enabled: false)')}</span>
                </label>
              </div>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

- [ ] **Step 3: views/index.ts + signals + Sidebar + MainShell 注册**

```ts
// views/index.ts
export { SkillsView } from './SkillsView';

// store/signals.ts: View 类型加 'skills'
export type View = 'chat' | 'items' | 'projects' | 'skills' | 'knowledge' | 'remote' | 'settings';

// layout/Sidebar.tsx: NAV_ITEMS 加
{ view: 'skills', icon: '🧠', label: 'Skills' },

// layout/MainShell.tsx:
{view === 'skills' && <SkillsView />}
```

- [ ] **Step 4: 加最简 CSS**

styles/ 现有 css 末尾追加：

```css
.skills-view {
  padding: 1rem;
}
.skills-view .view-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 1rem;
}
.skills-view .view-header h2 {
  flex: 1;
  margin: 0;
}
.skills-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.skills-list li {
  border: 1px solid var(--border, #ddd);
  border-radius: 6px;
  padding: 0.75rem 1rem;
  margin-bottom: 0.5rem;
}
.skill-row {
  display: flex;
  gap: 1rem;
  align-items: flex-start;
}
.skill-meta {
  flex: 1;
  min-width: 0;
}
.skill-title {
  font-weight: 500;
  margin-bottom: 0.25rem;
}
.skill-version {
  color: #888;
  font-size: 0.85em;
  font-weight: normal;
}
.skill-id {
  font-size: 0.85em;
  color: #888;
}
.skill-desc {
  margin: 0.5rem 0;
  color: #555;
}
.skill-keywords {
  font-size: 0.85em;
  display: flex;
  flex-wrap: wrap;
  gap: 0.25rem;
  align-items: center;
}
.skill-keywords .label {
  color: #888;
}
.skill-keywords .kw {
  background: var(--bg-tag, #eef);
  padding: 1px 6px;
  border-radius: 3px;
}
.skill-keywords .patterns {
  color: #888;
  font-style: italic;
}
.skill-toggle .switch {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.skill-toggle input {
  width: 32px;
  height: 18px;
}
```

- [ ] **Step 5: build + 测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills/rust/crates/attune-server/ui && \
npm run build 2>&1 | tail -5
echo '---'
cd /data/company/project/attune/.worktrees/sprint-2-skills/rust && \
cargo build --release --workspace 2>&1 | tail -5
echo '---'
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：build OK + **429 passed**。

- [ ] **Step 6: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills && \
git add rust/crates/attune-server/src/routes/ \
        rust/crates/attune-server/src/lib.rs \
        rust/crates/attune-server/ui/ && \
git commit -m "feat(ui): Skills tab — toggle 启用/禁用 + 关键词预览

GET /api/v1/skills 列出所有 type=skill plugin（含 chat_trigger 字段 + 用户禁用状态）.
SkillsView UI:
- 列名称 / 版本 / 描述 / keywords 高亮显示
- 启用/禁用 toggle (PATCH /settings skills.disabled)
- yaml enabled=false 的 skill 显示 '未启用 (yaml 内)' 灰态
- 空 registry 提示用户写 / 装 plugin

配置简单：用户从不需要改 yaml；toggle 即时生效。

Tests: 429 passed."
```

---

### Task 6: README 写"如何写自己的 skill"段（双语）

**Files:**
- Modify: `README.md`
- Modify: `README.zh.md`

- [ ] **Step 1: 加段（找合适位置，typically 在 AI 平台之后）**

`README.md`:

```markdown
## Skill 开发（免费版 + Pro 版通用）

写一个自定义 skill 让 chat 关键词自动触发：

1. 创建目录 `~/.local/share/attune/plugins/<my-plugin>/capabilities/<skill-id>/`
2. 写 `plugin.yaml`:

```yaml
id: my-plugin/contract-quick-review
name: 快速合同审查
type: skill
version: "0.1.0"
description: 30 秒读完合同关键风险

chat_trigger:
  enabled: true
  needs_confirm: true
  priority: 5
  patterns:
    - '帮我.*审查.*合同'
  keywords: ['审查合同', '合同风险']
  min_keyword_match: 1
  exclude_patterns: ['起草', '生成']
  requires_document: true
  description: AI 审查合同条款风险
```

3. 写 `prompt.md`（skill 实际 prompt，被 LLM 加载）
4. 重启 attune-server 让 PluginRegistry 重新扫描
5. 打开 Settings → Skills，查看新 skill 是否出现，toggle 启用

Pro 版预装行业 skill 集（律师 / 售前 / 学术等），机制完全一样。
社区 skill 通过 .attunepkg 打包分发，解压到 plugins 目录即装即用。
```

`README.zh.md` 加同等中文段（可与上面一致中文化）。

- [ ] **Step 2: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-2-skills && \
git add README.md README.zh.md && \
git commit -m "docs(readme): skill 开发教程（免费版 + Pro 版通用）

5 步教用户写自定义 skill：
- 目录约定 ~/.local/share/attune/plugins/<plugin>/capabilities/<skill>/
- plugin.yaml 完整示例（含 chat_trigger）
- prompt.md
- 重启扫描
- Settings → Skills 启用

Pro 版只是预装更多 skill；机制免费版/Pro 版一致。"
```

---

## Self-Review Notes

**Spec coverage:**
- ✅ §3.1 plugin.yaml chat_trigger → Task 1
- ✅ §3.2 IntentRouter route → Task 2
- ✅ chat 自然语言 → skill 路由 → Task 3
- ✅ 用户启用/禁用持久化 → Task 4
- ✅ 配置简单（UI 全管 yaml 不用改） → Task 5
- ✅ 免费版/Pro 版同一机制（attune-core 通用底座）+ 用户文档 → Task 6
- ⏭ "测试 skill" modal / "查看 prompt" 链接 → 留下个 sprint
- ⏭ 安装 .attunepkg UI（拖拽 / 选文件）→ 留下个 sprint

**Type consistency:**
- `ChatTrigger` 跨 plugin_loader / intent_router 一致
- `SkillMatch` / `SkillSummary` 后端 / 前端 mirror
- View 类型加 'skills' 跨 signals / Sidebar / MainShell 一致

---

## 完成 Phase 标志

6 个 Task 全部 checkbox 勾上 + Tests ≥ 429 passed + 文档更新 + UI Skills tab 可点 + 启用/禁用 toggle 即时生效。
