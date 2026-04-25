# 行业版 Attune 软件设计（独立应用 · 律师 vertical 第一刀）

**版本**：v1 · 2026-04-25
**作者**：qiurui144
**关联决策**：CLAUDE.md「独立应用边界」+「产品决策记录 2026-04-25」5 条
**前置 spec**：[2026-04-17-product-positioning-design.md](2026-04-17-product-positioning-design.md)（三大支柱定位：主动进化 / 对话式 / 混合智能）

---

## 0. 摘要

把 Attune 从"通用私有 AI 知识伙伴"升级为**会员制行业 AI 应用**，第一个 vertical 切律师（个人版 attune-law-personal）。

**与 lawcontrol 关系**：完全独立。不调 lawcontrol API、不复用其代码，可参考其 plugin / RPA / Intent Router 设计模式（七类插件分法 + AI 边界严守），实现完全自研。

**双形态**：
- **B 形态**（主路径）：本地笔电 + 远端 LLM token。Tauri 2 桌面壳套现有 Preact 前端，单一 Attune.exe / .AppImage 双击即用，含原生窗口 / 托盘 / 单实例 / 自动更新
- **A 形态**（二期）：K3 一体机（SpacemiT X100，192.168.100.209）跑 attune-server headless（无 Tauri），用户用浏览器或 Tauri Desktop 远程接入；底座推理由 K3 :8080 提供，可选装本地 LLM
- **同一份 Rust 后端代码**（attune-server crate），双形态共享

**核心价值**：律师丢一张借条照片，attune 5 秒内告诉他"这是王某诉李某案 · 第 3 份证据 · 与已有借款合同金额一致 · 与微信记录时间冲突 · 建议补充资金到账银行流水"。

---

## 1. 三层架构

```
┌──────────────────────────────────────────────────────────────┐
│  接入层  Web UI  ·  Chrome 扩展  ·  IM channel（v1.0+）       │
├──────────────────────────────────────────────────────────────┤
│  AI 层   skill (单步)  +  workflow (多步)  +  intent router  │  → 远端 token（默认）
│          plugin.yaml 契约：output schema · needs_confirm     │     K3 形态可走本地
│          chat_trigger.patterns / keywords 自然语言路由       │
├──────────────────────────────────────────────────────────────┤
│  数据层  RPA · 全文 · 向量 · Project 卷宗 · 个人知识库        │  ← 本地（笔电盘）
│          严禁碰 AI · 必须确定性 + 合规                         │     或 K3 SSD
└──────────────────────────────────────────────────────────────┘
```

**AI 边界硬约束**：数据层（RPA / crawler / 检索）禁用任何 AI 调用 — 这是商业可信的底座。AI 只在 AI 层（skill / workflow）里出现。借鉴自 lawcontrol，attune 自研实现。

---

## 2. 数据模型 — Project / Case 卷宗

### 2.1 Project 通用层（attune-core）

```sql
CREATE TABLE project (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    kind TEXT NOT NULL,                    -- 'case' / 'deal' / 'topic' / 'generic'
    metadata_encrypted BLOB,               -- 行业特化字段（律师 = 案件信息），AES-256-GCM
    created_at INTEGER, updated_at INTEGER,
    archived INTEGER DEFAULT 0
);

CREATE TABLE project_file (                -- 多对多：一个文件可属多 Project
    project_id TEXT, file_id TEXT,
    role TEXT,                             -- 行业特化（律师 = 'evidence'/'pleading'/'reference'）
    added_at INTEGER,
    PRIMARY KEY (project_id, file_id)
);

CREATE TABLE project_timeline (            -- 跨证据链推理的时间线
    project_id TEXT, ts INTEGER,
    event_type TEXT,                       -- 'fact' / 'evidence_added' / 'rpa_call' / 'ai_inference'
    payload_encrypted BLOB
);
```

### 2.2 Case 行业层（attune-law plugin）

`metadata_encrypted` 在 attune-law 渲染时反序列化为：

```yaml
case_no: "(2024)京02民终1234号"
court: "北京市第二中级人民法院"
parties:
  - role: plaintiff
    name: "王某"
    type: natural_person
  - role: defendant
    name: "李某"
    type: natural_person
case_type: "民间借贷纠纷"
status: "一审进行中"
filing_date: "2024-03-15"
hearing_dates: ["2024-05-20", "2024-07-08"]
```

attune-core 只看到一个 opaque blob，行业插件解码渲染。

### 2.3 创建时机（Q-A 答案：b 推荐式）

- **第一份文件上传时不强制选 Project**（不打断零散使用）
- AI 在以下任一条件触发后浮出 "建议归档到 Project" 气泡：
  - 用户已上传 ≥ 3 份文件且**实体重叠度** > 0.6（同人名 / 同案号 / 同公司）
  - 用户在 chat 里提到"案件 / 客户 / 项目"等关键词
  - 用户上传新文件时检测到 ≥ 2 个已有文件实体重叠
