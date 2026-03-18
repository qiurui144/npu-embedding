"""API 端点测试"""

import pytest
from httpx import ASGITransport, AsyncClient

from npu_webhook.main import app


@pytest.mark.asyncio
async def test_health_check():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/api/v1/status/health")
        assert resp.status_code == 200
        assert resp.json() == {"status": "ok"}


@pytest.mark.asyncio
async def test_system_status():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/api/v1/status")
        assert resp.status_code == 200
        data = resp.json()
        assert "version" in data
        assert "total_items" in data


@pytest.mark.asyncio
async def test_get_settings():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/api/v1/settings")
        assert resp.status_code == 200
        data = resp.json()
        assert data["server_port"] == 18900


@pytest.mark.asyncio
async def test_ingest_too_short():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.post("/api/v1/ingest", json={
            "title": "test",
            "content": "short",
        })
        assert resp.status_code == 400


@pytest.mark.asyncio
async def test_ingest_and_list():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        # 注入
        content = "这是一段足够长的测试内容，用于验证知识注入功能是否正常工作。" * 5
        resp = await client.post("/api/v1/ingest", json={
            "title": "测试知识",
            "content": content,
            "source_type": "note",
        })
        assert resp.status_code == 200
        item_id = resp.json()["id"]

        # 获取
        resp = await client.get(f"/api/v1/items/{item_id}")
        assert resp.status_code == 200
        assert resp.json()["title"] == "测试知识"

        # 列表
        resp = await client.get("/api/v1/items")
        assert resp.status_code == 200
        assert resp.json()["total"] >= 1

        # 更新
        resp = await client.patch(f"/api/v1/items/{item_id}", json={"tags": ["test"]})
        assert resp.status_code == 200

        # 删除
        resp = await client.delete(f"/api/v1/items/{item_id}")
        assert resp.status_code == 200


@pytest.mark.asyncio
async def test_search():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/api/v1/search", params={"q": "测试"})
        assert resp.status_code == 200
        assert "results" in resp.json()


@pytest.mark.asyncio
async def test_search_relevant():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.post("/api/v1/search/relevant", json={
            "query": "测试知识",
            "top_k": 3,
        })
        assert resp.status_code == 200


@pytest.mark.asyncio
async def test_index_status():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/api/v1/index/status")
        assert resp.status_code == 200
        assert "directories" in resp.json()
