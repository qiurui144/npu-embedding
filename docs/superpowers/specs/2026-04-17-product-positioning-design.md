# npu-vault 产品定位重设计

**日期**：2026-04-17
**作者**：产品定位 brainstorming 流程产出
**状态**：待实施
**范围**：Rust 商用线 npu-vault（不涉及 Python 原型线）

---

## 0. 背景与动机

### 为什么要重设定位

过去的定位核心词是 "**1Password 式加密 + 本地优先知识库引擎**"。这在产品早期（只有加密存储 + 基础搜索）成立，但随着近几个里程碑上线（专利插件、网络搜索 fallback、SkillClaw 风格自动技能进化），老定位无法覆盖：

1. **产品不再是被动仓库** —— SkillEvolver 会从失败查询中自学，主动优化检索
2. **产品不再是纯工具** —— RAG Chat + 引用 + 会话持久化已经是主交互，跟"存取"心智不同
3. **产品不再纯本地** —— 网络搜索 fallback 已落地，"本地优先"不准确，应该是"本地决定、全网增强"
4. **1Password 类比误导** —— 它是凭据保险箱（被动/静态），npu-vault 是知识伙伴（主动/进化）

### 本次重设的目标

- 给出覆盖当前能力的一句话定位、三大支柱、目标用户、典型场景
- 明确"零锁定付费"原则作为底层承诺
- 产出 README / 仓库文档的具体改动清单
- 同步规划：把 `WebSearchProvider` 的默认实现从付费 API 改为浏览器自动化

---

## 1. 核心承诺

### 产品名
**npu-vault**（暂不改名，后续可再议）

### 一句话定位
> **私有 AI 知识伙伴** — 本地决定，全网增强，越用越懂你的专业。
>
> *Your private AI knowledge companion — locally owned, globally augmented, increasingly attuned to your expertise.*

### 产品承诺（约 60 字）
> npu-vault 是为知识密集型专业人士打造的私有 AI 知识伙伴。你的专业领域它会越用越懂；本地知识够用时在本地决定，不够用时主动上网补全；所有数据加密存在你自己的设备上，换设备、换工作都能带走。

### 情感钩子（按重要性）

1. **伙伴关系** — 不是每次打开都陌生的工具，而是记得你研究过什么、习惯怎么问问题、领域常识越积越厚的长期搭档
2. **主权** — 你的知识是你的资产，加密存在你设备上，不被平台锁定
3. **进化** — 不需要你配置，产品自己从每次交互中学习

---

## 2. 三大支柱

### 支柱 1：主动进化（Active Evolution）
> 「它从每次查询中学习，不需要你配置。」

- **能做什么**：本地知识库没命中的查询会自动沉淀为"失败信号"；后台 `SkillEvolver` 每 4 小时或累积 10 条信号时触发，用 LLM 分析主题并生成同义词扩展，静默写入配置
- **用户感知**：用三个月后搜同一个词，结果比第一天准得多；整个过程没有任何"训练"按钮或提示弹窗
- **底层支撑**：SkillClaw 启发的 Summarize → Aggregate → Execute 三阶段流水线；`learned_expansions` 静默生效

### 支柱 2：对话伙伴（Conversational Companion）
> 「跟它讨论，不是跟它搜索。」

- **能做什么**：RAG Chat 为主界面，每条回答都带可点击的引用源（本地文档或网络结果）；会话持久化并可搜索，三周前的讨论能接着往下问
- **用户感知**：像跟一个长期配合的同事讨论 —— 它记得你上次查过什么、用过什么术语、关注哪些领域
- **底层支撑**：三阶段检索（vector + BM25 → rerank → top-k）+ 动态注入预算 + 加密会话存储

### 支柱 3：混合智能（Hybrid Intelligence）
> 「本地决定，全网增强。」

- **能做什么**：优先在本地知识库检索；本地无结果时自动切换到网络搜索；Chat 回答明确标注"来自本地"或"来自网络"
- **用户感知**：你的专业积累和私事在本地、加密；公开信息现查现用；每个答案都看得到出处
- **底层支撑**：可插拔的 `WebSearchProvider` trait；默认实现为**浏览器自动化**（见 §4），零 API 费用；本地失败自动 fallback；强制来源标记

