# Session Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 npu-vault 的 Chat API 增加对话 Session 管理，支持多轮对话历史持久化、Session CRUD 接口，消息内容字段级 AES-256-GCM 加密。

**Architecture:** 在 `vault-core/src/store.rs` 中新增 `conversations` + `conversation_messages` 两张表及 CRUD 方法；在 `vault-server` 新增 `routes/chat_sessions.rs` 提供 Session 列表/详情/删除接口；修改现有 `routes/chat.rs` 接受可选 `session_id` 字段，自动创建或续接 Session 并持久化消息。

**Tech Stack:** rusqlite (现有)、aes-gcm + uuid (现有)、axum (现有)，无需新增依赖。

---

## File Map

| 文件 | 操作 | 内容 |
|------|------|------|
| `npu-vault/crates/vault-core/src/store.rs` | Modify | 新增 conversations/conversation_messages DDL + 5 个 CRUD 方法 |
| `npu-vault/crates/vault-server/src/routes/chat_sessions.rs` | Create | 3 个 Session 路由 handler |
| `npu-vault/crates/vault-server/src/routes/mod.rs` | Modify | 添加 `pub mod chat_sessions;` |
| `npu-vault/crates/vault-server/src/routes/chat.rs` | Modify | `ChatRequest` 新增 `session_id`，写入 Session |
| `npu-vault/crates/vault-server/src/main.rs` | Modify | 注册 chat_sessions 路由 |
| `npu-vault/crates/vault-server/tests/server_test.rs` | Modify | 新增 Session CRUD 集成测试 |

---

### Task 1: store.rs — 新增 Schema DDL

**Files:**
- Modify: `npu-vault/crates/vault-core/src/store.rs`

- [ ] **Step 1: 在 SCHEMA_SQL 常量末尾追加两张表的 DDL**

  找到 `SCHEMA_SQL` 常量中最后的 `"#;` 之前，插入以下 SQL（在 `feedback` 表定义之后）：

  ```rust
  // 在 SCHEMA_SQL 中，紧接 feedback 表的 INDEX 之后，"#; 之前插入：
  
  CREATE TABLE IF NOT EXISTS conversations (
      id          TEXT PRIMARY KEY,
      title       TEXT NOT NULL,
      created_at  TEXT NOT NULL DEFAULT (datetime('now')),
      updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
  );
  
  CREATE TABLE IF NOT EXISTS conversation_messages (
      id              TEXT PRIMARY KEY,
      conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
      role            TEXT NOT NULL CHECK(role IN ('user','assistant','system')),
      content         BLOB NOT NULL,
      citations       TEXT,
      created_at      TEXT NOT NULL DEFAULT (datetime('now'))
  );
  CREATE INDEX IF NOT EXISTS idx_conv_messages_conv_id
      ON conversation_messages(conversation_id);
  ```

  注意：`content` 字段用 `BLOB`（与 `items.content` 保持一致，存 AES-GCM 加密后的二进制）。`citations` 是 JSON 文本，无需加密。

- [ ] **Step 2: 编译验证 schema 语法**

  ```bash
  cd npu-vault && cargo build -p vault-core 2>&1 | head -30
  ```
  Expected: 编译通过，无 error

- [ ] **Step 3: Commit**

  ```bash
  git add npu-vault/crates/vault-core/src/store.rs
  git commit -m "feat(store): add conversations and conversation_messages schema"
  ```

---

### Task 2: store.rs — CRUD 方法（带单元测试）

**Files:**
- Modify: `npu-vault/crates/vault-core/src/store.rs`

