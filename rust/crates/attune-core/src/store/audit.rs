//! v0.6 Phase A.5.3 — 出网审计日志（Outbound Audit Log）
//!
//! 设计目标：
//! - **每次云端调用都本地落 audit log**：合规员可导出 CSV 供等保 2.0 / GDPR 审计
//! - **0 用户原文落库**：只存 SHA256[:16] hash + redactions 统计 + meta（model/token/tier）
//! - **明文存储**：审计员需要直接读 timestamp/model/redaction_count，不加密
//! - **CSV 导出**：合规典型工作流是导出某时段范围给法务核
//!
//! ## API
//!
//! - `Store::record_outbound(&event)` — LLM provider hook 调用
//! - `Store::list_outbound_audit(from_ms, to_ms, limit)` — 时段查询
//! - `Store::export_outbound_csv(from_ms, to_ms, writer)` — CSV 流式导出
//!
//! ## 字段不变性
//!
//! `pre_redact_hash` 与 `post_redact_hash` 一定是 16 字符的 SHA256 头部 hex，
//! 让审计员对比"脱敏前后是否真的不同"（同 hash → 这次没脱敏到任何东西，可疑）。

use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::error::Result;
use crate::store::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditDirection {
    /// 用户消息 + 检索 chunks → 上行到 LLM
    Request,
    /// LLM 答案 → 本地接收（可选记录）
    Response,
}