- 用户三选一：**[新建 Project] / [加入 ${existing}] / [跳过，永远视为零散]**
- 已有文件支持事后批量归类（"案件管理" tab 拖拽分组）

---

## 3. AI 层

### 3.1 plugin.yaml 升级（在 attune-pro 现有基础上加 chat_trigger）

```yaml
# plugins/law-pro/capabilities/contract_review/plugin.yaml
id: law-pro/contract_review
type: skill
name: 合同风险审查
version: "0.1.0"

requires:
  attune_core: ">=0.6.0"

constraints:
  output_format: json
  temperature: 0.2

output:
  schema: { ... 已存在 ... }

# —— 新增：自然语言路由 ——
chat_trigger:
  enabled: true
  needs_confirm: true              # AI 处理前必须用户确认
  priority: 5
  patterns:
    - '(帮我|请).*(审查|审核|review).*(合同|协议|条款)'
  keywords: ['审查合同', '合同风险', '看一下这份合同']
  min_keyword_match: 1
  exclude_patterns: ['起草', '生成']
  requires_document: true          # 必须有上传的文件
  description: "AI 审查合同条款风险"

# —— 新增：跨 Project 上下文要求 ——
context_strategy:
  scope: project                   # 'project' | 'global' | 'file_only'
  inject_top_k_related: 5          # 自动注入同 Project 内最相关的 K 个 chunk
```

### 3.2 Intent Router（attune-core 新增 ~300 行）

```rust
// crates/attune-core/src/intent_router.rs
pub struct IntentRouter {
    skills: Vec<SkillManifest>,    // 启动时扫描 plugins/* 加载
}

impl IntentRouter {
    pub fn route(&self, user_message: &str, context: &ChatContext) -> Vec<IntentMatch> {
        // 1. 正则 patterns 匹配
        // 2. keywords 计数 ≥ min_keyword_match
        // 3. exclude_patterns 否决
        // 4. requires_document 检查 context.has_pending_file
        // 5. 多个匹配按 priority 排序，返回 top-N
    }
}

pub struct IntentMatch {
    pub skill_id: String,
    pub confidence: f32,
    pub needs_confirm: bool,
    pub args: serde_json::Value,
}
```

UI 层：用户敲完一句话 → router 返回匹配 → 如果 confidence > 阈值且 needs_confirm，浮出 chip "AI 检测到你想审查合同，使用 contract_review skill？[确定] [换个问法]"。

### 3.3 跨证据链推理 workflow（核心价值）

新 workflow type，不是 skill。三段式：

```yaml
# plugins/law-pro/workflows/evidence_chain_inference/workflow.yaml
id: law-pro/evidence_chain_inference
type: workflow
trigger:
  on: file_added                   # 文件上传后自动跑（被动触发，但只在 Project scope 内）
  scope: project

steps:
  - id: extract_entities
    type: skill
    skill: law-pro/entity_extraction
    input: { file_id: $event.file_id }
    output: entities                # 人名 / 金额 / 日期 / 案号 / 地点

  - id: cross_reference
    type: deterministic            # 不调 AI，纯 SQL 查询
    operation: find_overlap
    input:
      entities: $extract_entities.entities
      project_id: $event.project_id
    output: related_files

  - id: inference
    type: skill
    skill: law-pro/evidence_chain_skill
    input:
      new_file: $event.file_id
      related: $related_files
      project_metadata: $event.project_metadata
    output:
      - location: "证据归属哪条事实链"
      - relations: "与哪些证据呼应/矛盾"
      - gaps: "证据链还缺什么"

  - id: render
    type: deterministic
    operation: write_annotation
    input: $inference
```

输出落到批注侧栏 + Project timeline 节点，律师打开时一目了然。

---

## 4. 数据层 — 自研 RPA

### 4.1 七类插件分法（参考 lawcontrol）

| type | AI 允许？ | 例子 |
|:---|:---|:---|
| rpa | ❌ 严禁 | npc_law / 公众号 / 裁判文书（v0.7） |
| crawler | ❌ 严禁 | RSS / 法律出版社官网 |
| search | ❌ 严禁 | 已有的 web_search（DuckDuckGo / Bing） |
| **skill** | ✅ | contract_review / lawyer_letter / ... |
| **workflow** | ✅（步骤间编排） | evidence_chain_inference |
| channel | ❌ | 微信群（v1.0+） / Outlook（v1.0+） |
| industry | — | 聚合声明（attune-law）|

### 4.2 RPA 适配器（自研，复用 chromiumoxide）

底层基础已在 `attune-core/src/web_search_browser.rs`（chromiumoxide 驱动 system Chrome）。新增 `attune-core/src/rpa/` 模块：

