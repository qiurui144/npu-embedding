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

    content_len = len(req.content)
    if content_len < settings.ingest.min_content_length:
        raise HTTPException(
            status_code=400,
            detail=f"Content too short (min {settings.ingest.min_content_length} chars)",
        )
    # 防止超大内容导致 embedding 内存溢出（上限 500KB 字符）
    MAX_CONTENT = 500_000
    if content_len > MAX_CONTENT:
        raise HTTPException(
            status_code=413,
            detail=f"Content too large (max {MAX_CONTENT} chars, got {content_len})",
        )

    if req.domain and req.domain in settings.ingest.excluded_domains:
        raise HTTPException(status_code=400, detail="Domain is excluded")

    # 文本级近重复检测（前 200 字符匹配）
    existing_id = state.db.find_near_duplicate(req.content, req.source_type)
    if existing_id:
        return IngestResponse(id=existing_id, duplicate=True)

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
        sections = state.chunker.extract_sections(req.content, source_type=req.source_type)

        # Level 1: 章节
        for section_idx, section_text in sections:
            if section_text.strip():
                state.db.enqueue_embedding(
                    item_id=item_id,
                    chunk_index=section_idx,
                    chunk_text=section_text,
                    priority=1,
                    level=1,
                    section_idx=section_idx,
                )

        # Level 2: 段落块
        chunk_counter = 0
        for section_idx, section_text in sections:
            chunks = state.chunker.chunk(section_text)
            for chunk_text in chunks:
                state.db.enqueue_embedding(
                    item_id=item_id,
                    chunk_index=chunk_counter,
                    chunk_text=chunk_text,
                    priority=1,
                    level=2,
                    section_idx=section_idx,
                )
                chunk_counter += 1

    return IngestResponse(id=item_id)
