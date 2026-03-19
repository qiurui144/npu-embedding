"""RRF 混合搜索引擎（向量 + 全文融合 + Reranker）"""

import logging

from npu_webhook.core.fulltext import build_fts_query
from npu_webhook.core.vectorstore import VectorStore
from npu_webhook.db.sqlite_db import SQLiteDB

logger = logging.getLogger(__name__)


class Reranker:
    """Ollama Reranker — 通过 embedding 余弦相似度做 cross-encoder 风格的精排"""

    def __init__(self, base_url: str = "http://localhost:11434") -> None:
        self.base_url = base_url.rstrip("/")
        self._available: bool | None = None

    @property
    def available(self) -> bool:
        if self._available is None:
            self._available = self._probe()
        return self._available

    def _probe(self) -> bool:
        import urllib.request
        try:
            with urllib.request.urlopen(f"{self.base_url}/api/tags", timeout=2):
                return True
        except Exception:
            return False

    def rerank(self, query: str, documents: list[dict], top_k: int = 3) -> list[dict]:
        """用 embedding 余弦相似度对 documents 重排序"""
        if not documents or not self.available:
            return documents[:top_k]

        import json
        import urllib.request

        texts = [query] + [d.get("content", "")[:512] for d in documents]
        try:
            data = json.dumps({"model": "bge-m3", "input": texts}).encode()
            req = urllib.request.Request(
                f"{self.base_url}/api/embed",
                data=data,
                headers={"Content-Type": "application/json"},
            )
            with urllib.request.urlopen(req, timeout=30) as resp:
                result = json.loads(resp.read())

            embeddings = result["embeddings"]
            query_emb = embeddings[0]

            # 余弦相似度
            import math
            def cosine_sim(a: list[float], b: list[float]) -> float:
                dot = sum(x * y for x, y in zip(a, b))
                na = math.sqrt(sum(x * x for x in a))
                nb = math.sqrt(sum(x * x for x in b))
                return dot / max(na * nb, 1e-12)

            scored = []
            for i, doc in enumerate(documents):
                sim = cosine_sim(query_emb, embeddings[i + 1])
                doc["rerank_score"] = sim
                scored.append(doc)

            scored.sort(key=lambda d: d["rerank_score"], reverse=True)
            return scored[:top_k]

        except Exception as e:
            logger.warning("Rerank failed, returning original order: %s", e)
            return documents[:top_k]


class HybridSearchEngine:
    """Reciprocal Rank Fusion 混合搜索 + 可选 Rerank

    同时执行向量搜索和全文搜索，用 RRF 算法融合排序，
    可选 Reranker 二次精排。
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
        self.reranker = Reranker()

    def search(
        self,
        query: str,
        top_k: int = 10,
        source_types: list[str] | None = None,
        context: list[str] | None = None,
        min_score: float = 0.0,
        rerank: bool = False,
    ) -> list[dict]:
        """混合搜索：向量 + 全文，RRF 融合排序

        Args:
            query: 搜索查询
            top_k: 返回数量
            source_types: 过滤来源类型
            context: 对话上下文（拼接到 query 增强搜索语义）
            min_score: 最低分数阈值
            rerank: 是否启用 reranker 二次排序
        """
        # 上下文感知：将最近对话拼接到查询中
        search_query = query
        if context:
            # 取最近 3 条上下文，拼接到 query 前面作为语义背景
            ctx_text = " ".join(context[-3:])
            search_query = f"{ctx_text} {query}"

        # 1. 向量搜索
        vector_results: list[dict] = []
        if self.vector_store.available:
            where = None
            if source_types:
                where = {"source_type": {"$in": source_types}}
            vector_results = self.vector_store.search(search_query, top_k=top_k * 2, where=where)

        # 2. 全文搜索
        fts_query = build_fts_query(query)  # 全文搜索用原始 query（分词更精确）
        fts_results = self.db.fts_search(fts_query, limit=top_k * 2)

        # 过滤 source_type
        if source_types:
            fts_results = [r for r in fts_results if r.get("source_type") in source_types]

        # 3. RRF 融合
        coarse_k = top_k * 3 if rerank else top_k
        merged = self._rrf_merge(vector_results, fts_results, coarse_k)

        # 4. 阈值过滤
        if min_score > 0:
            merged = [r for r in merged if r.get("score", 0) >= min_score]

        # 5. Reranker 精排
        if rerank and len(merged) > top_k:
            merged = self.reranker.rerank(query, merged, top_k=top_k)

        return merged[:top_k]

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

        # 从 DB 补全信息 + quality_score 加权
        for item_id in scores:
            db_item = self.db.get_item(item_id)
            if db_item:
                if item_id not in item_data:
                    item_data[item_id] = {"id": item_id}
                item_data[item_id].setdefault("title", db_item["title"])
                item_data[item_id].setdefault("url", db_item.get("url"))
                item_data[item_id].setdefault("source_type", db_item["source_type"])
                item_data[item_id].setdefault("created_at", db_item["created_at"])
                # quality_score 加权: 高质量条目获得最多 20% 的分数 bonus
                quality = db_item.get("quality_score") or 1.0
                scores[item_id] *= (0.8 + 0.2 * quality)

        sorted_ids = sorted(scores.keys(), key=lambda x: scores[x], reverse=True)[:top_k]

        results = []
        for item_id in sorted_ids:
            data = item_data.get(item_id, {"id": item_id})
            data["score"] = scores[item_id]
            results.append(data)

        return results