```rust
// crates/attune-core/src/rpa/mod.rs
#[async_trait]
pub trait RpaAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn manifest(&self) -> &RpaManifest;       // 来自 plugin.yaml
    async fn invoke(&self, op: &str, args: serde_json::Value, ctx: &RpaContext) -> RpaResult;
    async fn health_check(&self) -> AdapterHealth;
}

pub struct RpaContext {
    pub user_id: String,
    pub project_id: Option<String>,
    pub task_id: String,                       // 给前端 follow 的 ID
    pub progress_tx: mpsc::Sender<Progress>,   // 异步增量推进度
    pub browser_pool: Arc<BrowserPool>,        // 共享 chromiumoxide 实例
}
```

### 4.3 v0.6 GA 第一批 RPA（只做免登录）

| adapter | 站点 | 操作 | 工作量 |
|:---|:---|:---|:---|
| `flk_npc` | flk.npc.gov.cn | search_law / get_article（按法条号） | 1 天 |
| `wechat_article` | mp.weixin.qq.com | extract（用户分享 URL，提取正文 + 元信息） | 1 天 |

需账号的（裁判文书 / pkulaw / qichacha）作 v0.7 升级卖点。

### 4.4 RPA 工作流四维（Q-C 答案：b 列清单律师勾选）

**1. 触发模式**

| 模式 | 默认 | 例子 |
|:---|:---|:---|
| 主动 | ✅ ON | chat: "查《劳动合同法》第 39 条" |
| 被动（文件触发） | 🔘 抽实体后**列清单律师勾选** | 上传起诉状 → AI 抽出"被告: 某某有限公司" → 浮气泡："要查工商信息吗？[查] [跳过]" |
| 定时 | 🔘 单条 opt-in | 暂缓到 v0.7 |

**2. 执行模式 — 异步后台 + 顶栏进度面板**

```
chat 输入"查王某 vs 李某 裁判文书"
  → IntentRouter 路由到 wenshu RPA（v0.7）/ flk_npc RPA（v0.6）
  → 立即返回 task_id（< 200ms）
  → 顶栏 chip "后台任务 (1)" 闪现
  → 用户继续聊天 / 浏览
  → 完成（~10s）→ 浏览器内通知 + chat 自动 follow-up
     "查到 5 条结果，归档到 ${project_name}：[#1 ...] [#2 ...]"
  → Project timeline 添加 'rpa_call' 节点
```

**3. 错误恢复**

| 故障 | 处理 |
|:---|:---|
| 账号失效 | 弹窗 "${站点}账号失效，开 headed 浏览器重新登录" → cookie 持久化到 vault |
| 验证码 | 切 headed 模式让用户手动过 |
| 限速 | 自动 backoff（指数退避，最多 3 次）+ 任务面板显示"等待中" |
| 数据缺失 | 返回结构化"未找到" + Suggested rewrite |

**4. 审批门 + 成本可见**

- 每次 RPA 调用前弹气泡（除非 Project 设置了"永远自动通过"）：
  > "即将调用 ${adapter} · 预计 1 配额（剩 99）· ~12s · 远端 LLM 解析 ~200 tok（¥0.0006）。[继续] [跳过] [此 Project 自动通过]"
- 顶栏"后台任务" chip 点开 = 当日 RPA 配额 / Token / 估算费用面板
- 每次调用记录到 Project timeline（合规审计）

---

## 5. 接入层 — Chrome 扩展行业化

### 5.1 现状 vs 目标

现状：扩展只捕 ChatGPT/Claude/Gemini 对话 + 注入个人知识 + 文件上传
目标：**自动捕行业相关浏览习惯**

### 5.2 行业模板

`attune-law plugin` 自带白名单：

```yaml
browser_capture_templates:
  - domain: flk.npc.gov.cn
    label: 国家法律法规库
    selector: { content: ".law-content", title: "h1.law-title" }
    auto_extract_fields: [law_no, effective_date, articles]

  - domain: wenshu.court.gov.cn
    label: 裁判文书网
    requires_user_account: true
    selector: { content: ".PDF_pox", title: ".labelBox" }
    auto_extract_fields: [case_no, court, parties, judgment_date, key_points]

  - domain: mp.weixin.qq.com
    label: 公众号文章（律法相关）
    keyword_filter: ['法律', '判例', '律师', '合同', '合规']  # 标题/作者关键词命中才识别
    selector: { content: ".rich_media_content", title: "#activity-name" }

  - domain: mail.qq.com
    label: 邮箱（标题含案件关键词）
    keyword_filter: ['案号', '诉讼', '律师函', '合同']
    selector: { content: ".body-content", title: ".subject" }
```

### 5.3 自动浮窗 + 三档默认（Q-B 答案：c）

