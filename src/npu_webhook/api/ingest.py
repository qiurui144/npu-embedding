"""POST /ingest - 知识注入"""

from fastapi import APIRouter, HTTPException

from npu_webhook.app_state import state
from npu_webhook.models.schemas import IngestRequest, IngestResponse

router = APIRouter(prefix="/api/v1", tags=["ingest"])


@router.post("/ingest", response_model=IngestResponse)
async def ingest(req: IngestRequest) -> IngestResponse:
    """接收浏览器推送的内容（对话/网页/选中文本）"""
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")

    from npu_webhook.config import settings

    if len(req.content) < settings.ingest.min_content_length:
        raise HTTPException(
            status_code=400,
            detail=f"Content too short (min {settings.ingest.min_content_length} chars)",
        )

    if req.domain and req.domain in settings.ingest.excluded_domains:
        raise HTTPException(status_code=400, detail="Domain is excluded")

    # 存入 SQLite
    item_id = state.db.insert_item(
        title=req.title,
        content=req.content,
        source_type=req.source_type,
        url=req.url,
        domain=req.domain,
        tags=req.tags,
        metadata=req.metadata,
    )

    # 分块并投递 embedding 队列（P1 近实时）
    if state.chunker:
        chunks = state.chunker.chunk(req.content)
        for i, chunk_text in enumerate(chunks):
            state.db.enqueue_embedding(
                item_id=item_id,
                chunk_index=i,
                chunk_text=chunk_text,
                priority=1,
            )

    return IngestResponse(id=item_id)
