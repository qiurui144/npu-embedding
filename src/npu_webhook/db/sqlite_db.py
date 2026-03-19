"""SQLite 数据库管理 + schema 初始化"""

import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path
from uuid import uuid4

SCHEMA_SQL = """
-- 知识条目
CREATE TABLE IF NOT EXISTS knowledge_items (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    url         TEXT,
    source_type TEXT NOT NULL DEFAULT 'webpage',
    domain      TEXT,
    tags        TEXT DEFAULT '[]',
    metadata    TEXT DEFAULT '{}',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    is_deleted  INTEGER NOT NULL DEFAULT 0
);

-- FTS5 全文索引（独立表，通过 item_id 关联）
CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(
    item_id UNINDEXED, title, content
);

-- Embedding 任务队列
CREATE TABLE IF NOT EXISTS embedding_queue (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id     TEXT NOT NULL REFERENCES knowledge_items(id),
    chunk_index INTEGER NOT NULL DEFAULT 0,
    chunk_text  TEXT NOT NULL DEFAULT '',
    priority    INTEGER NOT NULL DEFAULT 1,
    status      TEXT NOT NULL DEFAULT 'pending',
    attempts    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_eq_status_priority
    ON embedding_queue(status, priority, created_at);

-- 绑定的本地目录
CREATE TABLE IF NOT EXISTS bound_directories (
    id          TEXT PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    recursive   INTEGER NOT NULL DEFAULT 1,
    file_types  TEXT DEFAULT '["md","txt","pdf","docx","py","js"]',
    is_active   INTEGER NOT NULL DEFAULT 1,
    last_scan   TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 文件索引记录（用于增量更新）
CREATE TABLE IF NOT EXISTS indexed_files (
    id          TEXT PRIMARY KEY,
    dir_id      TEXT NOT NULL,
    path        TEXT NOT NULL UNIQUE,
    file_hash   TEXT NOT NULL,
    item_id     TEXT,
    indexed_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 技能
CREATE TABLE IF NOT EXISTS skills (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT,
    template    TEXT NOT NULL,
    match_pattern TEXT,
    extract_rule TEXT DEFAULT '{}',
    is_enabled  INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 注入反馈追踪
CREATE TABLE IF NOT EXISTS injection_feedback (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id     TEXT NOT NULL REFERENCES knowledge_items(id),
    query       TEXT NOT NULL,
    was_useful  INTEGER,  -- 1=有用, 0=无用, NULL=未反馈
    injected_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_if_item ON injection_feedback(item_id);

-- 系统配置 KV
CREATE TABLE IF NOT EXISTS app_config (
    key TEXT PRIMARY KEY, value TEXT NOT NULL
);

-- 优化历史记录
CREATE TABLE IF NOT EXISTS optimization_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    category    TEXT NOT NULL,
    action      TEXT NOT NULL,
    before_metrics TEXT DEFAULT '{}',
    after_metrics  TEXT DEFAULT '{}',
    improvement TEXT,
    version     TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
"""


def _now_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")