进入白名单页面时：
- 内容抽取（在扩展端 readability + selector）
- **三档默认行为**（首次安装时强制选）：
  - **激进**：5 秒倒计时浮窗，不点就归档到 Suggested Project（1Password Watchtower 风格）
  - **平衡**（推荐 ★）：永远显示气泡，需点击"归档/跳过/永远忽略"
  - **保守**：默认完全不显示，需用户点扩展工具栏图标才归档

### 5.4 检索行为捕获

用户在 pkulaw / 裁判文书网搜索 → 扩展捕获**检索词 + 命中前 5 条标题** → 自动入 Project research log（不入正文，只记元数据）。

### 5.5 浏览习惯画像

每周一早上扩展 sidebar 推送：
> "本周你在 pkulaw 检索 18 次，最关注'劳动合同 解除'。建议关注：《最高法关于审理劳动争议案件适用法律解释（二）》（已自动归档）"

---

## 6. 本地 AI 底座

### 6.1 模块成熟度（盘点 2026-04-25）

| 模块 | 笔电（B 形态） | K3（A 形态） |
|:---|:---|:---|
| Embedding | ORT bge-base / Ollama bge-m3 ✅ | K3 :8080 /v1/embeddings ✅ |
| Rerank | ORT bge-reranker ✅ | K3 :8080 /v1/rerank ✅ |
| ASR | **❌ 缺**（whisper.cpp 待集成） | K3 :8080 /v1/transcribe ✅ |
| OCR | tesseract + poppler ✅ | K3 :8080 /v1/ocr ✅ |
| LLM Chat | Ollama（用户自装）/ 远端 API（默认） | 远端 API 默认 / K3 可选装 |

### 6.2 ASR 集成方案（M3）

- **whisper.cpp binary + Rust subprocess**（与 K3 一致路径）
- 默认 model：**whisper-small Q8**（中文 WER 15-20% 业务可用，~500 MB）
- 安装包捆绑 whisper-cli.exe（Win）/ whisper-cli（Linux）+ ggml-small-q8.bin
- 用户硬件 < 8GB RAM 时降级到 whisper-tiny + UI 提示"精度有限，建议用 K3 一体机"
- 中文 WER 实测加入 `tests/golden/asr_*.json` 做 quality regression

### 6.3 模型分发（M2）

**捆绑**（笔电安装包，~150-200 MB）：
- bge-small ONNX（~90 MB，dim 512，所有硬件 fallback）
- bge-base ONNX（~280 MB，dim 768，≥ 16GB RAM 默认）— 可选下载
- whisper-small Q8（~500 MB）
- tesseract chi_sim 训练数据（~50 MB）

**不捆绑**：
- LLM 模型（用户走远端 token 默认；想本地装的提示运行 `ollama pull qwen2.5:7b`）
- bge-m3（~1.2 GB，"Settings → 升级模型"按需下载）

### 6.4 远端 LLM 默认配置

启动后用户 Settings 里有：
- Endpoint：默认 `https://api.attune.ai/v1`（attune 自营 gateway，含支付宝 / 微信扫码充值）
- 也可填 OpenAI / Anthropic / DeepSeek / 月之暗面 / 智谱 / 云端 Ollama 任意 OpenAI 兼容 endpoint
- API key 加密存到 vault

---

## 6.5 桌面壳（Tauri 2）

### 6.5.1 决策（Q-D / Q-E / Q-G）

- **Q-D 入口形态 = (a) 双轨发版** — Tauri 桌面包给笔电用户，纯 attune-server 包给 K3 / NAS / 服务器。同一份 Rust 后端代码。
- **Q-E WebView 加载方式 = (a) HTTP 加载 :18900** — 前端零改动，Tauri 是浏览器壳。后期可增量把高频 API 改走 Tauri IPC，不必一次到位。普通浏览器访问 :18900 也能用（headless 兼容）。
- **Q-G attune-server 启动方式 = (a) Tauri 主进程内嵌 axum runtime（同进程）** — 单一二进制 Attune.exe，启动时同时拉起 axum + WebView。崩溃即崩溃，分发简单。

### 6.5.2 架构

```
Attune Desktop（笔电用户）            Attune Server（K3 / NAS / 服务器）
┌──────────────────────────┐         ┌──────────────────────────┐
│ apps/attune-desktop      │         │ rust/crates/attune-server│
│ (Tauri 2, Rust)          │         │ (axum :18900 only)       │
│                          │         │                          │
│ ├─ Tauri WebView         │         │  ← 浏览器 / Chrome 扩展  │
│ │  → 加载 :18900         │         │     远程访问             │
│ ├─ 系统托盘 + 单实例     │         │                          │
│ ├─ 原生通知 / 文件关联   │         │                          │
│ ├─ 自动更新（见 §6.6）   │         │                          │
│ └─ 内嵌 attune-server    │         │                          │
│    （axum 跑在同进程）   │         │                          │
└──────────────────────────┘         └──────────────────────────┘
        Win MSI / Linux deb                Linux deb / aarch64
```