---

## 3. 底座：主权与透明

不作为支柱（差异化不靠这个，是底线），但写在文档显要位置作为承诺。

### 数据主权

- Argon2id(64MB/3 轮) + AES-256-GCM 字段级加密 + Device Secret 多因子
- 所有数据本地持有，单二进制分发，零运行时依赖
- 换设备通过加密导出/导入无损迁移
- 云端 LLM 可选但非必须（本地 Ollama 全能力）

### 零锁定付费原则

用户只需要支付两样东西：

1. **软件本身** —— npu-vault 的授权费（明码标价，一次性或订阅）
2. **LLM token**（可选） —— 如果你用云端 LLM（OpenAI / Anthropic / 其他），token 是你和模型厂商的事，我们不抽成、不加价、不代收

除此之外，**无任何隐藏费用**：

- 网络搜索默认走浏览器自动化，**不需要 Brave / Tavily 这类搜索 API 密钥**
- Embedding 和 Chat 默认走本地 Ollama，**无云端依赖**
- 存储在你自己的设备上，**没有云端订阅费**
- 所有数据加密可导出，**不锁定**

---

## 4. 网络搜索架构调整

### 现状
当前 `WebSearchProvider` 有三个实现：
- `BraveSearchProvider` —— Brave Search API，2000 次/月免费、超过付费
- `TavilySearchProvider` —— Tavily API，1000 次/月免费、超过付费
- `SearxngSearchProvider` —— 自托管 SearXNG，免费但需用户自建

这与"零锁定付费"原则冲突：如果用户量大，Brave/Tavily 会触发付费；SearXNG 需要额外部署负担。

### 调整后架构

```
WebSearchProvider trait
├─ BrowserSearchProvider   ← 新增，默认方案，零成本
│    复用系统 Chrome（CLAUDE.md 已强制 channel="chrome"）
│    通过 chromiumoxide 驱动，抓取 DuckDuckGo / Google
│    可插拔引擎策略（每个搜索引擎一套 DOM 选择器）
│
├─ SearxngSearchProvider   ← 保留，高隐私用户自托管备选
│
└─ BraveSearchProvider     ← 降级为"Power user 可选"
   TavilySearchProvider        用户显式配置 API key 时才启用
                               不再作为默认推荐
```

### 实现要点

- **Rust 选型**：`chromiumoxide`（async CDP 客户端，成熟库），不引入 playwright 的 Node 依赖
- **默认引擎**：DuckDuckGo HTML 端点（对爬虫友好、反爬压力小）
- **抓取策略**：可插拔 `SearchEngineStrategy`，每个引擎一个模块，改版只改一处
- **失败 fallback 链**：BrowserSearch 失败 → SearXNG（若配置）→ API provider（若配置）→ 返回空
- **反爬考虑**：低频（每查询 1 次、间隔 ≥ 2s）+ 真实 UA + 复用用户本地 Chrome profile（可选）
- **Chrome 依赖**：用户必须装 Chrome；启动时检测，缺失时降级到 API provider 或提示安装

### from_settings 默认值变化

**Before**
```json
"web_search": {
  "enabled": false,
  "provider": "brave",
  "api_key": "",
  "base_url": ""
}
```

**After**
```json
"web_search": {
  "enabled": true,
  "provider": "browser",
  "engine": "duckduckgo",
  "browser_path": null,
  "fallback_provider": "searxng",
  "api_provider": null
}
```

默认即开启、默认零配置可用，契合"零锁定"原则。

---

## 5. 目标用户与典型场景

### 四类用户（按权重）

| 用户 | 日常痛点 | npu-vault 的价值 |
|------|---------|----------------|
| **律师 / 专利代理**（核心） | 每个案件涉及的法条、判例、客户技术交底散落在邮件、Word、网盘；换律所时所有沉淀一夜归零 | 加密本地积累 + 专利/法律插件 + 可携带迁移 |
| **研究员 / 学者** | 读过的论文记不清在哪看过、笔记跟阅读行为脱节、跨课题调研重复劳动 | 对话式检索 + 引用可追溯 + 跨课题知识联动 |
| **独立顾问 / 分析师** | 每个项目客户、行业、方法论都不同，但 60% 的底层知识可复用 | 行业插件 + 主动扩展学习 + 本地 + 网络融合 |
| **AI 重度用户 / 技术 Prosumer** | ChatGPT Memory 不可控、不可导出、不懂领域；想要"私有版 AI 记忆" | 本地加密 + 可插拔 LLM + 自托管 + 数据主权 |

