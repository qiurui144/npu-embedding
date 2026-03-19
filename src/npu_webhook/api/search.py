"""GET /search + POST /search/relevant + POST /feedback - 搜索 + 注入反馈"""

from fastapi import APIRouter, HTTPException, Query
from pydantic import BaseModel

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
    """获取注入用相关知识 + 自动记录注入事件

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
        rerank=True,
    )

    # 记录注入事件（追踪哪些知识被注入过）
    feedback_ids = []
    if state.db and results:
        for r in results:
            try:
                fid = state.db.record_injection(r["id"], req.query)
                feedback_ids.append(fid)
            except Exception:
                pass

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
        feedback_ids=feedback_ids,
    )


# --- 注入反馈 ---


class FeedbackRequest(BaseModel):
    feedback_id: int
    was_useful: bool


@router.post("/feedback")
async def submit_feedback(req: FeedbackRequest) -> dict:
    """提交注入反馈（有用/无用），影响知识条目质量分数"""
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")
    state.db.update_feedback(req.feedback_id, req.was_useful)
    return {"status": "ok"}


# --- 过期/冷知识检测 ---


@router.get("/items/stale")
async def stale_items(
    days: int = Query(30, ge=1),
    limit: int = Query(50, ge=1, le=200),
) -> dict:
    """查找过期/低质量/冷知识条目"""
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")
    items = state.db.list_stale_items(days=days, limit=limit)
    return {
        "items": items,
        "total": len(items),
        "criteria": f"未使用 >{days} 天 或 quality_score < 0.3",
    }


@router.get("/items/{item_id}/stats")
async def item_stats(item_id: str) -> dict:
    """获取条目的使用统计和质量分数"""
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")
    stats = state.db.get_item_stats(item_id)
    if not stats:
        raise HTTPException(status_code=404, detail="Item not found")
    return stats
