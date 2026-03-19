# 版本计划

## 已发布

### v0.2.0 — Phase 0-2：后端核心 + Chrome 扩展 + Embedding

**后端（Phase 0-1）：**
- FastAPI + lifespan 生命周期 + 认证中间件 + CORS
- SQLite（WAL）+ FTS5 全文索引 + Embedding 优先级队列
- ChromaDB 向量存储（cosine）
- 多后端 Embedding：OllamaEmbedding（HTTP API）/ ONNXEmbedding（CPU/DirectML/ROCm）
- RRF 混合搜索引擎（向量 + 全文，可配置权重）
- 文档解析：MD / TXT / 代码 / PDF / DOCX
- 滑动窗口分块（句子边界感知）
- watchdog 多目录监听 + 增量索引管道（SHA-256 hash 去重）
- API：ingest / search / items / index / status / settings / models / ws

**Chrome 扩展（Phase 2）：**
- Manifest V3 + Preact + Vite 多阶段构建（IIFE/ESM/HTML）
- 平台适配器：ChatGPT / Claude / Gemini DOM 选择器 + 消息提取
- Content Script：MutationObserver 对话捕获 + 2s debounce + 流式完成检测
- 无感前缀注入：capture phase 拦截 + 知识分类前缀 + 平台输入框写入
- Background Worker：消息路由 + djb2 去重（session storage 持久化）+ 30s 健康检查
- Side Panel：搜索（source_type 过滤）/ 时间线（日期分组 + 分页 + 删除）/ 状态（8 项指标）
- Popup：连接状态 / 统计 / 注入开关 / 快速操作
- Options：后端地址 / 注入模式 / 排除域名 / 测试连接

**平台检测：**
- 芯片级精确匹配：Intel Meteor/Lunar/Arrow Lake、AMD Phoenix/Hawk/Strix/Krackan Point
- 内核版本比对 + 固件检查 + 内核模块检查 + 用户态运行时检查
- `/models/check` 部署检查 API + 一键安装命令生成
- `/models` 模型列表 + Ollama/ONNX 状态

**测试：** 62 个（20 后端单元 + 42 扩展 E2E Playwright Chromium）

## 路线图

### v0.3.0 — 技能系统

- Skill CRUD API + Jinja2 模板渲染
- URL glob 匹配自动触发
- 技能与知识库联动
- Side Panel 技能管理界面

### v0.4.0 — xPU 原生加速

- Intel NPU：OpenVINO 集成 + ONNX→IR 转换 + INT8 量化
- AMD NPU：DirectML EP 集成
- 硬件自动检测 → 最优设备自动切换
- 系统空闲检测 + 动态 batch size

### v0.5.0 — 分发与安装

- PyInstaller + AppImage（Linux）
- PyInstaller + NSIS EXE（Windows）
- 系统托盘图标（pystray）
- /setup 首次安装引导页
- 模型内嵌 + WebSocket 下载进度
- 开机自启（systemd user service / Windows Service）

### v1.0.0 — 正式发布

- GitHub Actions CI/CD 完整流水线
- 多模态：图片 OCR + 图表理解
- 知识图谱：实体抽取 + 关联推理
- 多轮对话上下文持续注入
- Firefox / Edge 扩展适配
- 端到端加密存储
