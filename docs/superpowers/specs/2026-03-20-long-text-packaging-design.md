# 设计文档：长文本质量提升 + 零摩擦安装

**日期：** 2026-03-20
**状态：** 已批准
**范围：** Phase 3（长文本）+ Phase 3（打包）

---

## 背景与目标

### 核心场景

用户在使用 Gemini / ChatGPT 等 web AI 时，原生文件上传处理效果有限（上下文窗口受限、无语义搜索）。本设计让用户将文件上传至本地知识库，由 npu-webhook 负责高质量处理，查询时自动注入最相关段落，替代原生文件上传。

### 要解决的问题

| 问题 | 现状 | 目标 |
|---|---|---|
| 长文本存入侧语义割裂 | 512字固定窗口切割，chunk 孤立无上下文 | 三层语义分块，章节完整性保留 |
| 注入侧截断严重 | 300字截断 × 3 = 900字，关键信息丢失 | 动态预算 2000字，返回父章节上下文 |
| 文件上传摩擦 | ingest API 只接收纯文本字符串 | 支持原始文件直接上传，后端自动解析 |
| 安装门槛高 | 需要 Python + 命令行 + 手动配置 | 单文件安装包 + 系统托盘 + 扩展自动发现 |

### 暂缓事项（Future Plan）

非浏览器 AI 客户端支持（MCP Server / 系统代理 / 全局热键）延至 Phase 5，见文末。

---

## 设计一：多粒度索引 + 层级检索

### 1.1 三层索引结构

每个 `knowledge_item` 同时建立三个粒度的向量索引：

```
knowledge_item
    ├── Level 0: 文档摘要  (1个/文档)
    │   ├── Markdown / 代码 / 笔记  → 规则提取：标题列表 + 前500字
    │   └── PDF / 网页 / 长文章    → Ollama 生成2-3句摘要（异步低优先级）
    │
    ├── Level 1: 章节      (N个/文档)
    │   ├── Markdown     → 按 ## 标题边界切割
    │   ├── 代码文件     → 按 def / class / function 顶层边界切割
    │   └── 纯文本/PDF   → 每 1500 字在段落边界切割
    │
    └── Level 2: 段落块   (M个/章节，现有 512 字 chunk)
        └── 现有滑动窗口，新增 section_idx 关联父章节
```

**设计原则：**
- Level 2 用于精准命中（高召回）
- Level 1 用于返回上下文（语义完整）
- Level 0 用于文档路由（未来跨文档场景）
- 三层使用同一 bge-m3 模型，NPU 只需加载一次

### 1.2 Schema 变更（向后兼容）

**`embedding_queue` 新增两列（增量迁移）：**

```sql
ALTER TABLE embedding_queue ADD COLUMN level INTEGER NOT NULL DEFAULT 2;
ALTER TABLE embedding_queue ADD COLUMN section_idx INTEGER NOT NULL DEFAULT 0;
```

**ChromaDB metadata 新增字段：**

```json
{
  "item_id": "xxx",
  "level": 1,
  "section_idx": 2,
  "source_type": "file"
}
```

`knowledge_items` 表和现有 API 接口**不变**。

### 1.3 入库流程

```
ingest(content, source_type)
    │
    ├── parser.extract_sections(content, source_type)
    │       → [(section_idx, section_text), ...]
    │
    ├── Level 2: 每个 section → chunker.chunk() → embedding_queue(level=2)
    ├── Level 1: 每个 section_text → embedding_queue(level=1, priority=1)
    └── Level 0: 整文档 → _build_doc_summary(source_type) → embedding_queue(level=0, priority=0)
                  规则类型: 立即生成（同步，< 5ms）
                  Ollama 类型: 异步队列（priority=0，后台最低优先级处理）
```

**NPU 影响：** embedding worker 无感知层级，批量处理队列即可。三层比单层多约 2x embedding 计算量，全部异步离线，查询路径零影响。

