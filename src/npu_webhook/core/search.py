"""RRF 混合搜索引擎（向量 + 全文融合）"""

import logging

from npu_webhook.core.fulltext import build_fts_query
from npu_webhook.core.vectorstore import VectorStore
from npu_webhook.db.sqlite_db import SQLiteDB

logger = logging.getLogger(__name__)


class HybridSearchEngine:
    """Reciprocal Rank Fusion 混合搜索

    同时执行向量搜索和全文搜索，用 RRF 算法融合排序。
    """

    def __init__(
        self,
        db: SQLiteDB,
        vector_store: VectorStore,
        rrf_k: int = 60,
        vector_weight: float = 0.6,
        fulltext_weight: float = 0.4,
    ) -> None:
        self.db = db
        self.vector_store = vector_store
        self.rrf_k = rrf_k
        self.vector_weight = vector_weight
        self.fulltext_weight = fulltext_weight

    def search(
        self,
        query: str,
        top_k: int = 10,
        source_types: list[str] | None = None,
    ) -> list[dict]:
        """混合搜索：向量 + 全文，RRF 融合排序"""
        # 1. 向量搜索
        vector_results: list[dict] = []
        if self.vector_store.available:
            where = None
            if source_types:
                where = {"source_type": {"$in": source_types}}
            vector_results = self.vector_store.search(query, top_k=top_k * 2, where=where)

        # 2. 全文搜索
        fts_query = build_fts_query(query)
        fts_results = self.db.fts_search(fts_query, limit=top_k * 2)

        # 过滤 source_type
        if source_types:
            fts_results = [r for r in fts_results if r.get("source_type") in source_types]

        # 3. RRF 融合
        return self._rrf_merge(vector_results, fts_results, top_k)

    def _rrf_merge(
        self,
        vector_results: list[dict],
        fts_results: list[dict],
        top_k: int,
    ) -> list[dict]:
        """RRF 融合排序

        score(d) = w_vec * 1/(k+rank_vec) + w_fts * 1/(k+rank_fts)
        """
        scores: dict[str, float] = {}
        item_data: dict[str, dict] = {}

        # 向量搜索结果
        for rank, r in enumerate(vector_results):
            # 向量结果的 id 是 chunk id (item_id:chunk_index)
            item_id = r.get("metadata", {}).get("item_id", r["id"].split(":")[0])
            rrf_score = self.vector_weight / (self.rrf_k + rank + 1)
            scores[item_id] = scores.get(item_id, 0) + rrf_score
            if item_id not in item_data:
                item_data[item_id] = {
                    "id": item_id,
                    "content": r.get("document", ""),
                    "source_type": r.get("metadata", {}).get("source_type", ""),
                    "vector_score": r.get("score", 0),
                }

        # 全文搜索结果
        for rank, r in enumerate(fts_results):
            item_id = r["id"]
            rrf_score = self.fulltext_weight / (self.rrf_k + rank + 1)
            scores[item_id] = scores.get(item_id, 0) + rrf_score
            if item_id not in item_data:
                item_data[item_id] = {
                    "id": item_id,
                    "title": r.get("title", ""),
                    "content": r.get("content", ""),
                    "source_type": r.get("source_type", ""),
                    "url": r.get("url"),
                    "created_at": r.get("created_at"),
                }
            else:
                # 补充全文搜索的字段
                item_data[item_id].setdefault("title", r.get("title", ""))
                item_data[item_id].setdefault("url", r.get("url"))
                item_data[item_id].setdefault("created_at", r.get("created_at"))

        # 按 RRF 分数排序
        sorted_ids = sorted(scores.keys(), key=lambda x: scores[x], reverse=True)[:top_k]

        results = []
        for item_id in sorted_ids:
            data = item_data[item_id]
            # 尝试从数据库补全信息
            if "title" not in data or not data.get("title"):
                db_item = self.db.get_item(item_id)
                if db_item:
                    data.update({
                        "title": db_item["title"],
                        "url": db_item.get("url"),
                        "source_type": db_item["source_type"],
                        "created_at": db_item["created_at"],
                    })

            data["score"] = scores[item_id]
            results.append(data)

        return results
