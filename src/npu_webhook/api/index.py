"""/index - 本地目录绑定"""

import json
import threading

from fastapi import APIRouter, HTTPException

from npu_webhook.app_state import state
from npu_webhook.models.schemas import BindDirectoryRequest

router = APIRouter(prefix="/api/v1", tags=["index"])


@router.post("/index/bind")
async def bind_directory(req: BindDirectoryRequest) -> dict:
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")

    from pathlib import Path

    if not Path(req.path).is_dir():
        raise HTTPException(status_code=400, detail="Directory does not exist")

    dir_id = state.db.bind_directory(
        path=req.path,
        recursive=req.recursive,
        file_types=req.file_types,
    )

    # 添加 watcher
    if state.watcher:
        state.watcher.watch(req.path, recursive=req.recursive, file_types=req.file_types)

    # 后台扫描
    if state.pipeline:
        dir_info = {
            "id": dir_id,
            "path": req.path,
            "recursive": req.recursive,
            "file_types": json.dumps(req.file_types),
        }
        threading.Thread(target=state.pipeline.scan_directory, args=(dir_info,), daemon=True).start()

    return {"status": "ok", "id": dir_id}


@router.delete("/index/unbind")
async def unbind_directory(dir_id: str) -> dict:
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")
    dirs = state.db.list_directories()
    target = next((d for d in dirs if d["id"] == dir_id), None)
    if not target:
        raise HTTPException(status_code=404, detail="Directory not found")

    if state.watcher:
        state.watcher.unwatch(target["path"])
    state.db.unbind_directory(dir_id)
    return {"status": "ok"}


@router.get("/index/status")
async def index_status() -> dict:
    if not state.db:
        raise HTTPException(status_code=503, detail="Database not initialized")
    dirs = state.db.list_directories()
    return {
        "directories": dirs,
        "pending_embeddings": state.db.pending_embedding_count(),
    }


@router.post("/index/reindex")
async def reindex() -> dict:
    if not state.db or not state.pipeline:
        raise HTTPException(status_code=503, detail="Not initialized")

    dirs = state.db.list_directories()

    def _do_reindex() -> None:
        for d in dirs:
            state.pipeline.scan_directory(d)  # type: ignore[union-attr]

    threading.Thread(target=_do_reindex, daemon=True).start()
    return {"status": "ok", "message": f"Reindexing {len(dirs)} directories"}