- [ ] **Step 1: 先写失败测试**

  在 `store.rs` 文件末尾 `#[cfg(test)]` 模块中添加：

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
  
      fn make_store() -> Store {
          Store::open_memory().unwrap()
      }
  
      fn make_dek() -> Key32 {
          Key32([0u8; 32])
      }
  
      #[test]
      fn test_create_and_list_conversations() {
          let store = make_store();
          let dek = make_dek();
          let id1 = store.create_conversation(&dek, "第一个会话").unwrap();
          let id2 = store.create_conversation(&dek, "第二个会话").unwrap();
          let list = store.list_conversations(10, 0).unwrap();
          assert_eq!(list.len(), 2);
          // 按 updated_at DESC 排序，最新在前
          assert_eq!(list[0].id, id2);
          assert_eq!(list[1].id, id1);
      }
  
      #[test]
      fn test_append_and_get_messages() {
          let store = make_store();
          let dek = make_dek();
          let conv_id = store.create_conversation(&dek, "测试会话").unwrap();
          store.append_message(&dek, &conv_id, "user", "你好", &[]).unwrap();
          store.append_message(&dek, &conv_id, "assistant", "你好！有什么可以帮你的？", &[]).unwrap();
          let msgs = store.get_conversation_messages(&dek, &conv_id).unwrap();
          assert_eq!(msgs.len(), 2);
          assert_eq!(msgs[0].role, "user");
          assert_eq!(msgs[0].content, "你好");
          assert_eq!(msgs[1].role, "assistant");
      }
  
      #[test]
      fn test_delete_conversation_cascades() {
          let store = make_store();
          let dek = make_dek();
          let conv_id = store.create_conversation(&dek, "待删除").unwrap();
          store.append_message(&dek, &conv_id, "user", "消息内容", &[]).unwrap();
          store.delete_conversation(&conv_id).unwrap();
          let msgs = store.get_conversation_messages(&dek, &conv_id).unwrap();
          assert!(msgs.is_empty());
          let list = store.list_conversations(10, 0).unwrap();
          assert!(list.is_empty());
      }
  
      #[test]
      fn test_citations_json_roundtrip() {
          let store = make_store();
          let dek = make_dek();
          let conv_id = store.create_conversation(&dek, "带引用").unwrap();
          let citations = vec![
              Citation { item_id: "abc".to_string(), title: "文档A".to_string(), relevance: 0.9 },
          ];
          store.append_message(&dek, &conv_id, "assistant", "回答内容", &citations).unwrap();
          let msgs = store.get_conversation_messages(&dek, &conv_id).unwrap();
          assert_eq!(msgs[0].citations.len(), 1);
          assert_eq!(msgs[0].citations[0].item_id, "abc");
      }
  }
  ```

- [ ] **Step 2: 运行测试确认失败**

  ```bash
  cd npu-vault && cargo test -p vault-core store::tests 2>&1 | tail -20
  ```
  Expected: error — `create_conversation` / `list_conversations` 等方法未定义

- [ ] **Step 3: 添加数据结构和 CRUD 实现**

  在 `store.rs` 的 `use` 块末尾添加 `use serde::{Deserialize, Serialize};`（`serde` 已是现有依赖）。
  
  在 `impl Store` 块内追加以下方法（紧接现有 `list_items` / `delete_item` 等方法之后）：

  ```rust
  // ── Conversation Session CRUD ─────────────────────────────────────────────
  
  pub fn create_conversation(&self, _dek: &Key32, title: &str) -> Result<String> {
      let id = uuid::Uuid::new_v4().to_string();
      let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
      self.conn.execute(
          "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
          params![id, title, now],
      )?;
      Ok(id)
  }
  
  pub fn list_conversations(&self, limit: usize, offset: usize) -> Result<Vec<ConversationSummary>> {
      let mut stmt = self.conn.prepare(
          "SELECT id, title, created_at, updated_at FROM conversations
           ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
      )?;
      let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
          Ok(ConversationSummary {
              id: row.get(0)?,
              title: row.get(1)?,
              created_at: row.get(2)?,
              updated_at: row.get(3)?,
          })
      })?;
      rows.collect::<std::result::Result<Vec<_>, _>>().map_err(VaultError::Db)
  }
  
  pub fn get_conversation_messages(&self, dek: &Key32, conv_id: &str) -> Result<Vec<ConvMessage>> {
      let mut stmt = self.conn.prepare(
          "SELECT id, role, content, citations, created_at
           FROM conversation_messages
           WHERE conversation_id = ?1 ORDER BY created_at ASC",
      )?;
      let rows = stmt.query_map(params![conv_id], |row| {
          let enc_content: Vec<u8> = row.get(2)?;
          let citations_json: Option<String> = row.get(3)?;
          Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, enc_content, citations_json, row.get::<_, String>(4)?))
      })?;
      let mut results = Vec::new();
      for row in rows {
          let (id, role, enc_content, citations_json, created_at) = row.map_err(VaultError::Db)?;
          let content = crypto::decrypt_field(dek, &enc_content)?;
          let citations: Vec<Citation> = citations_json
              .and_then(|j| serde_json::from_str(&j).ok())
              .unwrap_or_default();
          results.push(ConvMessage { id, role, content, citations, created_at });
      }
      Ok(results)
  }
  
  pub fn append_message(
      &self, dek: &Key32, conv_id: &str, role: &str,
      content: &str, citations: &[Citation],
  ) -> Result<String> {
      let id = uuid::Uuid::new_v4().to_string();
      let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
      let enc = crypto::encrypt_field(dek, content)?;
      let citations_json = if citations.is_empty() {
          None
      } else {
          Some(serde_json::to_string(citations).unwrap_or_default())
      };
      self.conn.execute(
          "INSERT INTO conversation_messages (id, conversation_id, role, content, citations, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
          params![id, conv_id, role, enc, citations_json, now],
      )?;
      // Update conversation updated_at
      self.conn.execute(
          "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
          params![now, conv_id],
      )?;
      Ok(id)
  }
  
  pub fn delete_conversation(&self, conv_id: &str) -> Result<()> {
      // CASCADE 会自动删 conversation_messages
      self.conn.execute("DELETE FROM conversations WHERE id = ?1", params![conv_id])?;
      Ok(())
  }
  ```

  在 `store.rs` 同文件（`impl Store` 块**外面**）添加数据结构：

  ```rust
  #[derive(Debug, Clone, Serialize)]
  pub struct ConversationSummary {
      pub id: String,
      pub title: String,
      pub created_at: String,
      pub updated_at: String,
  }
  
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct Citation {
      pub item_id: String,
      pub title: String,
      pub relevance: f32,
  }
  
  #[derive(Debug, Clone, Serialize)]
  pub struct ConvMessage {
      pub id: String,
      pub role: String,
      pub content: String,
      pub citations: Vec<Citation>,
      pub created_at: String,
  }
  ```

  > **注意**：`crypto::encrypt_field` / `crypto::decrypt_field` 是已有函数（与 `insert_item` 使用同一套）。如果这两个函数是 pub(crate) 或通过 `Vault` 间接调用，需要查看 `crypto.rs` 确认调用方式。如果 `store.rs` 无法直接调用，改为在调用层（`chat.rs`）加密后传 `Vec<u8>` 进来，或把内容改存明文（此时 `content` 字段保持 `TEXT`）。优先保持与 `items.content` 相同的加密方式。

- [ ] **Step 4: 运行测试确认通过**

  ```bash
  cd npu-vault && cargo test -p vault-core store::tests 2>&1 | tail -20
  ```
  Expected: 4 tests passed

- [ ] **Step 5: Commit**

  ```bash
  git add npu-vault/crates/vault-core/src/store.rs
  git commit -m "feat(store): add conversation session CRUD methods"
  ```

---

### Task 3: chat_sessions.rs — Session CRUD 路由

**Files:**
- Create: `npu-vault/crates/vault-server/src/routes/chat_sessions.rs`
- Modify: `npu-vault/crates/vault-server/src/routes/mod.rs`

- [ ] **Step 1: 创建 `chat_sessions.rs`**

  ```rust
  // npu-vault/crates/vault-server/src/routes/chat_sessions.rs
  
  use axum::extract::{Path, Query, State};
  use axum::http::StatusCode;
  use axum::Json;
  use serde::Deserialize;
  
  use crate::state::SharedState;
  
  type ApiError = (StatusCode, Json<serde_json::Value>);
  
  fn err_500(msg: &str) -> ApiError {
      (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": msg})))
  }
  
  #[derive(Deserialize)]
  pub struct PaginationQuery {
      #[serde(default = "default_limit")]
      pub limit: usize,
      #[serde(default)]
      pub offset: usize,
  }
  
  fn default_limit() -> usize { 20 }
  
  /// GET /api/v1/chat/sessions?limit=20&offset=0
  pub async fn list_sessions(
      State(state): State<SharedState>,
      Query(params): Query<PaginationQuery>,
  ) -> Result<Json<serde_json::Value>, ApiError> {
      let vault = state.vault.lock().map_err(|_| err_500("vault lock poisoned"))?;
      let _ = vault.dek_db().map_err(|e| {
          (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
      })?;
      let sessions = vault.store()
          .list_conversations(params.limit, params.offset)
          .map_err(|e| err_500(&e.to_string()))?;
      Ok(Json(serde_json::json!({
          "sessions": sessions,
          "total": sessions.len(),
      })))
  }
  
  /// GET /api/v1/chat/sessions/:id
  pub async fn get_session(
      State(state): State<SharedState>,
      Path(session_id): Path<String>,
  ) -> Result<Json<serde_json::Value>, ApiError> {
      let vault = state.vault.lock().map_err(|_| err_500("vault lock poisoned"))?;
      let dek = vault.dek_db().map_err(|e| {
          (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
      })?;
      // 查 session 摘要（列表中取 id 匹配的那条）
      let all = vault.store()
          .list_conversations(1000, 0)
          .map_err(|e| err_500(&e.to_string()))?;
      let summary = all.into_iter().find(|s| s.id == session_id).ok_or_else(|| {
          (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "session not found"})))
      })?;
      let messages = vault.store()
          .get_conversation_messages(&dek, &session_id)
          .map_err(|e| err_500(&e.to_string()))?;
      Ok(Json(serde_json::json!({
          "session": summary,
          "messages": messages,
      })))
  }
  
  /// DELETE /api/v1/chat/sessions/:id
  pub async fn delete_session(
      State(state): State<SharedState>,
      Path(session_id): Path<String>,
  ) -> Result<StatusCode, ApiError> {
      let vault = state.vault.lock().map_err(|_| err_500("vault lock poisoned"))?;
      let _ = vault.dek_db().map_err(|e| {
          (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()})))
      })?;
      vault.store()
          .delete_conversation(&session_id)
          .map_err(|e| err_500(&e.to_string()))?;
      Ok(StatusCode::NO_CONTENT)
  }
  ```

- [ ] **Step 2: 注册模块**

  在 `routes/mod.rs` 末尾添加：

  ```rust
  pub mod chat_sessions;
  ```

- [ ] **Step 3: 编译确认**

  ```bash
  cd npu-vault && cargo build -p vault-server 2>&1 | head -30
  ```
  Expected: 编译通过（路由尚未注册到 router，但模块编译应通过）

- [ ] **Step 4: Commit**

  ```bash
  git add npu-vault/crates/vault-server/src/routes/chat_sessions.rs \
          npu-vault/crates/vault-server/src/routes/mod.rs
  git commit -m "feat(server): add chat session routes (list/get/delete)"
  ```

---

### Task 4: main.rs — 注册路由

**Files:**
- Modify: `npu-vault/crates/vault-server/src/main.rs`

- [ ] **Step 1: 查找路由注册位置**

  ```bash
  grep -n "chat" npu-vault/crates/vault-server/src/main.rs | head -20
  ```
  找到 `.route("/api/v1/chat", ...)` 所在行。

- [ ] **Step 2: 在现有 chat 路由后追加 Session 路由**

  找到类似下面的路由注册块：

  ```rust
  .route("/api/v1/chat", post(routes::chat::chat))
  .route("/api/v1/chat/history", get(routes::chat::chat_history))
  ```

  在其后添加：

  ```rust
  .route("/api/v1/chat/sessions",
      get(routes::chat_sessions::list_sessions))
  .route("/api/v1/chat/sessions/:id",
      get(routes::chat_sessions::get_session)
      .delete(routes::chat_sessions::delete_session))
  ```

- [ ] **Step 3: 编译确认**

  ```bash
  cd npu-vault && cargo build -p vault-server 2>&1 | head -30
  ```
  Expected: 编译通过

- [ ] **Step 4: Commit**

  ```bash
  git add npu-vault/crates/vault-server/src/main.rs
  git commit -m "feat(server): register chat session routes in router"
  ```

---

### Task 5: chat.rs — 集成 session_id 写入 Session

**Files:**
- Modify: `npu-vault/crates/vault-server/src/routes/chat.rs`

- [ ] **Step 1: 扩展 ChatRequest**

  将现有：

  ```rust
  #[derive(Deserialize)]
  pub struct ChatRequest {
      pub message: String,
      #[serde(default)]
      pub history: Vec<HistoryMessage>,
  }
  ```

  替换为：

  ```rust
  #[derive(Deserialize)]
  pub struct ChatRequest {
      pub message: String,
      #[serde(default)]
      pub history: Vec<HistoryMessage>,
      pub session_id: Option<String>,
  }
  ```

- [ ] **Step 2: 修复 500-char 截断 bug**

  找到 `chat` 函数内（line ~116）：

  ```rust
  "content": item.content.chars().take(500).collect::<String>(),
  ```

  替换为：

  ```rust
  "content": item.content,
  ```

  > 这修复了设计文档中提到的 `content.chars().take(500)` 截断 bug，chat 路由现在与 search_relevant 保持一致（截断由 `allocate_budget` 统一管理）。

- [ ] **Step 3: 替换 auto-save 逻辑，改为写 Session**

  找到现有 auto-save 块（约 line 175-183）：

  ```rust
  // 5. Auto-save conversation
  {
      let vault = state.vault.lock().unwrap();
      let title: String = body.message.chars().take(50).collect();
      let content = format!("用户: {}\n\n助手: {}", body.message, response);
      let _ = vault
          .store()
          .insert_item(&dek, &title, &content, None, "ai_chat", None, None);
  }
  ```

  替换为：

  ```rust
  // 5. Persist to conversation session
  let session_id = {
      let vault = state.vault.lock().unwrap();
      let title: String = body.message.chars().take(50).collect();
      // 取已有或新建
      let sid = match &body.session_id {
          Some(id) => id.clone(),
          None => vault.store().create_conversation(&dek, &title)
              .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string()),
      };
      // 构造引用列表
      let citations_for_session: Vec<vault_core::store::Citation> = knowledge
          .iter()
          .map(|k| vault_core::store::Citation {
              item_id: k.get("item_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
              title: k.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
              relevance: k.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
          })
          .collect();
      let _ = vault.store().append_message(&dek, &sid, "user", &body.message, &[]);
      let _ = vault.store().append_message(&dek, &sid, "assistant", &response, &citations_for_session);
      sid
  };
  ```

- [ ] **Step 4: 在返回体加上 session_id**

  找到最终 return：

  ```rust
  Ok(Json(serde_json::json!({
      "content": response,
      "citations": citations,
      "knowledge_count": knowledge.len(),
  })))
  ```

  替换为：

  ```rust
  Ok(Json(serde_json::json!({
      "content": response,
      "citations": citations,
      "knowledge_count": knowledge.len(),
      "session_id": session_id,
  })))
  ```

- [ ] **Step 5: 编译确认**

  ```bash
  cd npu-vault && cargo build -p vault-server 2>&1 | head -30
  ```
  Expected: 编译通过

- [ ] **Step 6: Commit**

  ```bash
  git add npu-vault/crates/vault-server/src/routes/chat.rs
  git commit -m "feat(chat): add session_id support, fix 500-char truncation bug"
  ```

---

### Task 6: 集成测试 + RELEASE.md 更新

**Files:**
- Modify: `npu-vault/crates/vault-server/tests/server_test.rs`（或新建 `tests/chat_session_test.rs`）
- Modify: `npu-vault/RELEASE.md`

- [ ] **Step 1: 查看现有集成测试结构**

  ```bash
  ls npu-vault/crates/vault-server/tests/
  ```

- [ ] **Step 2: 添加 Session API 集成测试**

  在测试文件末尾添加：

  ```rust
  #[cfg(test)]
  mod chat_session_tests {
      // 注意：这些测试用的是 vault-core 的 Store::open_memory() 直接测试
      // 集成测试不需要启动完整 HTTP server，直接测试 Store 方法
      use vault_core::store::Store;
  
      fn make_dek() -> vault_core::crypto::Key32 {
          vault_core::crypto::Key32([0u8; 32])
      }
  
      #[test]
      fn test_session_lifecycle() {
          let store = Store::open_memory().unwrap();
          let dek = make_dek();
  
          // 创建 session
          let sid = store.create_conversation(&dek, "专利检索会话").unwrap();
          assert!(!sid.is_empty());
  
          // 写入消息
          store.append_message(&dek, &sid, "user", "分析权利要求书", &[]).unwrap();
          store.append_message(&dek, &sid, "assistant", "权利要求书分析如下...", &[]).unwrap();
  
          // 读取消息
          let msgs = store.get_conversation_messages(&dek, &sid).unwrap();
          assert_eq!(msgs.len(), 2);
          assert_eq!(msgs[0].content, "分析权利要求书");
          assert_eq!(msgs[1].content, "权利要求书分析如下...");
  
          // 列出 sessions
          let list = store.list_conversations(10, 0).unwrap();
          assert_eq!(list.len(), 1);
          assert_eq!(list[0].id, sid);
  
          // 删除 session（级联删消息）
          store.delete_conversation(&sid).unwrap();
          let msgs_after = store.get_conversation_messages(&dek, &sid).unwrap();
          assert!(msgs_after.is_empty());
      }
  
      #[test]
      fn test_session_pagination() {
          let store = Store::open_memory().unwrap();
          let dek = make_dek();
          for i in 0..5 {
              store.create_conversation(&dek, &format!("会话{}", i)).unwrap();
          }
          let page1 = store.list_conversations(3, 0).unwrap();
          let page2 = store.list_conversations(3, 3).unwrap();
          assert_eq!(page1.len(), 3);
          assert_eq!(page2.len(), 2);
      }
  
      #[test]
      fn test_session_updated_at_changes_on_append() {
          let store = Store::open_memory().unwrap();
          let dek = make_dek();
          let sid = store.create_conversation(&dek, "测试").unwrap();
          let before = store.list_conversations(1, 0).unwrap()[0].updated_at.clone();
          std::thread::sleep(std::time::Duration::from_millis(1100));
          store.append_message(&dek, &sid, "user", "消息", &[]).unwrap();
          let after = store.list_conversations(1, 0).unwrap()[0].updated_at.clone();
          assert_ne!(before, after, "updated_at 应在 append_message 后更新");
      }
  }
  ```

- [ ] **Step 3: 运行测试**

  ```bash
  cd npu-vault && cargo test 2>&1 | tail -30
  ```
  Expected: 所有测试通过（包括新增的 session tests）

- [ ] **Step 4: 更新 RELEASE.md**

  在 `npu-vault/RELEASE.md` 的最新版本章节下追加：

  ```markdown
  ### Chat Session Management
  - POST /api/v1/chat 新增可选 `session_id` 字段，不传时自动创建新会话并返回 `session_id`
  - GET /api/v1/chat/sessions — 分页获取会话列表（按 updated_at DESC）
  - GET /api/v1/chat/sessions/:id — 获取会话详情 + 消息历史（内容字段级解密）
  - DELETE /api/v1/chat/sessions/:id — 删除会话及其消息（CASCADE）
  - 修复 chat.rs 中 500 字符截断 bug（内容不再被截断后注入 RAG 上下文）
  - 消息内容字段级 AES-256-GCM 加密存储，与 items 保持一致
  ```

- [ ] **Step 5: Commit**

  ```bash
  cd npu-vault && cargo test 2>&1 | grep -E "^test|FAILED|passed|failed"
  git add npu-vault/crates/vault-server/tests/ npu-vault/RELEASE.md
  git commit -m "test(server): add chat session integration tests; update RELEASE.md"
  ```

---

## Self-Review

### Spec coverage check

| Spec requirement | Task |
|---|---|
| conversations + conversation_messages 表 DDL | Task 1 |
| create_conversation / list_conversations / get_conversation_messages / append_message / delete_conversation | Task 2 |
| GET /chat/sessions, GET /chat/sessions/:id, DELETE /chat/sessions/:id | Task 3–4 |
| POST /chat 支持 session_id 可选，不传时自动创建并返回 | Task 5 |
| 消息内容 AES-256-GCM 加密 | Task 2 Step 3（encrypt_field） |
| citations JSON 存储 | Task 2 Step 3 |
| chat.rs 500 字符截断 bug 修复 | Task 5 Step 2 |
| 集成测试 | Task 6 |

### Type consistency check

- `Citation` 结构体在 `vault-core/store.rs` 定义，`chat.rs` 通过 `vault_core::store::Citation` 引用 ✓
- `ConversationSummary.updated_at` — Task 3 的 `list_sessions` 返回的 JSON 字段名与 spec 中 `updated_at` 一致 ✓
- `append_message` 返回 `Result<String>` (message id)，Task 5 用 `let _ =` 忽略 ✓

### Placeholder scan

无 TBD / TODO 占位符。Task 2 Step 3 中有一个条件注释（关于 `encrypt_field` 可见性），已提供备用方案。