**共通画像**：每天处理大量非结构化信息、知识沉淀是核心资产、对隐私有真实诉求（不是口头的）、愿意接受一点点技术门槛换取长期主权。

### 四个典型场景

**场景 1 — 专利代理做一次 FTO 检索**
上午客户发来技术交底书。代理把文件拖进 npu-vault → 自动分类（patent 插件识别为"技术交底书 / 申请阶段"）→ Chat 提问"这个技术方案在 USPTO 有类似先行技术吗" → 本地历史案卷先匹配 → 没有匹配时自动跳到 USPTO 实时检索 → 回答带完整引用链。**三大支柱全部在场。**

**场景 2 — 律师跨三年持续跟进一类案件**
每次办完一个案件把卷宗导入。半年后再办同类案时，直接 Chat 问"我之前处理过的类似条款争议案里，最后哪种抗辩策略成功率最高" → 本地多年积累 + 引用指向每个具体案卷。**对话伙伴 + 主动进化在场。**

**场景 3 — 研究员深入一篇论文**
下载 PDF 丢进来，问"这篇和我六个月前读的 X 论文在方法论上有什么差异" → 两篇都在本地 → RAG 给出对比 + 原文段落高亮。同时 SkillEvolver 在后台发现该研究员最近几周反复查询"扩散模型采样加速"相关词，自动把 "DPM-Solver / consistency models / rectified flow" 这类术语加入扩展词典。**主动进化 + 对话伙伴在场。**

**场景 4 — 顾问切换项目**
结项后把项目文档归档，不删。三个月后新客户行业不同，但某个"定价策略框架"可复用。提问时 npu-vault 跨项目检索 → 自动识别方法论迁移性 → 给出老项目的引用 + 新领域需补充的信息（自动 web 搜索补齐）。**混合智能 + 对话伙伴在场。**

---

## 6. 文档改动清单

| 文档 | 改动类型 | 核心内容 |
|------|---------|---------|
| `npu-vault/README.md` | **重写头部 + 重排功能列表** | 新 tagline、加入三大支柱章节、按支柱重排功能、新增"目标用户与场景"小节、新增"主权与透明"小节 |
| `README.md`（仓库顶层） | **更新头部** | tagline 同步、删除"1Password 式加密"表述、同步"两条产品线"描述 |
| `CLAUDE.md`（仓库顶层） | **更新产品描述段落** | 删除"1Password 式加密"旧表述、双产品线段落同步 |
| `npu-vault/DEVELOP.md` | **极小改动** | 只在开头项目定位段同步 tagline，技术章节不动 |
| `npu-vault/RELEASE.md` | **不动** | 历史记录保留；下次发版时用新定位词汇写 changelog |

### npu-vault/README.md 头部改写示例

**Before**
```markdown
# npu-vault

**本地优先、端到端加密的个人知识库引擎。** 跨 Linux / Windows / NAS（HTTPS 远程），通过 Chrome 扩展、本地文件扫描、文件上传自动积累知识，让云端 AI 更懂你。

单一静态 Rust 二进制，零运行时依赖，28 MB 含完整 Web UI、TLS 和加密搜索引擎。

## 功能

- **1Password 式加密** — Master Password + Device Secret → ...
```