### 6.5.3 Cargo workspace 改动

```toml
# rust/Cargo.toml
[workspace]
members = [
    "crates/attune-core",
    "crates/attune-server",       # 改为 lib + 旧 bin headless 入口保留
    "apps/attune-desktop",        # 新增 — Tauri shell + 内嵌 attune-server
]
```

`attune-server` 从单一 binary crate 改为 **library + 一个 headless bin**：
- `lib.rs` 暴露 `pub fn run_in_runtime(handle: tokio::runtime::Handle, config: ServerConfig) -> ServerHandle`
- `bin/attune-server-headless.rs` 是原 main，保留给 K3 / NAS 部署
- `apps/attune-desktop/src/main.rs` 调用 `attune_server::run_in_runtime()` 把 axum 跑在 Tauri tokio runtime 上

### 6.5.4 桌面壳必备特性

| 特性 | 实现 | 优先级 |
|:---|:---|:---|
| 系统托盘（关闭最小化） | `tauri::tray::TrayIconBuilder` | P0 |
| 单实例锁（重复双击只激活已有窗口） | `tauri-plugin-single-instance` | P0 |
| 启动 splash + axum 健康检查 | 自定义 webview pre-init | P0 |
| 文件关联（双击 .pdf 用 attune 打开） | tauri.conf.json `fileAssociations` | P1 |
| 拖拽文件到窗口 → 自动上传 | webview drop event → invoke('upload') | P0 |
| 原生通知（RPA 任务完成） | `tauri::notification::Notification` | P1 |
| 自动启动（开机自启） | `tauri-plugin-autostart` | P2，opt-in |
| 系统暗色模式跟随 | `tauri::WebviewWindow::theme()` | P2 |
| 顶部菜单栏（Mac 风 / Win 风） | `tauri::Menu` | P2 |

### 6.5.5 Tauri 工程结构

```
apps/attune-desktop/
├── Cargo.toml
├── tauri.conf.json          # 构建配置（appId / bundler / updater endpoint）
├── build.rs                  # tauri-build 编译时脚本
├── src/
│   ├── main.rs               # tauri::Builder + 内嵌 attune_server::run_in_runtime
│   ├── tray.rs               # 系统托盘
│   ├── single_instance.rs    # 单实例锁
│   └── commands.rs           # Tauri IPC commands（v0.6 暂不用，HTTP 优先）
├── icons/                    # 平台图标（icon.ico / icon.png / icon.icns）
└── ../../crates/attune-server/ui/dist/  # 前端构建产物（被 Tauri 嵌入）
```

---

## 6.6 安装更新策略

### 6.6.1 设计目标

- **零摩擦**：用户不需要主动检查更新；后台静默检查，重大版本弹窗提示
- **可信任**：所有更新包 Ed25519 签名，离线验签，劫持包无法安装
- **可控**：用户可选 stable / beta 通道；可关闭自动检查
- **回滚**：若新版崩溃，下次启动时检测到 panic 自动建议回滚（需保留上一版二进制）
- **双轨形态各自适配**：Tauri Desktop 走 in-app updater；attune-server-only 走系统包管理（apt / yum / dnf）

### 6.6.2 Tauri Desktop 更新流（笔电）

```
启动后 30 秒（不阻塞 UI）
  ↓
GET https://updates.attune.ai/desktop/{stable|beta}/latest.json
  ↓
{
  "version": "0.7.0",
  "notes": "新增...",
  "pub_date": "2026-05-08T10:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "...",                        # minisign 签名
      "url": "https://.../Attune_0.7.0_x64.msi"
    },
    "linux-x86_64": { ... }
  }
}
  ↓
比较版本号 → 高于当前 ?
  ↓ 是
  ↓
判断 channel：
  - "patch"（0.6.0 → 0.6.1）= 后台静默下载，下次启动应用
  - "minor"（0.6.x → 0.7.0）= 弹窗提示用户，"现在更新 / 稍后 / 跳过此版本"
  - "major"（0.x → 1.0）= 强制弹窗 + 列出 breaking changes，必须用户点击
```

### 6.6.3 签名链路

```
开发机
├── Ed25519 私钥（离线保管，2-of-3 Shamir 分片）
└── tauri signer sign Attune_0.7.0_x64.msi → Attune_0.7.0_x64.msi.sig

CI 发版
├── 构建产物上传 OSS / S3
├── 生成 latest.json（含签名 base64）
└── 推到 https://updates.attune.ai/desktop/stable/latest.json

客户端验签
├── 启动时下载 latest.json
├── tauri-plugin-updater 用内置公钥验 latest.json 签名
├── 下载 .msi → 用同公钥验 .msi.sig → OK 才允许安装
└── 验签失败 → 拒绝 + 上报遥测
```

