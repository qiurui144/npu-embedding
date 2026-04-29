# Sprint 1 Phase A: store.rs 拆分 + Project Schema + 实体抽取

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `store.rs` (2403 行) 按主题拆成 `store/` 子模块（R5 backlog 落地），同时引入 spec §2 的 Project / ProjectFile / ProjectTimeline 三表 + 实体抽取 `entities.rs` 作为后续 Phase B 的"AI 推荐归类"前置依赖。

**Architecture:**
- Rust inherent impl 跨文件分裂：`impl Store { ... }` 可写在 9 个独立文件中，rustc 自动合并。无需 trait 化重构。
- `store/mod.rs` 持有 `SCHEMA const` + `pub struct Store` + `open/open_memory/migrate`；其余主题分到 `store/{items,dirs,queue,tags,history,conversations,signals,chunk_summaries,annotations,project}.rs`。
- `entities.rs` 是新独立模块，纯函数 `extract_entities(text: &str) -> Vec<Entity>`，正则 + 启发式规则。Sprint 1 Phase B 将用它做"AI 推荐归类"的实体重叠度计算。

**Tech Stack:**
- rusqlite 0.32（已在用）
- regex 1（attune-core 间接依赖，需要时显式加）
- 测试：固定语料（CLAUDE.md "零随机测试数据"原则）

**Spec source:** [`docs/superpowers/specs/2026-04-25-industry-attune-design.md`](../specs/2026-04-25-industry-attune-design.md) §2.1 / §2.2

---

## File Structure

**Modify:**
- `rust/crates/attune-core/src/lib.rs` — `pub mod store;` 不变（mod 名一样，从 file 改 dir 后透明）
- `rust/crates/attune-core/Cargo.toml` — 加 `regex = "1"` 到 `[dependencies]`（用于 entities.rs）

**Move:**
- `rust/crates/attune-core/src/store.rs` → `rust/crates/attune-core/src/store/mod.rs`（保留 SCHEMA const + struct Store + open/open_memory/migrate；其余拆出去）

**Create:**
- `rust/crates/attune-core/src/store/types.rs` — 所有共享 pub struct（RawItem / ItemSummary / StaleItemSummary / ItemStats / QueueTask / BoundDirRow / SearchHistoryRow / IndexedFileRow / ConversationSummary / Citation / ConvMessage / SkillSignal / Annotation / AnnotationInput）
- `rust/crates/attune-core/src/store/items.rs` — items 表 CRUD
- `rust/crates/attune-core/src/store/dirs.rs` — bound_dirs / indexed_files
- `rust/crates/attune-core/src/store/queue.rs` — embed_queue
- `rust/crates/attune-core/src/store/history.rs` — search_history / click_events / feedback
- `rust/crates/attune-core/src/store/conversations.rs` — sessions / conversations / conversation_messages
- `rust/crates/attune-core/src/store/signals.rs` — skill_signals
- `rust/crates/attune-core/src/store/chunk_summaries.rs` — chunk_summaries
- `rust/crates/attune-core/src/store/annotations.rs` — annotations
- `rust/crates/attune-core/src/store/project.rs` — **新**：Project / ProjectFile / ProjectTimeline schema + Repo 方法
- `rust/crates/attune-core/src/entities.rs` — **新**：实体抽取（人名 / 金额 / 日期 / 案号 / 公司）
- `rust/crates/attune-core/tests/entities_test.rs` — entities.rs 端到端测试（固定语料）

---

## Progress Tracking

每 Task 完成后回到本文件勾 checkbox。每 Task 一个独立 commit。中间确保 `cargo test --workspace` 维持 ≥ 377 passed。

---

### Task 1: 拆 store.rs 骨架 + types.rs

把 `store.rs` 移到 `store/mod.rs`（行为不变），把所有 `pub struct ...`（共 14 个）抽到 `store/types.rs`。

**Files:**
- Move: `rust/crates/attune-core/src/store.rs` → `rust/crates/attune-core/src/store/mod.rs`
- Create: `rust/crates/attune-core/src/store/types.rs`

- [ ] **Step 1: 创建子目录 + git mv**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
mkdir -p rust/crates/attune-core/src/store && \
git mv rust/crates/attune-core/src/store.rs rust/crates/attune-core/src/store/mod.rs
ls rust/crates/attune-core/src/store/
```

预期：`mod.rs` 在 store/ 目录里，`store.rs` 不再存在。

- [ ] **Step 2: 跑测试确认 mod 名变化无影响**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：377 passed, 0 failed（mod 名 `store` 不变，`mod.rs` 等价于原 `store.rs`）。

- [ ] **Step 3: 创建 types.rs，搬迁 14 个 pub struct**

读 `rust/crates/attune-core/src/store/mod.rs`，把以下行整段（含 `#[derive(...)]` 行）剪切到新 `rust/crates/attune-core/src/store/types.rs`：

| Struct | 在 mod.rs 中的位置（参考行号） |
|:---|:---|
| `RawItem` 及其 `impl RawItem` | ~L1090-1142 |
| `DecryptedItem` | ~L1143-1155 |
| `ItemSummary` | ~L1156-1164 |
| `StaleItemSummary` | ~L1165-1173 |
| `ItemStats` | ~L1174-1184 |
| `QueueTask` | ~L1185-1197 |
| `BoundDirRow` | ~L1198-1206 |
| `SearchHistoryRow` | ~L1207-1214 |
| `IndexedFileRow` | ~L1215-1223 |
| `ConversationSummary` | ~L1224-1231 |
| `Citation` | ~L1232-1238 |
| `ConvMessage` | ~L1239-1248 |
| `SkillSignal` | ~L1249-1256 |
| `Annotation` | ~L1387-1404 |
| `AnnotationInput` | ~L1405-1416 |