class SQLiteDB:
    """同步 SQLite 数据库管理"""

    def __init__(self, db_path: str | Path) -> None:
        self.db_path = str(db_path)
        self.conn = sqlite3.connect(self.db_path, check_same_thread=False)
        self.conn.row_factory = sqlite3.Row
        self.conn.execute("PRAGMA journal_mode=WAL")
        self.conn.execute("PRAGMA foreign_keys=ON")
        self._init_schema()

    def _init_schema(self) -> None:
        self.conn.executescript(SCHEMA_SQL)
        # 增量 schema 迁移（兼容旧数据库）
        for col, default in [
            ("quality_score", "1.0"),
            ("last_used_at", "NULL"),
            ("use_count", "0"),
        ]:
            try:
                self.conn.execute(f"ALTER TABLE knowledge_items ADD COLUMN {col} REAL DEFAULT {default}")
            except sqlite3.OperationalError:
                pass  # 列已存在
        self.conn.commit()

    def close(self) -> None:
        self.conn.close()

    # === knowledge_items ===

    def insert_item(
        self,
        title: str,
        content: str,
        source_type: str = "webpage",
        url: str | None = None,
        domain: str | None = None,
        tags: list[str] | None = None,
        metadata: dict | None = None,
        item_id: str | None = None,
    ) -> str:
        item_id = item_id or uuid4().hex
        now = _now_iso()
        self.conn.execute(
            """INSERT INTO knowledge_items
               (id, title, content, url, source_type, domain, tags, metadata, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            (
                item_id,
                title,
                content,
                url,
                source_type,
                domain,
                json.dumps(tags or [], ensure_ascii=False),
                json.dumps(metadata or {}, ensure_ascii=False),
                now,
                now,
            ),
        )
        # 同步 FTS5
        self.conn.execute(
            "INSERT INTO knowledge_fts (item_id, title, content) VALUES (?, ?, ?)",
            (item_id, title, content),
        )
        self.conn.commit()
        return item_id

    def get_item(self, item_id: str) -> dict | None:
        row = self.conn.execute(
            "SELECT * FROM knowledge_items WHERE id = ? AND is_deleted = 0", (item_id,)
        ).fetchone()
        return dict(row) if row else None

    def list_items(
        self,
        offset: int = 0,
        limit: int = 20,
        source_type: str | None = None,
    ) -> list[dict]:
        sql = "SELECT * FROM knowledge_items WHERE is_deleted = 0"
        params: list = []
        if source_type:
            sql += " AND source_type = ?"
            params.append(source_type)
        sql += " ORDER BY created_at DESC LIMIT ? OFFSET ?"
        params.extend([limit, offset])
        rows = self.conn.execute(sql, params).fetchall()
        return [dict(r) for r in rows]

    def update_item(self, item_id: str, **kwargs: str | list | dict) -> bool:
        sets = []
        params: list = []
        for k, v in kwargs.items():
            if k in ("title", "content", "url", "domain", "source_type"):
                sets.append(f"{k} = ?")
                params.append(v)
            elif k in ("tags", "metadata"):
                sets.append(f"{k} = ?")
                params.append(json.dumps(v, ensure_ascii=False))
        if not sets:
            return False
        sets.append("updated_at = ?")
        params.append(_now_iso())
        params.append(item_id)
        self.conn.execute(
            f"UPDATE knowledge_items SET {', '.join(sets)} WHERE id = ?", params
        )
        # 同步 FTS5（如果 title 或 content 变了就重建）
        if any(k in kwargs for k in ("title", "content")):
            item = self.get_item(item_id)
            if item:
                self.conn.execute("DELETE FROM knowledge_fts WHERE item_id = ?", (item_id,))
                self.conn.execute(
                    "INSERT INTO knowledge_fts (item_id, title, content) VALUES (?, ?, ?)",
                    (item_id, item["title"], item["content"]),
                )
        self.conn.commit()
        return True

    def delete_item(self, item_id: str) -> bool:
        self.conn.execute(
            "UPDATE knowledge_items SET is_deleted = 1, updated_at = ? WHERE id = ?",
            (_now_iso(), item_id),
        )
        self.conn.execute("DELETE FROM knowledge_fts WHERE item_id = ?", (item_id,))
        self.conn.commit()
        return True

    def count_items(self) -> int:
        row = self.conn.execute(
            "SELECT COUNT(*) FROM knowledge_items WHERE is_deleted = 0"
        ).fetchone()
        return row[0]

    # === FTS5 搜索 ===

    def fts_search(self, query: str, limit: int = 20) -> list[dict]:
        """全文搜索，返回匹配的知识条目

        FTS5 unicode61 tokenizer 将 CJK 字符逐字拆分。
        直接传入的 query 已由 fulltext.build_fts_query 预处理。
        如果 MATCH 失败则回退到 LIKE 搜索。
        """
        try:
            rows = self.conn.execute(
                """SELECT ki.*, fts.rank
                   FROM knowledge_fts fts
                   JOIN knowledge_items ki ON ki.id = fts.item_id
                   WHERE knowledge_fts MATCH ? AND ki.is_deleted = 0
                   ORDER BY fts.rank
                   LIMIT ?""",
                (query, limit),
            ).fetchall()
            if rows:
                return [dict(r) for r in rows]
        except Exception:
            pass

        # FTS 匹配失败时回退到 LIKE
        like_q = f"%{query}%"
        rows = self.conn.execute(
            """SELECT * FROM knowledge_items
               WHERE is_deleted = 0 AND (title LIKE ? OR content LIKE ?)
               ORDER BY created_at DESC LIMIT ?""",
            (like_q, like_q, limit),
        ).fetchall()
        return [dict(r) for r in rows]

    # === embedding_queue ===

    def enqueue_embedding(
        self,
        item_id: str,
        chunk_index: int = 0,
        chunk_text: str = "",
        priority: int = 1,
    ) -> int:
        cur = self.conn.execute(
            """INSERT INTO embedding_queue (item_id, chunk_index, chunk_text, priority)
               VALUES (?, ?, ?, ?)""",
            (item_id, chunk_index, chunk_text, priority),
        )
        self.conn.commit()
        return cur.lastrowid  # type: ignore[return-value]

    def dequeue_embeddings(self, batch_size: int = 16) -> list[dict]:
        """取出一批待处理的 embedding 任务"""
        rows = self.conn.execute(
            """SELECT * FROM embedding_queue
               WHERE status = 'pending'
               ORDER BY priority ASC, created_at ASC
               LIMIT ?""",
            (batch_size,),
        ).fetchall()
        if rows:
            ids = [r["id"] for r in rows]
            placeholders = ",".join("?" * len(ids))
            self.conn.execute(
                f"UPDATE embedding_queue SET status = 'processing' WHERE id IN ({placeholders})",
                ids,
            )
            self.conn.commit()
        return [dict(r) for r in rows]

    def complete_embedding(self, queue_id: int) -> None:
        self.conn.execute(
            "UPDATE embedding_queue SET status = 'done' WHERE id = ?", (queue_id,)
        )
        self.conn.commit()

    def fail_embedding(self, queue_id: int) -> None:
        self.conn.execute(
            "UPDATE embedding_queue SET status = 'failed', attempts = attempts + 1 WHERE id = ?",
            (queue_id,),
        )
        self.conn.commit()

    def pending_embedding_count(self) -> int:
        row = self.conn.execute(
            "SELECT COUNT(*) FROM embedding_queue WHERE status = 'pending'"
        ).fetchone()
        return row[0]

    # === bound_directories ===

    def bind_directory(
        self,
        path: str,
        recursive: bool = True,
        file_types: list[str] | None = None,
    ) -> str:
        dir_id = uuid4().hex
        self.conn.execute(
            """INSERT INTO bound_directories (id, path, recursive, file_types)
               VALUES (?, ?, ?, ?)""",
            (
                dir_id,
                path,
                int(recursive),
                json.dumps(file_types or ["md", "txt", "pdf", "docx", "py", "js"]),
            ),
        )
        self.conn.commit()
        return dir_id

    def unbind_directory(self, dir_id: str) -> bool:
        self.conn.execute("DELETE FROM bound_directories WHERE id = ?", (dir_id,))
        self.conn.commit()
        return True

    def list_directories(self) -> list[dict]:
        rows = self.conn.execute(
            "SELECT * FROM bound_directories WHERE is_active = 1"
        ).fetchall()
        return [dict(r) for r in rows]

    def update_directory_scan(self, dir_id: str) -> None:
        self.conn.execute(
            "UPDATE bound_directories SET last_scan = ? WHERE id = ?",
            (_now_iso(), dir_id),
        )
        self.conn.commit()

    # === indexed_files ===

    def get_indexed_file(self, path: str) -> dict | None:
        row = self.conn.execute(
            "SELECT * FROM indexed_files WHERE path = ?", (path,)
        ).fetchone()
        return dict(row) if row else None

    def upsert_indexed_file(
        self, dir_id: str, path: str, file_hash: str, item_id: str
    ) -> None:
        file_id = uuid4().hex
        self.conn.execute(
            """INSERT INTO indexed_files (id, dir_id, path, file_hash, item_id)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT(path) DO UPDATE SET
               file_hash = excluded.file_hash,
               item_id = excluded.item_id,
               indexed_at = datetime('now')""",
            (file_id, dir_id, path, file_hash, item_id),
        )
        self.conn.commit()

    # === app_config ===

    def get_config(self, key: str, default: str = "") -> str:
        row = self.conn.execute(
            "SELECT value FROM app_config WHERE key = ?", (key,)
        ).fetchone()
        return row[0] if row else default

    def set_config(self, key: str, value: str) -> None:
        self.conn.execute(
            "INSERT INTO app_config (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = ?",
            (key, value, value),
        )
        self.conn.commit()

    # === injection_feedback ===

    def record_injection(self, item_id: str, query: str) -> int:
        """记录一次注入事件，同时更新条目使用统计"""
        now = _now_iso()
        cur = self.conn.execute(
            "INSERT INTO injection_feedback (item_id, query, injected_at) VALUES (?, ?, ?)",
            (item_id, query, now),
        )
        self.conn.execute(
            "UPDATE knowledge_items SET use_count = use_count + 1, last_used_at = ? WHERE id = ?",
            (now, item_id),
        )
        self.conn.commit()
        return cur.lastrowid  # type: ignore[return-value]

    def update_feedback(self, feedback_id: int, was_useful: bool) -> None:
        """更新注入反馈（有用/无用）"""
        self.conn.execute(
            "UPDATE injection_feedback SET was_useful = ? WHERE id = ?",
            (1 if was_useful else 0, feedback_id),
        )
        # 更新条目质量分数
        row = self.conn.execute(
            "SELECT item_id FROM injection_feedback WHERE id = ?", (feedback_id,)
        ).fetchone()
        if row:
            self._recalc_quality(row[0])
        self.conn.commit()

    def _recalc_quality(self, item_id: str) -> None:
        """重新计算条目质量分数: useful_rate * log(use_count+1)"""
        import math
        row = self.conn.execute(
            """SELECT
                COUNT(*) as total,
                SUM(CASE WHEN was_useful = 1 THEN 1 ELSE 0 END) as useful,
                SUM(CASE WHEN was_useful = 0 THEN 1 ELSE 0 END) as useless
            FROM injection_feedback WHERE item_id = ?""",
            (item_id,),
        ).fetchone()
        total, useful, useless = row[0], row[1] or 0, row[2] or 0
        if total == 0:
            return
        rated = useful + useless
        if rated == 0:
            score = 1.0  # 无反馈时保持默认
        else:
            useful_rate = useful / rated
            score = useful_rate * math.log(rated + 1) / math.log(11)  # 归一化到 0~1
        self.conn.execute(
            "UPDATE knowledge_items SET quality_score = ? WHERE id = ?",
            (round(score, 3), item_id),
        )

    def list_stale_items(self, days: int = 30, min_use: int = 0, limit: int = 50) -> list[dict]:
        """查找过期/冷知识条目

        条件：
        - 超过 N 天未被使用
        - 使用次数低于阈值
        - 质量分数低于 0.3
        """
        cutoff = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")
        rows = self.conn.execute(
            """SELECT * FROM knowledge_items
               WHERE is_deleted = 0
               AND (
                   (last_used_at IS NULL AND julianday(?) - julianday(created_at) > ?)
                   OR (last_used_at IS NOT NULL AND julianday(?) - julianday(last_used_at) > ?)
                   OR quality_score < 0.3
               )
               AND use_count <= ?
               ORDER BY quality_score ASC, created_at ASC
               LIMIT ?""",
            (cutoff, days, cutoff, days, min_use, limit),
        ).fetchall()
        return [dict(r) for r in rows]

    def get_item_stats(self, item_id: str) -> dict:
        """获取条目的使用统计"""
        item = self.get_item(item_id)
        if not item:
            return {}
        feedback = self.conn.execute(
            """SELECT COUNT(*) as total,
                SUM(CASE WHEN was_useful = 1 THEN 1 ELSE 0 END) as useful,
                SUM(CASE WHEN was_useful = 0 THEN 1 ELSE 0 END) as useless
            FROM injection_feedback WHERE item_id = ?""",
            (item_id,),
        ).fetchone()
        return {
            "use_count": item.get("use_count", 0),
            "quality_score": item.get("quality_score", 1.0),
            "last_used_at": item.get("last_used_at"),
            "feedback_total": feedback[0],
            "feedback_useful": feedback[1] or 0,
            "feedback_useless": feedback[2] or 0,
        }