公钥**编译进二进制**（不是配置文件，避免被替换）。私钥泄露则发**轮替版本**，把新公钥编进新版二进制。

### 6.6.4 attune-server-only 更新流（K3 / NAS）

走标准 Linux 包管理，不重新发明：
- **Debian/Ubuntu**：发版到 `apt.attune.ai` 仓库（或 GitHub Releases + apt-get），用户走 `apt update && apt upgrade attune-server`
- **systemd timer 自动检查**：可选 `attune-update.timer`（每天 03:00 检查），失败则 `journalctl -u attune-update` 告警
- **K3 一体机**：出厂带 `apt.attune.ai` 仓库配置，开机即可收到更新通知

### 6.6.5 通道与版本策略

| 通道 | 用户 | 频率 | 稳定性 | 配置 |
|:---|:---|:---|:---|:---|
| stable | 默认 | 月度 minor / 周度 patch | 严格通过 QA + golden set 全过 | Settings 默认值 |
| beta | 主动 opt-in | 双周 | golden set 通过即发 | Settings 切换 |
| nightly | 内部 / 内测律师 | 每日 | 仅 CI 通过 | 隐藏入口（特殊 license key） |

### 6.6.6 回滚机制

- 每次成功更新后保留 **上一版二进制**到 `~/.local/share/attune/.previous/`（最多 1 个）
- 若新版连续 3 次启动后 5 秒内 crash，下次启动时弹窗：
  > "Attune 0.7.0 启动后频繁崩溃。要回滚到 0.6.5 吗？[回滚] [继续尝试] [发送崩溃报告]"
- 用户点回滚 → 替换 binary + 写日志 + 上报遥测

### 6.6.7 数据迁移与 schema 版本

- vault schema 版本号嵌入 SQLite `PRAGMA user_version`
- 新版启动时检查 `db_version < embedded_version` → 跑 `migrations/<from>_to_<to>.sql`
- 迁移**只前进不后退**；回滚到旧版时若 schema 已升级，旧版报错"vault 已被新版打开过，请重新升级或从备份恢复"
- 重大 schema 变更（如 v0.6 → v1.0）必须发布前 3 个月预告 + changelog + 自动备份

---

## 7. 跨平台分发（M1 + 平台优先级）

**两条独立的发版流水线**：

### 7.1 阶段 0：跨平台编译卫生（共享前置，0.5 周）

```toml
# Cargo.toml
[features]
default = []
cuda = ["ort/cuda"]                  # Linux NVIDIA
directml = ["ort/directml"]          # Windows 核显/独显
# coreml feature 保留但 v0.6/v0.7 不验证
```

`#[cfg(unix)]` / `#[cfg(windows)]` 全面补全（vault 文件权限 / 临时目录 / 进程管理 / 路径分隔符）。

### 7.2 Attune Desktop（笔电用户，Tauri bundler 打包）

通过 Tauri bundler 一键出三种产物（**不再用 WiX / cargo-deb 各自配置**）：

| 平台 | 产物 | 优先级 | 大小（估）|
|:---|:---|:---|:---|
| Windows x86_64 | `Attune_0.6.0_x64-setup.exe`（NSIS）+ `Attune_0.6.0_x64.msi`（Wix from Tauri） | **P0** | ~150 MB |
| Linux x86_64 | `attune_0.6.0_amd64.deb` + `Attune-0.6.0.AppImage` | **P1** | ~150 MB |
| Linux aarch64 | `attune_0.6.0_arm64.deb`（K3 一体机预装） | P2 | ~150 MB |
| macOS | `Attune_0.6.0_x64.dmg` + `Attune_0.6.0_aarch64.dmg` | 暂不做 | — |

**安装包内含**：Tauri shell + attune-server runtime + Ollama runtime + whisper-cli + tesseract + 必要底座模型（bge-small + whisper-small Q8 + tesseract chi_sim）。**不含 LLM 模型**（M2 决策）。

**Windows 签名**：
- v0.6 alpha：用 self-signed 证书或不签（用户首次会有 Defender 警告，给定向用户用）
- v0.6 GA：购买 EV Code Signing 证书（¥2000-5000/年），SmartScreen 信誉冷启动 7-14 天
- 关键：留 2 周 buffer 给签名信誉积累

**自动更新**：见 §6.6 — Tauri 内置 updater 接 `https://updates.attune.ai/desktop/`。

### 7.3 Attune Server（K3 / NAS / 服务器，纯 attune-server）

走标准 Linux 包管理，**不打 Tauri**：