**After**
```markdown
# npu-vault

**私有 AI 知识伙伴** — 本地决定，全网增强，越用越懂你的专业。

npu-vault 是为知识密集型专业人士打造的本地 AI 知识伙伴。你的专业领域它会越用越懂；本地知识够用时在本地决定，不够用时主动上网补全；所有数据加密存在你自己的设备上，换设备、换工作都能带走。

单一静态 Rust 二进制约 28 MB，含完整 Web UI、TLS 和加密搜索引擎。

## 三大支柱

### 主动进化
它从每次查询中学习，不需要你配置。本地无命中的查询自动沉淀为信号，后台定期让 LLM 分析并生成同义词扩展，静默生效 —— 三个月后搜同一个词结果明显更准。

### 对话伙伴
RAG Chat 为主界面，每条回答带可追溯的引用源；会话持久化并可搜索，跨时间、跨项目的知识能顺着对话接上。

### 混合智能
本地知识库优先；本地无结果时自动通过浏览器网络搜索补充（零 API 费用）；回答明确标注来源。专业积累留在本地、加密；公开信息现查现用。

## 主权与透明

- Argon2id + AES-256-GCM 字段级加密 + Device Secret，所有数据本地持有
- 单二进制分发，零运行时依赖
- 换设备通过加密导出/导入无损迁移
- **你只付两样钱**：软件本身 + 你自己的 LLM token（如果你用云端 LLM）。无中间商、无搜索 API 订阅、无隐藏费用
```

### 功能列表重排原则

当前的扁平功能列表改为按支柱分组：

```markdown
## 核心能力

### 主动进化
- 失败信号自动沉淀 + SkillEvolver 后台进化（4h / 10 条信号触发）
- 查询词自动扩展（learned_expansions 静默生效）

### 对话伙伴
- RAG Chat + 引用源追溯
- 三阶段检索（vector + BM25 → rerank → top-k）
- 会话持久化 + 跨会话知识联动
- HDBSCAN 聚类"回忆"

### 混合智能
- 本地全文 + 向量混合检索
- 浏览器自动化网络搜索（零 API 成本，默认）
- SearXNG 自托管（可选）
- 可插拔 Embedding（Ollama / ONNX）和 LLM（Ollama / OpenAI 兼容端点）
- 领域插件（patent / law / tech / presales + 运行时加载）

### 数据主权与透明
- 加密本地存储（Argon2id + AES-256-GCM + Device Secret）
- 单二进制分发，零运行时依赖
- NAS 模式（HTTPS + Bearer auth）
- 加密导出/导入跨设备迁移
- Chrome 扩展兼容 18 个 API 端点
```

---

## 7. 代码改动清单（伴随文档改动）

### 必须改动

- **新增** `npu-vault/crates/vault-core/src/web_search/browser.rs`
  - 实现 `BrowserSearchProvider`
  - 使用 `chromiumoxide` 驱动系统 Chrome
  - 默认引擎 DuckDuckGo，可插拔 `SearchEngineStrategy`

- **重构** `npu-vault/crates/vault-core/src/web_search.rs`
  - 拆成 mod：`web_search/{mod.rs, brave.rs, tavily.rs, searxng.rs, browser.rs}`
  - `from_settings` 默认 `provider: "browser"`
  - 新增 provider 选择逻辑 + fallback 链

- **Cargo.toml** 新增依赖：`chromiumoxide = "0.5"`（或当前最新版）

### 可选改动（视工作量）

- **测试** 新增 BrowserSearchProvider 单元测试（用本地静态 HTML 模拟搜索结果页）

### 不改动

- `WebSearchProvider` trait 定义（已经对）
- chat 路由中的 fallback 触发逻辑（已经对）
- BraveSearchProvider / TavilySearchProvider 代码保留（降级为 opt-in）

---

## 8. 实施顺序

1. **Phase A（本 spec 通过后）** — 产出 writing-plans 的实施计划
2. **Phase B** — 按计划 subagent-driven 或 inline 执行
3. **Phase C** — 最后统一做 Playwright E2E 验证（README 改动不会破坏 Web UI，主要跑 chat + web search 回归）

---

## 9. 开放问题（非阻塞）

- **产品改名**：`npu-vault` 对外偏技术向（易被误认为 NPU 开发者工具）。若要大规模推广，可考虑更贴近"AI 伙伴"心智的名字。暂不在本次 spec 范围内，保留议题
- **定价**：spec 只声明"软件付费"是合理的，具体金额和订阅/买断制度不在定位文档里解决
- **多语言文档**：目前所有定位文字以中文为主、Tagline 附英文。未来面向海外用户时需要独立的英文文档线