新 `types.rs` 顶部放：

```rust
//! Store 层共享类型（DTO / 行映射结构体）
//!
//! 抽出来集中管理 - 让 mod.rs 专注 schema + open/migrate，
//! impl Store 的具体方法散落在 items.rs / dirs.rs / ... 等子模块中。

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
```

把搬来的所有 struct 直接 paste 到下面（顺序按表格）。

注意：剪切时**保留 derive 注解**（`#[derive(Debug, Clone, Serialize, Deserialize)]` 等）。如果 struct 有相邻的辅助 impl 块（如 `impl RawItem { ... }`），把整个 impl 块也搬过来。

- [ ] **Step 4: mod.rs 顶部 declare types submodule + glob re-export**

`store/mod.rs` 顶部（在 `use rusqlite ...` 之前）加：

```rust
mod types;
pub use types::*;
```

这样所有原来 `attune_core::store::RawItem` 之类调用方仍能访问（API 兼容）。

- [ ] **Step 5: 跑测试验证拆迁不破**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo build --release --workspace 2>&1 | tail -5
echo '---'
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：build OK，377 passed。

如果 build fail（可能某 struct 用了 mod.rs private const）：把缺失的 const 改 `pub(super) const ...` 或挪到 types.rs 一起 export。

- [ ] **Step 6: 看 mod.rs 现在的行数（应缩一半左右）**

```bash
wc -l rust/crates/attune-core/src/store/mod.rs rust/crates/attune-core/src/store/types.rs
```

预期：mod.rs ~1500 行（原 2403 - 搬走 ~900）；types.rs ~900 行。

- [ ] **Step 7: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add rust/crates/attune-core/src/store/ && \
git commit -m "refactor(store): split out types.rs from store.rs

Move 14 shared DTO/struct types (RawItem, ItemSummary, QueueTask, ...)
to store/types.rs. mod.rs glob re-exports them for API compat.
Tests: 377 passed (no behavior change)."
```

---

### Task 2: 拆 items / dirs / queue 三个高频子模块

把 `impl Store { ... }` 中关于 items / bound_dirs+indexed_files / embed_queue 的方法分别搬到独立文件。

**Files:**
- Create: `rust/crates/attune-core/src/store/items.rs`
- Create: `rust/crates/attune-core/src/store/dirs.rs`
- Create: `rust/crates/attune-core/src/store/queue.rs`
- Modify: `rust/crates/attune-core/src/store/mod.rs`（剪走对应方法）

- [ ] **Step 1: 在 mod.rs 找出 impl Store 块的方法分布**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
grep -nE '^\s*(pub )?(async )?fn ' rust/crates/attune-core/src/store/mod.rs | head -60
```

按 fn 名前缀分组：
- `items_*` / `insert_item` / `update_item` / `get_item` / `delete_item` / `list_*` 涉及 items 表 → items.rs
- `bind_directory` / `unbind_directory` / `list_directories` / `upsert_indexed_file` / `get_indexed_file` → dirs.rs
- `enqueue_*` / `dequeue_*` / `mark_done` / `mark_failed` / `pending_count` 涉及 embed_queue → queue.rs

记录每个 fn 名 + 行号到本地 scratchpad，便于 Step 3 精确剪切。

- [ ] **Step 2: 创建 items.rs 骨架**

`rust/crates/attune-core/src/store/items.rs`:

```rust
//! Items 表 CRUD（attune-core 主资产 — 文件/笔记内容 + 加密）
//!
//! 所有方法属于 `impl Store`（inherent impl 跨文件分裂，rustc 自动合并）。

use crate::store::{types::*, Store};
use crate::error::Result;
use rusqlite::params;
use uuid::Uuid;
use chrono::Utc;

impl Store {
    // 方法将在 Step 3 从 mod.rs 剪过来
}
```

类似创建 `dirs.rs` 和 `queue.rs`（doc comment + use list 调整 + 空 impl Store 块）。

- [ ] **Step 3: 从 mod.rs 剪 fn 到对应 .rs**

逐 fn 操作。例如 `pub fn insert_item(...)` 整段（含 `pub fn` 行起到对应 `}` 止）剪到 `items.rs` 的 `impl Store { ... }` 内部。

每剪 1 个 fn 立即跑：

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo build --release -p attune-core 2>&1 | tail -3
```

预期：每次 build OK（fn 移动只影响 impl 解析，rustc 透明）。

如果某 fn 引用了 mod.rs 内的 private helper，需要把 helper 也跟去 / 或改 `pub(super)` 再 import。

- [ ] **Step 4: 在 mod.rs 顶部声明子模块**

`store/mod.rs` 顶部已有 `mod types; pub use types::*;`，扩展：

```rust
mod types;
mod items;
mod dirs;
mod queue;

