"""搜索引擎测试"""

import tempfile
from pathlib import Path

from npu_webhook.core.search import HybridSearchEngine
from npu_webhook.core.vectorstore import VectorStore
from npu_webhook.db.chroma_db import ChromaDB
from npu_webhook.db.sqlite_db import SQLiteDB


def test_hybrid_search_basic():
    """基础混合搜索：无 embedding 引擎，仅 FTS5"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db = SQLiteDB(Path(tmpdir) / "test.db")
        chroma = ChromaDB(Path(tmpdir) / "chroma")
        vs = VectorStore(chroma, engine=None)
        engine = HybridSearchEngine(db=db, vector_store=vs)

        # 插入数据
        db.insert_item(title="Python 教程", content="Python 是一种编程语言", source_type="note")
        db.insert_item(title="JavaScript 入门", content="JS 是前端开发的核心语言", source_type="note")

        # 搜索
        results = engine.search("Python 编程")
        assert isinstance(results, list)

        db.close()


def test_fts_search():
    """FTS5 全文搜索"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db = SQLiteDB(Path(tmpdir) / "test.db")

        db.insert_item(title="深度学习入门", content="深度学习是机器学习的一个分支", source_type="note")
        db.insert_item(title="Web 开发", content="前端和后端开发技术", source_type="note")

        results = db.fts_search("深度学习")
        assert len(results) >= 1
        assert "深度学习" in results[0]["title"]

        db.close()


def test_find_near_duplicate():
    """入库前近重复检测：前 200 字符相同视为重复"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db = SQLiteDB(Path(tmpdir) / "test.db")

        content = "这是一段很长的测试文本内容，用于验证近重复检测逻辑。" * 20
        db.insert_item(title="原始条目", content=content, source_type="ai_chat")

        # 相同前缀 → 返回已有 id
        assert db.find_near_duplicate(content, "ai_chat") is not None

        # 不同 source_type → 不视为重复
        assert db.find_near_duplicate(content, "note") is None

        # 完全不同内容 → 不重复
        assert db.find_near_duplicate("完全不同的内容，没有任何相似之处。" * 10, "ai_chat") is None

        db.close()


def test_get_items_batch():
    """批量获取条目：一次查询替代 N+1"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db = SQLiteDB(Path(tmpdir) / "test.db")

        ids = [
            db.insert_item(title=f"条目{i}", content=f"内容{i}" * 20, source_type="note")
            for i in range(5)
        ]

        results = db.get_items_batch(ids)
        assert len(results) == 5
        assert {r["id"] for r in results} == set(ids)
        assert db.get_items_batch([]) == []

        db.close()


def test_bulk_archive_stale():
    """批量软删除冷 ai_chat 条目"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db = SQLiteDB(Path(tmpdir) / "test.db")

        item_id = db.insert_item(
            title="旧对话", content="这是一段很老的对话内容。" * 10, source_type="ai_chat"
        )
        # 模拟 31 天前创建
        db.conn.execute(
            "UPDATE knowledge_items SET created_at = datetime('now', '-31 days') WHERE id = ?",
            (item_id,),
        )
        db.conn.commit()

        archived = db.bulk_archive_stale(
            quality_threshold=0.2, unused_days=60, chat_unused_days=30
        )
        assert item_id in archived
        assert db.get_item(item_id) is None  # 软删除后 get_item 返回 None

        db.close()


def test_chroma_delete_by_item_ids():
    """ChromaDB 按 item_id 元数据删除所有 chunk，防止孤立向量"""
    with tempfile.TemporaryDirectory() as tmpdir:
        chroma = ChromaDB(Path(tmpdir) / "chroma")

        dummy_emb = [0.1] * 10
        chroma.add("item1:0", dummy_emb, metadata={"item_id": "item1"}, document="chunk0")
        chroma.add("item1:1", dummy_emb, metadata={"item_id": "item1"}, document="chunk1")
        chroma.add("item2:0", dummy_emb, metadata={"item_id": "item2"}, document="other")

        assert chroma.count() == 3
        chroma.delete_by_item_ids(["item1"])
        assert chroma.count() == 1  # 只剩 item2:0