| 渠道 | 命令 | 优先级 |
|:---|:---|:---|
| Debian/Ubuntu apt 仓库 | `curl ... \| sudo apt-key add - && apt install attune-server` | P0（K3 一体机依赖）|
| GitHub Releases tarball | 解压即用 | P1 |
| Docker image | `docker run -p 18900:18900 attune/server:0.6.0` | P2 |
| systemd unit 模板 | `apt install` 时自动注册 user unit | P0 |

**自动更新**：apt 自带 `apt-get update && apt-get upgrade`；`attune-update.timer` 可 opt-in。

### 7.4 macOS（暂不做）

不投入资源至 v1.0；保留 Tauri / cfg 抽象，未来一行改动可通。

---

## 8. 会员体系

### 8.1 三档定价

| | 笔电软件订阅 | K3 一体机捆绑 |
|:---|:---|:---|
| **个人版** | ¥99/月（含 50 万 tok/月 远端 LLM + flk_npc/wechat 免费 RPA） | ¥3999 硬件 + ¥99/月（同 quota） |
| **专业版** | ¥299/月（含 200 万 tok/月 + 所有 RPA + skill 优先级） | ¥6999 硬件 + ¥299/月 |
| **行业插件包** | 单买 ¥199/月/包（attune-law / attune-presales / ...） | 同 |

### 8.2 License Key（沿用 attune-pro/docs/license-key-design.md）

- HMAC-SHA256 离线校验
- payload：`{ key_id, plan, seats, features, device_fp, issued_at, expires_at, grace_days, customer_id }`
- 失效后 **grace 7 天**全功能 → 7-30 天只读 → 30 天后只能 export
- 撤销列表：CRL 走 attune.ai/api/v1/license/crl，每 24h 拉一次（离线时旧规则生效）

---

## 9. Sprint 节奏（11 周到 attune-law-personal v0.1）

| Sprint | 周 | 交付 | 依赖 | 可并行？ |
|:---|:---|:---|:---|:---|
| **0** | 0.5 | 跨平台编译卫生（ort feature 拆 / cfg 补全 / Win MSVC build 通） | — | — |
| **0.5** | 1.5 | **Tauri 2 桌面壳**（apps/attune-desktop）+ 内嵌 axum + 托盘 + 单实例 + 拖拽 + Win/Linux Tauri bundler 出包成功 | S0 | — |
| **1** | 1.5 | Project / Case 数据模型 + AI 推荐归类 + 跨证据链 workflow | S0 | 与 S0.5 并行 |
| **2** | 2 | Intent Router + 9 个 attune-pro skill 加 chat_trigger（5 law + 4 presales）| S1 | — |
| **3** | 2 | RPA 自研：flk_npc + wechat_article + 异步后台框架 + 顶栏进度面板 | S2 | 与 S4 并行 |
| **4** | 1 | 扩展行业化：白名单 + 浮窗 + 三档默认 + 检索捕获 | S2 | 与 S3 并行 |
| **5** | 1 | ASR 集成（whisper.cpp） + 中文 WER golden test | S0 | 与 S2/S3/S4 并行 |
| **6** | 1 | **自动更新链路打通**（Tauri updater + 签名 + latest.json gateway）+ apt 仓库搭建 | S0.5 | — |
| **7** | 1 | License key 联调 + 会员配额扣减 + Playwright 全链路 E2E | S6 | — |

并行度高于线性 11 周；理论最优 ~7-8 周。

**首批落地（Sprint 0 + 0.5 + 1，~3.5 周）即可 demo 给律师看**：双击 .exe → 看到 Tauri 窗口 → 拖文件进去 → AI 推荐归类到 Project → 跨证据链联想批注。这是核心价值最快落地路径。

---

## 10. 测试策略

沿用 `docs/TESTING.md` 六层金字塔，新增律师 corpus：

```bash
tests/corpora/law/
├── 真实劳动合同样本-2024.zip      # 公开样本（脱敏）
├── 公开判决书-2023-2024.zip      # 中国裁判文书网公开判决（commit 固化）
├── 民法典全文-2020.md            # 全国人大公开
└── golden/
    ├── contract_review_evidence_chain.json  # 跨证据链推理 golden
    ├── chat_trigger_router.json             # intent router precision
    └── asr_chinese_wer.json                 # 中文 ASR 准确率回归
```

### E2E 关键路径

Playwright 走完一遍**核心场景**：
1. 安装 → unlock vault → 检测到首次启动 → 三档默认选择
2. 上传起诉状 PDF → AI 抽实体 → 浮气泡"要建 Project 吗" → 用户确认
3. 上传借条照片 → OCR → 跨证据链推理 → 批注侧栏出现"与第 1 份合同金额一致"
4. chat 输入"帮我审查这份合同" → IntentRouter 命中 contract_review → 弹 confirm → AI 输出风险清单
5. 文件上传后浮气泡"要查被告工商信息吗" → 用户勾选 → RPA 调 gsxt（v0.7）或 flk_npc（v0.6 demo）
6. 浏览 flk.npc.gov.cn 一篇法条 → 扩展浮窗 → 归档到当前 Project
7. 用户敲入 ¥99/月会员 license key → 配额展示