pub use types::*;
```

注意：`mod items;` 等不需要 `pub` 因为 inherent impl 跨文件不需要；它们的内部不暴露 type 给 store 外部 — 所有公共 API 通过 mod.rs glob re-export 自然可见。

- [ ] **Step 5: 跑全测**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**377 passed, 0 failed**（行为完全不变，仅 fn 重排）。

- [ ] **Step 6: 看 mod.rs 行数（应再缩 30-40%）**

```bash
wc -l rust/crates/attune-core/src/store/*.rs
```

预期：mod.rs ~900 行；items.rs / dirs.rs / queue.rs 各 100-300 行。

- [ ] **Step 7: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add rust/crates/attune-core/src/store/ && \
git commit -m "refactor(store): split items/dirs/queue into dedicated submodules

Inherent impl Store split across files (rustc transparent).
mod.rs reduced from ~1500 to ~900 lines.
Tests: 377 passed (no behavior change)."
```

---

### Task 3: 拆 history / conversations / signals / chunk_summaries / annotations

把剩余 5 个主题的方法挪走，让 mod.rs 只剩 SCHEMA + open/migrate + 100 行以内。

**Files:**
- Create: `rust/crates/attune-core/src/store/history.rs`
- Create: `rust/crates/attune-core/src/store/conversations.rs`
- Create: `rust/crates/attune-core/src/store/signals.rs`
- Create: `rust/crates/attune-core/src/store/chunk_summaries.rs`
- Create: `rust/crates/attune-core/src/store/annotations.rs`
- Modify: `rust/crates/attune-core/src/store/mod.rs`

- [ ] **Step 1: 创建 5 个子模块骨架**

每个文件用同一模板（替换主题名）：

```rust
//! <主题> 表方法
//!
//! 属于 `impl Store` inherent impl（跨文件分裂）。

use crate::store::{types::*, Store};
use crate::error::Result;
use rusqlite::params;

impl Store {
    // ...
}
```

主题对应：

| File | 关键 fn 前缀 | 表 |
|:---|:---|:---|
| history.rs | `record_search` / `record_click` / `record_feedback` / `list_search_history` | search_history, click_events, feedback |
| conversations.rs | `conversation_*` / `session_*` / `append_message` / `list_conversations` | sessions, conversations, conversation_messages |
| signals.rs | `record_skill_signal` / `list_skill_signals` | skill_signals |
| chunk_summaries.rs | `upsert_chunk_summary` / `get_chunk_summary` / `delete_chunk_summary` | chunk_summaries |
| annotations.rs | `create_annotation` / `update_annotation` / `delete_annotation` / `list_annotations_for_item` | annotations |

- [ ] **Step 2: 逐 fn 剪到对应 .rs**

跟 Task 2 Step 3 同样的逐 fn 操作。每剪 1 个 cargo build 验证。

- [ ] **Step 3: mod.rs 加新子模块声明**

```rust
mod types;
mod items;
mod dirs;
mod queue;
mod history;
mod conversations;
mod signals;
mod chunk_summaries;
mod annotations;

pub use types::*;
```

- [ ] **Step 4: 跑全测**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**377 passed**。

- [ ] **Step 5: 看最终行数分布**

```bash
wc -l rust/crates/attune-core/src/store/*.rs
```

预期：
- mod.rs ≤ 200 行（仅 SCHEMA + open/migrate + mod 声明）
- 其他 9 个文件各 100-400 行

- [ ] **Step 6: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add rust/crates/attune-core/src/store/ && \
git commit -m "refactor(store): split history/conversations/signals/chunk_summaries/annotations

Final split: mod.rs reduced to schema + open/migrate (~200 lines).
9 themed submodules now hold impl Store methods.
R5 audit backlog (store.rs 2403 lines split) — DONE.
Tests: 377 passed."
```

---

### Task 4: Project / ProjectFile / ProjectTimeline schema + Repo

落 spec §2.1 的三表数据模型，加 `store/project.rs` 提供 CRUD。

**Files:**
- Create: `rust/crates/attune-core/src/store/project.rs`
- Modify: `rust/crates/attune-core/src/store/mod.rs`（SCHEMA const 加三表 + mod 声明）
- Modify: `rust/crates/attune-core/src/store/types.rs`（加 Project / ProjectFile / ProjectTimeline struct）

- [ ] **Step 1: 写失败测试**

`rust/crates/attune-core/src/store/project.rs` 顶部直接加（暂用 `mod tests` 内置；最终可独立 tests/ 但这先内联）：

```rust
//! Project / ProjectFile / ProjectTimeline 卷宗模型
//!
//! 实施 spec §2.1：通用 Project 层（kind = case/deal/topic/generic）。
//! 行业层（如 attune-law 的 Case 反序列化）通过 metadata_encrypted 持有 opaque blob。

use crate::store::{types::*, Store};
use crate::error::Result;
use rusqlite::params;
use uuid::Uuid;
use chrono::Utc;

impl Store {
    pub fn create_project(&self, title: &str, kind: ProjectKind) -> Result<Project> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let conn = self.conn.lock();
        let conn = conn.unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT INTO project (id, title, kind, metadata_encrypted, created_at, updated_at, archived) \
             VALUES (?1, ?2, ?3, NULL, ?4, ?4, 0)",
            params![&id, title, kind.as_str(), now],
        )?;
        Ok(Project {
            id,
            title: title.to_string(),
            kind,
            metadata_encrypted: None,
            created_at: now,
            updated_at: now,
            archived: false,
        })
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>> {
        let conn = self.conn.lock();
        let conn = conn.unwrap_or_else(|e| e.into_inner());
        conn.query_row(
            "SELECT id, title, kind, metadata_encrypted, created_at, updated_at, archived \
             FROM project WHERE id = ?1",
            params![id],
            |row| {
                Ok(Project {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    kind: ProjectKind::from_str(&row.get::<_, String>(2)?),
                    metadata_encrypted: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    archived: row.get::<_, i64>(6)? != 0,
                })
            },
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other.into()),
        })
    }

    pub fn list_projects(&self, include_archived: bool) -> Result<Vec<Project>> {
        let conn = self.conn.lock();
        let conn = conn.unwrap_or_else(|e| e.into_inner());
        let sql = if include_archived {
            "SELECT id, title, kind, metadata_encrypted, created_at, updated_at, archived \
             FROM project ORDER BY updated_at DESC"
        } else {
            "SELECT id, title, kind, metadata_encrypted, created_at, updated_at, archived \
             FROM project WHERE archived = 0 ORDER BY updated_at DESC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                title: row.get(1)?,
                kind: ProjectKind::from_str(&row.get::<_, String>(2)?),
                metadata_encrypted: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                archived: row.get::<_, i64>(6)? != 0,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.into())
    }

    pub fn add_file_to_project(
        &self,
        project_id: &str,
        file_id: &str,
        role: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        let conn = self.conn.lock();
        let conn = conn.unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT OR REPLACE INTO project_file (project_id, file_id, role, added_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![project_id, file_id, role, now],
        )?;
        Ok(())
    }

    pub fn list_files_for_project(&self, project_id: &str) -> Result<Vec<ProjectFile>> {
        let conn = self.conn.lock();
        let conn = conn.unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT project_id, file_id, role, added_at FROM project_file \
             WHERE project_id = ?1 ORDER BY added_at ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(ProjectFile {
                project_id: row.get(0)?,
                file_id: row.get(1)?,
                role: row.get(2)?,
                added_at: row.get(3)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.into())
    }

    pub fn append_timeline(
        &self,
        project_id: &str,
        event_type: &str,
        payload_encrypted: Option<&[u8]>,
    ) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        let conn = self.conn.lock();
        let conn = conn.unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT INTO project_timeline (project_id, ts, event_type, payload_encrypted) \
             VALUES (?1, ?2, ?3, ?4)",
            params![project_id, now, event_type, payload_encrypted],
        )?;
        Ok(())
    }

    pub fn list_timeline(&self, project_id: &str) -> Result<Vec<ProjectTimelineEntry>> {
        let conn = self.conn.lock();
        let conn = conn.unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT project_id, ts, event_type, payload_encrypted FROM project_timeline \
             WHERE project_id = ?1 ORDER BY ts ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(ProjectTimelineEntry {
                project_id: row.get(0)?,
                ts_ms: row.get(1)?,
                event_type: row.get(2)?,
                payload_encrypted: row.get(3)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get_project() {
        let store = Store::open_memory().expect("open memory store");
        let p = store
            .create_project("王某 vs 李某 民间借贷", ProjectKind::Case)
            .expect("create project");
        assert_eq!(p.title, "王某 vs 李某 民间借贷");
        assert_eq!(p.kind, ProjectKind::Case);

        let fetched = store.get_project(&p.id).expect("get").expect("some");
        assert_eq!(fetched.id, p.id);
    }

    #[test]
    fn list_projects_excludes_archived_by_default() {
        let store = Store::open_memory().expect("open");
        let p1 = store.create_project("Active", ProjectKind::Generic).expect("c1");
        let _p2 = store.create_project("Other", ProjectKind::Topic).expect("c2");

        let active = store.list_projects(false).expect("list");
        assert_eq!(active.len(), 2);

        // 归档 p1 的逻辑（暂时手动 SQL，正式 fn 在下个 sprint 加）
        store
            .conn
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .execute(
                "UPDATE project SET archived = 1 WHERE id = ?1",
                params![&p1.id],
            )
            .expect("archive update");

        let active = store.list_projects(false).expect("list active");
        assert_eq!(active.len(), 1);
        let all = store.list_projects(true).expect("list all");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn add_file_and_list() {
        let store = Store::open_memory().expect("open");
        let p = store
            .create_project("案件 A", ProjectKind::Case)
            .expect("c");
        store
            .add_file_to_project(&p.id, "file-uuid-001", "evidence")
            .expect("add");
        store
            .add_file_to_project(&p.id, "file-uuid-002", "pleading")
            .expect("add");

        let files = store.list_files_for_project(&p.id).expect("list");
        assert_eq!(files.len(), 2);
        let roles: Vec<_> = files.iter().map(|f| f.role.as_str()).collect();
        assert!(roles.contains(&"evidence"));
        assert!(roles.contains(&"pleading"));
    }

    #[test]
    fn timeline_append_and_list() {
        let store = Store::open_memory().expect("open");
        let p = store
            .create_project("案件 B", ProjectKind::Case)
            .expect("c");
        store
            .append_timeline(&p.id, "evidence_added", None)
            .expect("append 1");
        store
            .append_timeline(&p.id, "rpa_call", Some(b"opaque payload"))
            .expect("append 2");

        let timeline = store.list_timeline(&p.id).expect("list");
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].event_type, "evidence_added");
        assert_eq!(timeline[1].event_type, "rpa_call");
        assert_eq!(timeline[1].payload_encrypted.as_deref(), Some(b"opaque payload" as &[u8]));
    }
}
```

- [ ] **Step 2: 跑测试，验证 fail（type 未定义 / 表未建）**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo test --release -p attune-core --lib store::project 2>&1 | tail -30
```

预期：编译失败，提示 `Project` / `ProjectKind` / `ProjectFile` / `ProjectTimelineEntry` 未定义；或者 schema 没建表（runtime 失败）。

- [ ] **Step 3: 加类型到 types.rs**

`rust/crates/attune-core/src/store/types.rs` 末尾追加：

```rust
// ============================================================================
// Project / Case 卷宗（spec §2.1）
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Case,    // 律师案件
    Deal,    // 售前交易
    Topic,   // 学术研究主题
    Generic, // 通用
}

impl ProjectKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectKind::Case => "case",
            ProjectKind::Deal => "deal",
            ProjectKind::Topic => "topic",
            ProjectKind::Generic => "generic",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "case" => ProjectKind::Case,
            "deal" => ProjectKind::Deal,
            "topic" => ProjectKind::Topic,
            _ => ProjectKind::Generic,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub kind: ProjectKind,
    /// 行业层在此存 opaque blob（如 attune-law 的 case_no/parties/court 序列化 + AES-GCM 加密）。
    /// attune-core 不解析。
    pub metadata_encrypted: Option<Vec<u8>>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub project_id: String,
    pub file_id: String,
    /// 行业层语义（律师 = 'evidence' / 'pleading' / 'reference'；
    /// 售前 = 'rfp' / 'proposal' / 'reference'；空字符串表示未分类）
    pub role: String,
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTimelineEntry {
    pub project_id: String,
    /// 毫秒级时间戳（比一般 timestamp 精度高，便于排序时间相近事件）
    pub ts_ms: i64,
    /// `fact` / `evidence_added` / `rpa_call` / `ai_inference` 等
    pub event_type: String,
    pub payload_encrypted: Option<Vec<u8>>,
}
```

- [ ] **Step 4: 加表到 SCHEMA + 注册子模块**

打开 `rust/crates/attune-core/src/store/mod.rs`，找到 SCHEMA const（顶部 ~L11 起，含 14 张 `CREATE TABLE IF NOT EXISTS ...;`），在末尾追加：

```rust
// 简单办法：在 SCHEMA 字符串末尾追加三段 SQL
// 实际操作：找到 SCHEMA: &str 的结束 quote，在前面插入下面 3 段
```

具体定位：

```bash
grep -n 'CREATE TABLE IF NOT EXISTS annotations' rust/crates/attune-core/src/store/mod.rs
```

得到 annotations 表位置（~L158）。在 annotations CREATE TABLE 块**结束的 `;` 之后**、SCHEMA const `"` 闭合之前，加入：

```sql
-- Project / Case 卷宗（spec §2.1）
CREATE TABLE IF NOT EXISTS project (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'generic',
    metadata_encrypted BLOB,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS project_file (
    project_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT '',
    added_at INTEGER NOT NULL,
    PRIMARY KEY (project_id, file_id),
    FOREIGN KEY (project_id) REFERENCES project(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS project_timeline (
    project_id TEXT NOT NULL,
    ts INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    payload_encrypted BLOB,
    FOREIGN KEY (project_id) REFERENCES project(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_project_timeline_pid ON project_timeline(project_id, ts);
```

然后在 `mod.rs` 顶部的 `mod history; mod conversations; ...` 列表加：

```rust
mod project;
```

- [ ] **Step 5: 跑测试验证 pass**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo test --release -p attune-core --lib store::project 2>&1 | tail -10
```

预期：4 个测试 ok（create_and_get_project / list_projects_excludes_archived_by_default / add_file_and_list / timeline_append_and_list）。

- [ ] **Step 6: 跑全工作区测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**381 passed, 0 failed**（baseline 377 + 新增 4 个 project 测试）。

- [ ] **Step 7: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add rust/crates/attune-core/src/store/ && \
git commit -m "feat(project): Project/ProjectFile/ProjectTimeline schema + Repo

Spec §2.1: generic Project layer with kind enum (case/deal/topic/generic).
Industry-specific metadata (e.g. attune-law Case fields) lives in
metadata_encrypted as opaque AES-GCM blob — attune-core stays generic.

API: create_project / get_project / list_projects / add_file_to_project /
list_files_for_project / append_timeline / list_timeline.
Foreign keys + cascade delete; index on timeline (project_id, ts).
Tests: 381 passed (377 baseline + 4 project)."
```

---

### Task 5: 实体抽取 entities.rs（人名/金额/日期/案号/公司）

加 `crates/attune-core/src/entities.rs`，提供 `extract_entities(text) -> Vec<Entity>`。Sprint 1 Phase B 将基于此做"AI 推荐归类"的实体重叠度。

**Files:**
- Create: `rust/crates/attune-core/src/entities.rs`
- Create: `rust/crates/attune-core/tests/entities_test.rs`
- Modify: `rust/crates/attune-core/src/lib.rs`（`pub mod entities;`）
- Modify: `rust/crates/attune-core/Cargo.toml`（如 regex 不在依赖里则添加）

- [ ] **Step 1: 验证 regex 是否已是依赖**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
grep -E '^regex ' rust/crates/attune-core/Cargo.toml || echo 'regex not direct dep'
echo '---'
# regex 可能间接通过 tantivy / scraper 引入，但 attune-core 直接 use 需要直接声明
cargo tree -p attune-core --target all 2>&1 | grep '^├──\|^└──\|regex' | head -10
```

如果 attune-core/Cargo.toml 没直接 `regex = "1"`：

```toml
# 在 [dependencies] 末尾追加
regex = "1"
```

- [ ] **Step 2: 写失败测试 — 整合测试在 tests/entities_test.rs**

`rust/crates/attune-core/tests/entities_test.rs`:

```rust
//! 实体抽取端到端测试 — 固定语料（CLAUDE.md "零随机数据"原则）。

use attune_core::entities::{extract_entities, Entity, EntityKind};

#[test]
fn extract_money_simple() {
    let text = "借款金额为人民币壹拾万元整（¥100,000.00）";
    let ents = extract_entities(text);
    let monies: Vec<&Entity> = ents.iter().filter(|e| e.kind == EntityKind::Money).collect();
    assert!(!monies.is_empty(), "should detect money");
    // 至少一个金额匹配 ¥100,000.00 或 100000
    assert!(monies.iter().any(|e| e.value.contains("100,000") || e.value.contains("壹拾万")));
}

#[test]
fn extract_chinese_dates() {
    let text = "本合同于 2024 年 3 月 15 日签订，至 2026-01-31 到期。";
    let ents = extract_entities(text);
    let dates: Vec<&Entity> = ents.iter().filter(|e| e.kind == EntityKind::Date).collect();
    assert_eq!(dates.len(), 2, "应抽两个日期");
    assert!(dates.iter().any(|e| e.value.contains("2024") && e.value.contains("3") && e.value.contains("15")));
    assert!(dates.iter().any(|e| e.value.contains("2026-01-31")));
}

#[test]
fn extract_case_no() {
    let text = "本案案号 (2024)京02民终1234号，承办法官张三。";
    let ents = extract_entities(text);
    let cases: Vec<&Entity> = ents.iter().filter(|e| e.kind == EntityKind::CaseNo).collect();
    assert_eq!(cases.len(), 1);
    assert!(cases[0].value.contains("(2024)京02民终1234号"));
}

#[test]
fn extract_company_suffix() {
    let text = "甲方：北京云麓科技有限公司，乙方：上海某某有限责任公司。";
    let ents = extract_entities(text);
    let companies: Vec<&Entity> = ents.iter().filter(|e| e.kind == EntityKind::Company).collect();
    assert!(companies.len() >= 2, "至少应抽两个公司");
    assert!(companies.iter().any(|e| e.value.contains("北京云麓科技有限公司")));
}

#[test]
fn extract_chinese_person_heuristic() {
    let text = "甲方代表张三，乙方代表李四（法定代表人王小明）。";
    let ents = extract_entities(text);
    let persons: Vec<&Entity> = ents.iter().filter(|e| e.kind == EntityKind::Person).collect();
    // 启发式 — 至少应抽 1 个，理想 2-3 个
    assert!(!persons.is_empty(), "至少抽一个人名");
    let names: Vec<&str> = persons.iter().map(|e| e.value.as_str()).collect();
    // 简单姓 + 1-2 字名 — "张三" / "李四" / "王小明" 应该在
    assert!(names.iter().any(|n| n == &"张三" || n == &"李四" || n == &"王小明"));
}

#[test]
fn empty_text_returns_empty() {
    let ents = extract_entities("");
    assert!(ents.is_empty());
}

#[test]
fn no_entities_text_returns_empty() {
    let ents = extract_entities("the quick brown fox jumps over the lazy dog");
    let chinese_kinds: Vec<&Entity> = ents.iter()
        .filter(|e| matches!(e.kind, EntityKind::Person | EntityKind::Company | EntityKind::CaseNo))
        .collect();
    assert!(chinese_kinds.is_empty(), "纯英文不应误抽中文实体");
}
```

- [ ] **Step 3: 跑测试验证 fail**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo test --release -p attune-core --test entities_test 2>&1 | tail -10
```

预期：编译失败，`unresolved import attune_core::entities`。

- [ ] **Step 4: 实现 entities.rs**

`rust/crates/attune-core/src/entities.rs`:

```rust
//! 实体抽取：从中文 / 英文文本中抽出 Person / Money / Date / CaseNo / Company 等结构化实体。
//!
//! Sprint 1 Phase B 将使用这些实体计算 Project 推荐归类的"实体重叠度"
//! （spec §2.3 的 0.6 阈值）。
//!
//! 设计：纯函数 + 正则 + 中文启发式。无外部 API、无模型推理。

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Person,  // 中文 2-4 字人名
    Money,   // ¥xxx / 人民币 X 元 / 数额单位
    Date,    // YYYY-MM-DD / YYYY 年 M 月 D 日
    CaseNo,  // (YYYY)XX民终/民初/刑初 NNNN 号
    Company, // 含"有限公司"/"股份公司"/"研究所"等后缀
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub kind: EntityKind,
    pub value: String,
    /// 在原文中的字节起止（UTF-8 byte offset）— 上层可截取上下文
    pub byte_start: usize,
    pub byte_end: usize,
}

/// 从给定文本抽出所有实体。返回顺序：按出现位置升序。
///
/// 对中文 + 英文混合文本鲁棒。空文本返回空 Vec。
pub fn extract_entities(text: &str) -> Vec<Entity> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();

    extract_money(text, &mut out);
    extract_dates(text, &mut out);
    extract_case_no(text, &mut out);
    extract_company(text, &mut out);
    extract_chinese_person(text, &mut out);

    out.sort_by_key(|e| e.byte_start);
    out
}

fn push(out: &mut Vec<Entity>, kind: EntityKind, m: regex::Match<'_>) {
    out.push(Entity {
        kind,
        value: m.as_str().to_string(),
        byte_start: m.start(),
        byte_end: m.end(),
    });
}

fn extract_money(text: &str, out: &mut Vec<Entity>) {
    // ¥xxx / ¥xxx.xx / xxx 元 / 人民币 xxx 元 / 壹拾万元 等
    static MONEY_RE_PAT: &str = r"(?x)
        ( ¥ \s* \d[\d,]*(?:\.\d+)?           # ¥1,000.50
        | (?:人民币|RMB|CNY) \s* \d[\d,]*(?:\.\d+)? \s*(?:元|圆|万|亿)?  # 人民币 100 元
        | \d[\d,]*(?:\.\d+)? \s* (?:元|圆|万元|亿元|万|亿)              # 100,000 元
        | (?:壹|贰|叁|肆|伍|陆|柒|捌|玖|拾|佰|仟|万|亿|零|整|圆|元|角|分){2,}  # 壹拾万元整
        )
    ";
    let re = Regex::new(MONEY_RE_PAT).expect("money regex compile");
    for m in re.find_iter(text) {
        push(out, EntityKind::Money, m);
    }
}

fn extract_dates(text: &str, out: &mut Vec<Entity>) {
    // 2024-03-15 / 2024/3/15 / 2024 年 3 月 15 日
    static DATE_PATS: &[&str] = &[
        r"\b\d{4}[-/]\d{1,2}[-/]\d{1,2}\b",
        r"\d{4}\s*年\s*\d{1,2}\s*月\s*\d{1,2}\s*日",
        r"\d{4}\s*年\s*\d{1,2}\s*月",
    ];
    for pat in DATE_PATS {
        let re = Regex::new(pat).expect("date regex compile");
        for m in re.find_iter(text) {
            push(out, EntityKind::Date, m);
        }
    }
}

fn extract_case_no(text: &str, out: &mut Vec<Entity>) {
    // (2024)京02民终1234号 / (2023)沪01刑初567号 / (2024)粤民申000号 等
    // 格式：( YYYY ) <省市码 + 数字> <案件类型如 民终/民初/刑初/行初> <数字> 号
    static CASE_PAT: &str =
        r"\(\s*\d{4}\s*\)\s*[一-龥]{1,3}\d{0,3}[一-龥]{1,4}\d{1,6}\s*号";
    let re = Regex::new(CASE_PAT).expect("case_no regex compile");
    for m in re.find_iter(text) {
        push(out, EntityKind::CaseNo, m);
    }
}

fn extract_company(text: &str, out: &mut Vec<Entity>) {
    // 含特定后缀：有限公司 / 股份有限公司 / 有限责任公司 / 研究所 / 事务所 / 学校
    // 简单办法：匹配以这些后缀结尾、前面跟 1-15 个中文字符
    static COMPANY_PAT: &str = r"[一-龥（）()]{2,15}(?:有限公司|股份有限公司|有限责任公司|研究所|事务所|律师事务所|科技公司|分公司)";
    let re = Regex::new(COMPANY_PAT).expect("company regex compile");
    for m in re.find_iter(text) {
        push(out, EntityKind::Company, m);
    }
}

/// 中文人名启发式：百家姓单字姓 + 1-3 字名（拒绝公司/案号片段）
fn extract_chinese_person(text: &str, out: &mut Vec<Entity>) {
    // 仅最常见 50 个百家姓（足够 prove-of-concept；Sprint 1 Phase B 可换 jieba NER）
    let common_surnames =
        "李王张刘陈杨赵黄周吴徐孙朱马胡郭林何高梁郑罗宋谢唐韩曹许邓萧冯曾程蔡彭潘袁于董余苏叶吕魏蒋田杜丁沈姜范江";
    let surnames_chars: std::collections::HashSet<char> = common_surnames.chars().collect();
    // 简单滑窗：从每个 char 起试 1+1 / 1+2 / 1+3 字
    // 拒绝相邻命中（如"张三李四"应抽两个独立而非"张三李四"四字一个）
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let n = chars.len();
    let mut consumed = vec![false; n];

    for i in 0..n {
        if consumed[i] {
            continue;
        }
        let (byte_start, c0) = chars[i];
        if !surnames_chars.contains(&c0) {
            continue;
        }
        // 后面 1-3 字也是中文（且不是数字、标点、ASCII）
        let mut name_end_idx = i;
        for j in 1..=3 {
            if i + j >= n {
                break;
            }
            let (_, cj) = chars[i + j];
            if is_chinese_name_char(cj) {
                name_end_idx = i + j;
            } else {
                break;
            }
        }
        if name_end_idx == i {
            continue; // 单字姓没有名 — 跳过
        }
        let byte_end = chars[name_end_idx].0 + chars[name_end_idx].1.len_utf8();
        let value: String = chars[i..=name_end_idx].iter().map(|(_, c)| *c).collect();
        // 拒绝结尾在公司/职务字（"张总" / "李部长" 不算人名）
        if value.ends_with('总') || value.ends_with('司') || value.ends_with('厂') {
            continue;
        }
        out.push(Entity {
            kind: EntityKind::Person,
            value,
            byte_start,
            byte_end,
        });
        for k in i..=name_end_idx {
            consumed[k] = true;
        }
    }
}

fn is_chinese_name_char(c: char) -> bool {
    // CJK 主要范围
    let code = c as u32;
    (0x4E00..=0x9FFF).contains(&code)
        || (0x3400..=0x4DBF).contains(&code) // CJK Extension A
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn money_basic() {
        let v = extract_entities("¥1,000");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, EntityKind::Money);
    }

    #[test]
    fn date_basic() {
        let v = extract_entities("2024-03-15");
        assert_eq!(v[0].kind, EntityKind::Date);
        assert_eq!(v[0].value, "2024-03-15");
    }

    #[test]
    fn case_no_basic() {
        let v = extract_entities("(2024)京02民终1234号");
        assert_eq!(v[0].kind, EntityKind::CaseNo);
    }

    #[test]
    fn ordering_by_position() {
        let text = "2024-03-15 张三借款 ¥10,000";
        let v = extract_entities(text);
        // 顺序：date → person → money（按 byte_start 升序）
        let kinds: Vec<EntityKind> = v.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, vec![EntityKind::Date, EntityKind::Person, EntityKind::Money]);
    }
}
```

- [ ] **Step 5: lib.rs 注册新 mod**

`rust/crates/attune-core/src/lib.rs` 顶部已有的 `pub mod ...;` 列表中加：

```rust
pub mod entities;
```

具体位置：找 `pub mod taxonomy;` 之类的同级位置插入（按字母序大致即可）。

- [ ] **Step 6: 跑 entities 单元 + 集成测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
cargo test --release -p attune-core entities 2>&1 | tail -20
```

预期：所有测试 pass（unit_tests 4 个 + entities_test.rs 7 个 = 11 个）。

如某 test fail（启发式不够准确）：
- 如果是 `extract_chinese_person_heuristic` — 调整百家姓表 / 拒绝词
- 如果是 `extract_company_suffix` — 调整 COMPANY_PAT 后缀列表
- 如果是 `extract_money_simple` — 调整 MONEY_RE_PAT
- 不要为了让测试过而**改测试期望**（测试期望来自 spec / 真实需求）— 改 impl

- [ ] **Step 7: 跑全工作区测试**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project/rust && \
timeout 240 cargo test --release --workspace -- --test-threads=2 2>&1 | grep -E '^test result' | awk '{passed+=$4; failed+=$6} END {print passed " passed, " failed " failed"}'
```

预期：**392 passed**（baseline 377 + 4 project + 11 entities = 392）。

- [ ] **Step 8: Commit**

```bash
cd /data/company/project/attune/.worktrees/sprint-1-project && \
git add rust/crates/attune-core/src/entities.rs \
        rust/crates/attune-core/src/lib.rs \
        rust/crates/attune-core/tests/entities_test.rs \
        rust/crates/attune-core/Cargo.toml && \
git commit -m "feat(entities): extract Person/Money/Date/CaseNo/Company entities

Pure-function regex + heuristic extractor. No external API, no model.
Sprint 1 Phase B will use this for Project recommendation overlap-score
(spec §2.3, 0.6 threshold).

Tests: 11 entity tests + 392 workspace passed."
```

---

## Self-Review Notes

**Spec coverage:**
- ✅ §2.1 Project 通用层 → Task 4（schema + Repo + 4 tests）
- ✅ §2.3 实体重叠度计算前置（实体抽取） → Task 5
- ⏭ §2.2 Case 行业层（attune-law plugin 反序列化 metadata_encrypted） → Phase B
- ⏭ §2.3 AI 推荐归类触发逻辑（基于实体重叠） → Phase B
- ⏭ §3 AI 层（Intent Router / 跨证据链 workflow） → Phase C
- ⏭ R5 backlog（store.rs 拆分） → Tasks 1-3 ✅ 落地

**Placeholder scan:** 完整代码 + 完整命令 + 完整预期，无 TBD/TODO 留给 implementer 猜（regex pattern 直接给完整字符串）。

**Type consistency:**
- `Project { id, title, kind, metadata_encrypted, created_at, updated_at, archived }` — Task 4 定义后 Sprint 1 Phase B 沿用
- `ProjectKind::{Case, Deal, Topic, Generic}` + `as_str() / from_str()` — 跨文件调用一致
- `Entity { kind, value, byte_start, byte_end }` + `EntityKind::{Person, Money, Date, CaseNo, Company}` — 跨 entity_test.rs / unit_tests / 未来 Phase B 推荐归类一致
- `extract_entities(&str) -> Vec<Entity>` — 顶级公开 API，签名跨文件稳定

---

## 完成 Phase A 标志

5 个 Task 全部 checkbox 勾上，且：
- [ ] `wc -l rust/crates/attune-core/src/store/mod.rs` ≤ 250 行（schema + open + migrate + mod 声明）
- [ ] `wc -l rust/crates/attune-core/src/store/types.rs` 含 14 + 3 = 17 个 struct
- [ ] `cargo test --workspace`: **392 passed, 0 failed**（377 baseline + 4 project + 11 entities）
- [ ] `attune_core::store::Project` / `attune_core::store::ProjectKind` / `attune_core::entities::extract_entities` 公开 API 可用
- [ ] R5 backlog（store.rs 拆 13 子模块）落地（实际拆 10：mod / types / items / dirs / queue / history / conversations / signals / chunk_summaries / annotations / project — Phase A 11 个文件 = 10 子模块 + project，足够减负）

完成后 Phase B 将基于这些底座做：
- AI 推荐归类（基于 entities 重叠度 + chat 关键词触发）
- REST API `/api/v1/projects/*`（routes/projects.rs）
- WebSocket 推送推荐到前端
- 跨证据链 workflow 引擎
- 前端 Project tab + attune-law Case 渲染层
