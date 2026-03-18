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