---

## 11. 风险与未决问题

### 已识别风险

| 风险 | 缓解 |
|:---|:---|
| Windows EV Code Signing 周期长（首次 7-14 天 SmartScreen 信誉冷启动） | v0.6 走 alpha 内测渠道；正式版必须留 2 周 buffer |
| RPA 站点反爬变化（pkulaw / 裁判文书网随时改 selector） | 抽 selector 到 plugin.yaml，热更新；建立 RPA 健康监控 + 自动报警 |
| 中文 WER 小模型不达标 | 实测后再决定默认；不达标的硬件提示"建议上 K3 一体机" |
| 首批律师试用反馈差（Project AI 推荐归类不准） | v0.6 alpha 只发 ≤ 20 个律师；准确率 < 70% 不进 GA |

### 未决问题（v0.7 之前要拍）

- **远端 LLM gateway 谁建？**自营（attune.ai 走 OpenRouter 风格代理 + 国内支付）vs 用户自带 API key（让用户自己上 DeepSeek 等）
- **K3 一体机销售渠道**：直营 / 京东 / 找硬件代工厂？
- **是否需要 lawcontrol 互通的 export/import 格式？**（用户主动触发型）

---

## 12. 与既有 attune-pro 商用仓的关系

attune-pro 现有 9 capabilities（5 law + 4 presales）按本 spec 升级：
- 加 `chat_trigger` 字段 → Intent Router 可路由
- 加 `context_strategy.scope: project` → 跨证据链联想自动注入
- 加 `needs_confirm: true`（关键 skill 用 LLM 前确认）
- 加 attune-law plugin 把 Project 渲染为 Case

**不重写**，只加配置 + 升级。预计 attune-pro 这部分工作量 1-2 天。

---

## 13. 验收清单（v0.1 GA Definition of Done）

### 桌面体验（Tauri Desktop）

- [ ] 双击 `Attune.exe` / `Attune.AppImage` ≤ 30 秒看到主窗口（含 axum 启动）
- [ ] 关闭主窗口 = 最小化到系统托盘（不退出进程）
- [ ] 单实例锁：重复双击只激活已有窗口
- [ ] 拖文件到窗口 → 自动进入上传流
- [ ] RPA 任务完成 → 系统原生通知

### 安装与更新

- [ ] Win NSIS/MSI + Linux deb + AppImage + aarch64 deb 四个产物从 CI 一键出
- [ ] 安装包不含 LLM 模型，但含 Ollama runtime + bge-small + whisper-small Q8 + tesseract chi_sim（共 ~150 MB）
- [ ] Tauri updater 自动检查 `https://updates.attune.ai/desktop/stable/latest.json` 通过
- [ ] `latest.json` 和 `.msi/.deb` 双重 Ed25519 签名，验签失败拒绝安装
- [ ] patch / minor / major 三档更新策略可配置（Settings UI）
- [ ] 回滚机制：3 次启动 5 秒内 crash 自动建议回滚，保留上一版二进制
- [ ] vault schema 自动迁移（v0.5 → v0.6 升级测试通过）

### 行业版核心价值

- [ ] Project / Case 数据模型 + AI 推荐归类（准确率 ≥ 70% 在 20 律师样本上）
- [ ] Intent Router 路由 9 个 attune-pro skill 准确率 ≥ 85%
- [ ] flk_npc + wechat_article 两个 RPA 走完异步后台 + 顶栏进度面板 + 错误恢复
- [ ] 跨证据链推理 workflow 在律师 corpus golden test 上 precision@3 ≥ 0.6
- [ ] 中文 ASR WER ≤ 20%（whisper-small Q8）
- [ ] 扩展行业化模板（≥ 5 个白名单域名）+ 三档默认

### 商业化与合规

- [ ] License key 离线校验 + 配额扣减 + grace period
- [ ] Playwright 7 步关键路径全过（详见 §10）

### Headless 兼容（双轨）

- [ ] `attune-server-headless` bin 可独立运行（不依赖 Tauri）
- [ ] 浏览器访问 :18900 等价于 Tauri 内 webview（API 一致）
- [ ] aarch64 deb 包可在 K3（192.168.100.209）上 `apt install` 成功

---

## 14. 实施前提

- 本 spec 通过用户 review（待）
- 调用 `superpowers:writing-plans` 出每个 Sprint 的实现 plan
- 每个 Sprint 用 `superpowers:subagent-driven-development` 执行
- 用 `superpowers:using-git-worktrees` 隔离开发分支

— end of spec —
