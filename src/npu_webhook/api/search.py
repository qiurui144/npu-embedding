"""GET /search + POST /search/relevant - 搜索"""

from fastapi import APIRouter, HTTPException, Query

from npu_webhook.app_state import state
from npu_webhook.models.schemas import RelevantRequest, SearchResponse, SearchResult

router = APIRouter(prefix="/api/v1", tags=["search"])


@router.get("/search", response_model=SearchResponse)
async def search(
    q: str = Query(..., min_length=1),
    top_k: int = Query(10, ge=1, le=100),
    source_types: str | None = Query(None),
) -> SearchResponse:
    """混合搜索（向量+全文, RRF融合）"""
    if not state.search_engine:
        raise HTTPException(status_code=503, detail="Search engine not initialized")

    types = source_types.split(",") if source_types else None
    results = state.search_engine.search(q, top_k=top_k, source_types=types)

    return SearchResponse(
        results=[
            SearchResult(
                id=r["id"],
                title=r.get("title", ""),
                content=r.get("content", "")[:500],
                score=r.get("score", 0),
                source_type=r.get("source_type", ""),
                url=r.get("url"),
                created_at=r.get("created_at"),
            )
            for r in results
        ],
        total=len(results),
    )


@router.post("/search/relevant", response_model=SearchResponse)
async def search_relevant(req: RelevantRequest) -> SearchResponse:
    """获取注入用相关知识（Content Script 调用）

    支持：上下文感知搜索 + 阈值过滤 + Reranker 精排
    """
    if not state.search_engine:
        raise HTTPException(status_code=503, detail="Search engine not initialized")

    results = state.search_engine.search(
        req.query,
        top_k=req.top_k,
        source_types=req.source_types,
        context=req.context,
        min_score=req.min_score,
        rerank=True,  # 注入场景始终启用 rerank
    )

    return SearchResponse(
        results=[
            SearchResult(
                id=r["id"],
                title=r.get("title", ""),
                content=r.get("content", ""),
                score=r.get("score", 0),
                source_type=r.get("source_type", ""),
                url=r.get("url"),
                created_at=r.get("created_at"),
            )
            for r in results
        ],
        total=len(results),
    )
