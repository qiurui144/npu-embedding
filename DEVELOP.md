# 开发指南

## 环境搭建

```bash
git clone <repo-url> && cd npu-webhook

# Python 后端
python -m venv .venv && source .venv/bin/activate
pip install -i https://pypi.tuna.tsinghua.edu.cn/simple -e ".[dev]"

# Chrome 扩展
cd extension && npm install --registry https://registry.npmmirror.com && cd ..

# Embedding（开发用 Ollama 最简单）
curl -fsSL https://ollama.com/install.sh | sh && ollama pull bge-m3

# 验证
pytest tests/ -v    # 62 个测试全部通过
ruff check src/ tests/
```

## 项目结构

```
src/npu_webhook/
├── main.py                 # FastAPI 入口 + lifespan 初始化
├── config.py               # Pydantic Settings + YAML 配置
├── app_state.py            # 全局状态容器
├── api/                    # API 路由
│   ├── ingest.py           # POST /ingest
│   ├── search.py           # GET /search + POST /search/relevant
│   ├── items.py            # CRUD /items
│   ├── index.py            # /index 目录绑定
│   ├── status.py           # /status 系统状态
│   ├── settings.py         # /settings 配置
│   ├── model_routes.py     # /models 模型管理 + 部署检查
│   ├── skills.py           # /skills (Phase 3)
│   ├── ws.py               # WebSocket
│   └── setup.py            # /setup 安装引导
├── core/                   # 核心引擎
│   ├── embedding.py        # OllamaEmbedding / ONNXEmbedding / OpenVINO
│   ├── vectorstore.py      # ChromaDB 向量检索
│   ├── search.py           # RRF 混合搜索
│   ├── fulltext.py         # jieba + FTS5
│   ├── chunker.py          # 滑动窗口分块
│   └── parser.py           # 文件解析
├── db/                     # 数据库
│   ├── sqlite_db.py        # SQLite (schema/CRUD/FTS5/队列)
│   └── chroma_db.py        # ChromaDB 封装
├── indexer/                # 文件索引
│   ├── watcher.py          # watchdog 监听
│   └── pipeline.py         # 索引管道
├── scheduler/              # 调度
│   └── queue.py            # Embedding 队列 Worker
├── platform/               # 跨平台
│   ├── base.py             # NPUDevice + PlatformProvider ABC
│   ├── detector.py         # 硬件检测 + 芯片匹配 + 驱动检查
│   ├── linux.py / windows.py / paths.py
│   └── ...
└── models/schemas.py       # Pydantic 模型

extension/                  # Chrome 扩展 (Manifest V3 + Preact + Vite)
├── src/
│   ├── content/            # Content Script
│   │   ├── detector.js     # 平台适配器 (ChatGPT/Claude/Gemini)
│   │   ├── capture.js      # MutationObserver 对话捕获
│   │   ├── injector.js     # 无感前缀注入
│   │   ├── indicator.js    # 状态指示器
│   │   └── index.js        # 入口整合
│   ├── background/worker.js    # 消息路由 + 去重 + 健康检查
│   ├── sidepanel/          # Side Panel (搜索/时间线/状态)
│   ├── popup/Popup.jsx     # 快速操作面板
│   ├── options/Options.jsx # 设置页面
│   └── shared/             # messages.js / api.js / storage.js
├── vite.config.js          # 多阶段构建 (IIFE/ESM/HTML)
└── manifest.json
```

## 启动序列

lifespan 初始化顺序（`main.py`）：

1. 日志（RotatingFileHandler 50MB×3）
2. SQLite（schema + WAL）
3. ChromaDB（PersistentClient）
4. Embedding（Ollama 优先 → ONNX 回退 → None 降级）
5. VectorStore + HybridSearchEngine
6. Chunker + IndexPipeline
7. EmbeddingQueueWorker（后台线程）
8. DirectoryWatcher（加载绑定目录）

## Embedding 引擎架构

```
create_embedding_engine(device="auto")
  ├─ device in (auto, ollama) → OllamaEmbedding(HTTP API) → 成功返回
  │                           → 失败: auto 继续, ollama 返回 None
  ├─ device == openvino → 降级警告 → ONNX CPU
  └─ ONNX 模式 → 查找 model_dir → ONNXEmbedding(CPU/DirectML/ROCm)
                → 未找到 → None (搜索回退 FTS5)
```

## 芯片检测流程

`platform/detector.py` 中的 `full_platform_check()`:

1. **内核信息**：`platform.release()` 获取版本
2. **硬件扫描**：Intel NPU(`/dev/accel*`) → AMD NPU(`amdxdna`) → Intel iGPU(`lspci`) → AMD iGPU → Ollama → CPU
3. **芯片级匹配**：通过 PCI ID / lspci 关键词识别具体芯片代（Meteor Lake / Strix Point 等）
4. **驱动栈检查**：内核模块(`lsmod`) + 固件(`/lib/firmware/`) + 用户态运行时(`dpkg -l`)
5. **版本比对**：当前内核版本 vs 芯片最低内核要求
6. **命令生成**：缺失组件 → 精确安装命令

## 扩展构建

```bash
cd extension
npm run build    # 三阶段构建

# 阶段 1: Pages (Preact HTML)  → dist/{popup,sidepanel,options}/
# 阶段 2: Content (IIFE)       → dist/content/index.js
# 阶段 3: Background (ESM)     → dist/background/worker.js
# 最后:   cp indicator.css      → dist/content/indicator.css
```

Content Script 必须是 IIFE（Chrome 不支持 ES module content script），Background Worker 可以是 ES module。

## 测试

```bash
pytest tests/ -v                          # 全部 62 个
pytest tests/test_api.py -v               # API 端点 (8)
pytest tests/test_extension.py -v         # 扩展 E2E (42, 需要后端运行)
pytest tests/test_search.py -v            # 搜索 (2)
pytest tests/test_indexer.py -v           # 索引管道 (6)
pytest tests/test_platform.py -v          # 平台检测 (3)
pytest tests/test_embedding.py -v         # Embedding (1)
```

扩展 E2E 测试使用 Playwright Chromium（`--load-extension`），注意 Google Chrome 不支持此参数。

## 代码规范

- ruff：line-length=120, target=py311
- 公开函数必须有类型注解
- 测试放 `tests/`，调试代码放 `tmp/`（使用后删除）
- API 前缀 `/api/v1/`，端口 18900
- pip 使用清华源

```bash
ruff check src/ tests/
ruff format src/ tests/
```

## 数据库 Schema

| 表 | 用途 |
|---|------|
| `knowledge_items` | 知识条目 |
| `knowledge_fts` | FTS5 全文索引 |
| `embedding_queue` | Embedding 任务队列（P0-P3） |
| `bound_directories` | 绑定目录 |
| `indexed_files` | 文件索引记录（路径/hash） |

ChromaDB collection: `knowledge_embeddings`（cosine 相似度）。

## 认证

- localhost（`127.0.0.1` / `::1`）免认证
- 非本机需 `X-API-Token` 请求头
