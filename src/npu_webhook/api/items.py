"""CRUD /items - 知识条目管理"""

import json

from fastapi import APIRouter, HTTPException, Query

from npu_webhook.app_state import state
from npu_webhook.models.schemas import ItemListResponse, ItemUpdateRequest, KnowledgeItem

router = APIRouter(prefix="/api/v1", tags=["items"])


def _row_to_item(row: dict) -> KnowledgeItem:
    return KnowledgeItem(
        id=row["id"],
        title=row["title"],
        content=row["content"],
        url=row.get("url"),
        source_type=row["source_type"],
        domain=row.get("domain"),
        tags=json.loads(row.get("tags", "[]")),
        metadata=json.loads(row.get("metadata", "{}")),
        created_at=row["created_at"],
        updated_at=row["updated_at"],
    )


@router.get("/items", response_model=ItemListResponse)
async def list_items(
    offset: int = Query(0, ge=0),
    limit: int = Query(20, ge=1, le=100),
    source_type: str | None = Query(None),
) -> ItemListResponse:
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")
    rows = state.db.list_items(offset=offset, limit=limit, source_type=source_type)
    total = state.db.count_items()
    return ItemListResponse(
        items=[_row_to_item(r) for r in rows],
        total=total,
        offset=offset,
        limit=limit,
    )


@router.get("/items/{item_id}", response_model=KnowledgeItem)
async def get_item(item_id: str) -> KnowledgeItem:
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")
    row = state.db.get_item(item_id)
    if not row:
        raise HTTPException(status_code=404, detail="Item not found")
    return _row_to_item(row)


@router.patch("/items/{item_id}")
async def update_item(item_id: str, req: ItemUpdateRequest) -> dict:
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")
    if not state.db.get_item(item_id):
        raise HTTPException(status_code=404, detail="Item not found")

    kwargs = {}
    if req.title is not None:
        kwargs["title"] = req.title
    if req.tags is not None:
        kwargs["tags"] = req.tags
    if req.metadata is not None:
        kwargs["metadata"] = req.metadata

    if kwargs:
        state.db.update_item(item_id, **kwargs)
    return {"status": "ok"}


@router.delete("/items/{item_id}")
async def delete_item(item_id: str) -> dict:
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")
    if not state.db.get_item(item_id):
        raise HTTPException(status_code=404, detail="Item not found")
    state.db.delete_item(item_id)
    # 也从向量库中删除
    if state.vector_store:
        state.vector_store.delete([f"{item_id}:0"])
    return {"status": "ok"}