### 1.4 两阶段层级检索

```
query
    │
    Stage 1: Level-1 向量搜索
    │    query_embedding → ChromaDB(where: level=1) → top 5 候选章节
    │
    Stage 2: Level-2 精排
    │    在候选章节内 → ChromaDB(where: level=2, section_idx IN [...]) → top K 段落
    │
    Stage 3: 上下文扩展
         每个命中段落 → 取其父章节文本（level=1）作为返回内容
```

**效果对比：**

| 指标 | 现有 | 新设计 |
|---|---|---|
| 返回内容 | 512字截断片段 | 完整父章节（~1500字）|
| 注入上限 | 300字 × 3 = 900字 | 动态预算，默认 2000字 |
| 语义完整性 | 低（片段孤立）| 高（有前后文）|
| 查询延迟增量 | — | +1次 ChromaDB 查询（< 5ms）|

### 1.5 动态注入预算

废弃固定 300 字截断，改为按总预算分配：

```python
INJECTION_BUDGET = 2000  # 字符，可通过 settings 配置

def _allocate_budget(results, budget):
    # 按 score 加权分配预算
    total_score = sum(r["score"] for r in results)
    for r in results:
        share = r["score"] / total_score
        r["_inject_len"] = int(budget * share)
        r["_inject_content"] = r["content"][:r["_inject_len"]]
    return results
```

---

## 设计二：文件上传端点

### 2.1 新增 API

```
POST /api/v1/upload
Content-Type: multipart/form-data

字段：
  file        必填  原始文件（PDF / DOCX / MD / TXT / 代码）
  session_id  可选  会话标识，用于注入时优先排序

响应：
{
  "id": "item_id",
  "title": "提取的文件名",
  "chunks_queued": 42,
  "status": "processing"   // FTS5 立即可搜，向量搜索异步就绪
}
```

### 2.2 内部流程

```
接收文件（multipart）
    → 文件类型检测（MIME + 扩展名）
    → parser.parse_file(file_bytes, file_type) → 纯文本
    → ingest 标准流程（三层入库）
    → 立即返回（FTS5 可搜，embedding 后台处理）
```

**支持格式（复用现有 parser.py）：** PDF、DOCX、MD、TXT、Python、JavaScript

**文件大小限制：** 20MB（可配置），超限返回 413

### 2.3 会话感知注入加权

上传文件后，`item_id` 写入 `chrome.storage.session`。注入时对本次会话内上传的文件额外加权：

```js
// worker.js
if (sessionUploadedIds.has(result.id)) {
  result.score *= 1.5;  // 会话内上传文件优先展示
}
```

---

## 设计三：扩展侧边栏文件标签

在 SidePanel 增加第四个标签"文件"：

```
侧边栏标签：[搜索] [时间线] [文件] [状态]

文件页：
  ┌─────────────────────────────┐
  │  拖拽文件到此处，或点击选择  │
  │  支持：PDF DOCX MD TXT 代码 │
  └─────────────────────────────┘
  上传中... [████████░░] 80%
  ✓ report.pdf — 已处理（42个段落）
  ✓ notes.md   — 已处理（8个段落）
```

上传完成后自动切换到搜索页，用户可预览命中效果。

---

## 设计四：零摩擦安装（普通用户）

### 4.1 目标安装体验

```
1. 下载安装包（.exe / .AppImage / .dmg）
2. 双击运行 → 系统托盘图标出现，后端自动启动
3. 安装 Chrome 扩展
4. 扩展首次打开 → 自动发现本地后端（固定端口 18900）→ 显示"已连接"
5. 完成
```

### 4.2 系统托盘（新增 `tray.py`）