impl AuditDirection {
    fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Response => "response",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "response" => Self::Response,
            _ => Self::Request,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PrivacyTier {
    /// 全本地 LLM，0 字节出网
    L0,
    /// 正则 + 词典脱敏后 → 云
    L1,
    /// LLM 语义脱敏后 → 云（高端硬件 + K3 一体机）
    L3,
}

impl PrivacyTier {
    fn as_str(self) -> &'static str {
        match self {
            Self::L0 => "L0",
            Self::L1 => "L1",
            Self::L3 => "L3",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "L0" => Self::L0,
            "L3" => Self::L3,
            _ => Self::L1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundAuditEvent {
    /// id 由 SQLite 自增分配，写入前为 0
    #[serde(default)]
    pub id: i64,
    pub ts_ms: i64,
    pub direction: AuditDirection,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub token_estimate: i64,
    pub privacy_tier: PrivacyTier,
    pub pre_redact_hash: String,
    pub post_redact_hash: String,
    /// JSON: {"PHONE":2,"EMAIL":1,"CASE_NO":3}
    #[serde(default = "empty_json_object")]
    pub redactions_json: String,
    #[serde(default)]
    pub session_id: String,
}

fn empty_json_object() -> String {
    "{}".to_string()
}

impl OutboundAuditEvent {
    /// 便利构造：自动用当前时间 ms。
    pub fn now(
        direction: AuditDirection,
        provider: impl Into<String>,
        model: impl Into<String>,
        privacy_tier: PrivacyTier,
        pre_redact_hash: impl Into<String>,
        post_redact_hash: impl Into<String>,
    ) -> Self {
        Self {
            id: 0,
            ts_ms: chrono::Utc::now().timestamp_millis(),
            direction,
            provider: provider.into(),
            model: model.into(),
            token_estimate: 0,
            privacy_tier,
            pre_redact_hash: pre_redact_hash.into(),
            post_redact_hash: post_redact_hash.into(),
            redactions_json: "{}".to_string(),
            session_id: String::new(),
        }
    }

    pub fn with_token_estimate(mut self, n: i64) -> Self {
        self.token_estimate = n;
        self
    }

    pub fn with_redactions_json(mut self, s: impl Into<String>) -> Self {
        self.redactions_json = s.into();
        self
    }

    pub fn with_session_id(mut self, s: impl Into<String>) -> Self {
        self.session_id = s.into();
        self
    }
}

/// 计算文本的 SHA256[:16] hex（用于 pre/post redact hash 字段）。
pub fn hash16(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}

impl Store {
    /// 记录一次出网事件。返回新 row 的 id。
    pub fn record_outbound(&self, event: &OutboundAuditEvent) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO outbound_audit
             (ts_ms, direction, provider, model, token_estimate, privacy_tier,
              pre_redact_hash, post_redact_hash, redactions_json, session_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                event.ts_ms,
                event.direction.as_str(),
                &event.provider,
                &event.model,
                event.token_estimate,
                event.privacy_tier.as_str(),
                &event.pre_redact_hash,
                &event.post_redact_hash,
                &event.redactions_json,
                &event.session_id,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 查询审计记录（按时间倒序，可选时段过滤）。
    pub fn list_outbound_audit(
        &self,
        from_ms: Option<i64>,
        to_ms: Option<i64>,
        limit: usize,
    ) -> Result<Vec<OutboundAuditEvent>> {
        let from = from_ms.unwrap_or(i64::MIN);
        let to = to_ms.unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare(
            "SELECT id, ts_ms, direction, provider, model, token_estimate, privacy_tier,
                    pre_redact_hash, post_redact_hash, redactions_json, session_id
             FROM outbound_audit
             WHERE ts_ms >= ?1 AND ts_ms <= ?2
             ORDER BY ts_ms DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![from, to, limit as i64], |row| {
            Ok(OutboundAuditEvent {
                id: row.get(0)?,
                ts_ms: row.get(1)?,
                direction: AuditDirection::from_str(&row.get::<_, String>(2)?),
                provider: row.get(3)?,
                model: row.get(4)?,
                token_estimate: row.get(5)?,
                privacy_tier: PrivacyTier::from_str(&row.get::<_, String>(6)?),
                pre_redact_hash: row.get(7)?,
                post_redact_hash: row.get(8)?,
                redactions_json: row.get(9)?,
                session_id: row.get(10)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// CSV 流式导出（合规员典型用法：导出某月给法务）。
    /// 列顺序：ts_iso, direction, provider, model, token_estimate, privacy_tier,
    ///         pre_redact_hash, post_redact_hash, redactions_json, session_id
    pub fn export_outbound_csv(
        &self,
        from_ms: Option<i64>,
        to_ms: Option<i64>,
        writer: &mut impl Write,
    ) -> Result<usize> {
        writeln!(
            writer,
            "ts_iso,direction,provider,model,token_estimate,privacy_tier,pre_hash,post_hash,redactions,session_id"
        )?;
        let events = self.list_outbound_audit(from_ms, to_ms, 1_000_000)?;
        let mut count = 0usize;
        for ev in &events {
            let ts_iso = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ev.ts_ms)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default();
            writeln!(
                writer,
                "{},{},{},{},{},{},{},{},{},{}",
                ts_iso,
                ev.direction.as_str(),
                csv_escape(&ev.provider),
                csv_escape(&ev.model),
                ev.token_estimate,
                ev.privacy_tier.as_str(),
                ev.pre_redact_hash,
                ev.post_redact_hash,
                csv_escape(&ev.redactions_json),
                csv_escape(&ev.session_id),
            )?;
            count += 1;
        }
        Ok(count)
    }
}

/// CSV 字段简单转义：含逗号/引号/换行的字段加双引号 + 内部引号双写。
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_store() -> Store {
        Store::open_memory().expect("open in-memory store")
    }

    #[test]
    fn hash16_is_16_hex_chars() {
        let h = hash16("hello");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn record_and_list_basic() {
        let s = open_store();
        let ev = OutboundAuditEvent::now(
            AuditDirection::Request,
            "anthropic",
            "claude-3-5-sonnet",
            PrivacyTier::L1,
            "abc123",
            "def456",
        )
        .with_token_estimate(1234)
        .with_redactions_json(r#"{"PHONE":2,"EMAIL":1}"#)
        .with_session_id("sess-001");

        let id = s.record_outbound(&ev).expect("insert");
        assert!(id > 0);

        let list = s.list_outbound_audit(None, None, 10).expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].provider, "anthropic");
        assert_eq!(list[0].model, "claude-3-5-sonnet");
        assert_eq!(list[0].token_estimate, 1234);
        assert_eq!(list[0].privacy_tier, PrivacyTier::L1);
        assert_eq!(list[0].redactions_json, r#"{"PHONE":2,"EMAIL":1}"#);
        assert_eq!(list[0].session_id, "sess-001");
    }

    #[test]
    fn list_orders_by_ts_desc() {
        let s = open_store();
        let mut ev_a = OutboundAuditEvent::now(
            AuditDirection::Request,
            "openai",
            "gpt-4",
            PrivacyTier::L1,
            "h1",
            "h2",
        );
        ev_a.ts_ms = 100;
        let mut ev_b = ev_a.clone();
        ev_b.ts_ms = 200;

        s.record_outbound(&ev_a).unwrap();
        s.record_outbound(&ev_b).unwrap();
        let list = s.list_outbound_audit(None, None, 10).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].ts_ms, 200);
        assert_eq!(list[1].ts_ms, 100);
    }

    #[test]
    fn list_filters_by_time_range() {
        let s = open_store();
        for ts in [100i64, 200, 300, 400] {
            let mut ev = OutboundAuditEvent::now(
                AuditDirection::Request,
                "p",
                "m",
                PrivacyTier::L1,
                "h",
                "h",
            );
            ev.ts_ms = ts;
            s.record_outbound(&ev).unwrap();
        }
        let in_range = s.list_outbound_audit(Some(150), Some(350), 10).unwrap();
        assert_eq!(in_range.len(), 2);
        assert!(in_range.iter().all(|e| (150..=350).contains(&e.ts_ms)));
    }

    #[test]
    fn csv_export_has_header_and_rows() {
        let s = open_store();
        s.record_outbound(
            &OutboundAuditEvent::now(
                AuditDirection::Request,
                "anthropic",
                "claude-3-5-sonnet",
                PrivacyTier::L1,
                "pre",
                "post",
            )
            .with_token_estimate(99)
            .with_redactions_json(r#"{"PHONE":1}"#),
        )
        .unwrap();

        let mut buf = Vec::new();
        let n = s.export_outbound_csv(None, None, &mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();

        assert_eq!(n, 1);
        assert!(text.starts_with("ts_iso,direction,provider"));
        assert!(text.contains("anthropic"));
        assert!(text.contains("claude-3-5-sonnet"));
        assert!(text.contains("99"));
        assert!(text.contains("L1"));
    }

    #[test]
    fn csv_escape_handles_commas_and_quotes() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape(r#"{"a":1}"#), r#""{""a"":1}""#);
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
    }

    #[test]
    fn privacy_tier_roundtrip() {
        for t in [PrivacyTier::L0, PrivacyTier::L1, PrivacyTier::L3] {
            assert_eq!(PrivacyTier::from_str(t.as_str()), t);
        }
    }

    #[test]
    fn direction_roundtrip() {
        for d in [AuditDirection::Request, AuditDirection::Response] {
            assert_eq!(AuditDirection::from_str(d.as_str()), d);
        }
    }
}