```python
# src/npu_webhook/tray.py
启动顺序：
  TrayApp
    ├── 子线程启动 uvicorn（复用现有 main.py）
    ├── pystray 系统托盘
    │     ├── 图标状态：● 运行中 / ● 处理中 / ○ 错误
    │     ├── Tooltip: "npu-webhook ● | 设备: Intel Arc | 条目: 1234"
    │     └── 菜单：打开侧边栏 / 暂停注入 / 查看日志 / 退出
    └── 开机自启（systemd user service / launchd plist / 注册表 Run）
```

**依赖：** `pystray`、`Pillow`（托盘图标渲染）

### 4.3 扩展首次引导

```jsx
// Popup.jsx — 后端未连接时
if (!backendConnected) {
  return <OnboardingView
    steps={[
      { label: '下载并运行 npu-webhook', done: false, link: RELEASE_URL },
      { label: '安装后此界面自动刷新', done: false },
    ]}
    autoRefreshMs={3000}
  />;
}
```

无需用户填写任何 URL，固定探测 `http://localhost:18900`。

### 4.4 发布产物

| 平台 | 产物 | 打包方式 |
|---|---|---|
| Windows | `npu-webhook-setup.exe` | PyInstaller + NSIS |
| Linux | `npu-webhook.AppImage` | PyInstaller + AppImage |
| macOS | `npu-webhook.dmg` | PyInstaller + dmgbuild |

**已有基础：** `packaging/` 目录有 PyInstaller spec 和 AppImage/NSIS 配置，本次在此基础上补充 tray 入口。

---

## 完整变更范围

| 模块 | 变更类型 | 说明 |
|---|---|---|
| `core/chunker.py` | 扩展 | 新增 `extract_sections(text, source_type)` 语义分块 |
| `core/parser.py` | 扩展 | 补全 `parse_file(bytes, type)` 接口；章节边界识别 |
| `core/search.py` | 修改 | 两阶段层级检索替换现有单层检索；动态预算注入 |
| `db/sqlite_db.py` | 迁移 | `embedding_queue` 加 `level`/`section_idx` 列 |
| `api/ingest.py` | 扩展 | 新增 `POST /upload` multipart 端点 |
| `api/search.py` | 小改 | 传递层级参数到搜索引擎 |
| `extension/sidepanel` | 新增 | 新增"文件"标签页 + 拖拽上传 UI |
| `extension/worker.js` | 小改 | 会话感知加权逻辑 |
| `extension/shared/api.js` | 扩展 | 新增 `uploadFile(file)` 方法 |
| `src/npu_webhook/tray.py` | 新增 | 系统托盘主进程 |
| `packaging/` | 调整 | tray 入口 + pystray/Pillow 依赖 |

**不变：** `knowledge_items` 表结构、现有 `/ingest` `/search` `/items` API、ChromaDB collection 结构（仅扩展 metadata 字段）

---

## Future Plan：Phase 5 — 非浏览器 AI 客户端（暂缓）

```
优先级排序：

1. MCP Server（Claude Desktop / Cursor）
   知识库暴露为 MCP tool:
     - search_knowledge(query, top_k) → 返回相关知识片段
     - upload_file(path) → 入库本地文件
   零侵入，AI 客户端原生调用，无需 DOM 注入

2. 系统代理注入（所有桌面 AI 工具）
   本地 HTTP 代理拦截 OpenAI-compatible API 请求
   在 system prompt 中自动注入相关知识
   适用：aichat / mods / shell_gpt / 自建工具

3. 全局热键浮层（任意场景兜底）
   Ctrl+Shift+K 打开搜索浮层（Qt / Tauri）
   结果一键复制到剪贴板或注入输入焦点
```

---

## 测试计划

- `tests/test_chunker.py`：`extract_sections` 对 Markdown/代码/纯文本的分割正确性
- `tests/test_search.py`：层级检索结果包含父章节上下文；动态预算不超限
- `tests/test_api.py`：`/upload` 端点接受 PDF/DOCX，返回正确 item_id 和 chunks_queued
- E2E：文件上传后侧边栏搜索可命中文件内容；注入时会话内文件排名靠前
