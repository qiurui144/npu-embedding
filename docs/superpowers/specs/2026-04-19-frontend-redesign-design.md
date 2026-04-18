# Attune Frontend Redesign + Client-side Stability

**Date:** 2026-04-19
**Status:** 待实施
**Scope:** Attune Rust 商用线（`rust/crates/attune-server/`）前端重构
**Supersedes:** Batch 2 UI overhaul（commit `003dfa7`）

---

## 0. 背景与动机

### 当前状态

前一阶段 Batch 2 UI overhaul（顶栏 + 模态 Settings + 模型 chip，`003dfa7`）已把主界面拉到 ChatGPT 风格基线。但用户反馈：

1. **不够专业** —— 仍像"加了装饰的 demo"，不像"日常工作台"
2. **不够动态** —— 纯静态切换，没有成熟产品的 loading / 过渡 / 状态反馈
3. **缺少 onboarding** —— 首次解锁 vault 直接进空界面，用户不知道做什么。用户用"路由器配置"打比方：插上电 → 配置向导 → 配完进入常用界面

### 设计目标

把 Attune 前端升到产品级，参考坐标 **Kimi / Gemini / ChatGPT** 的 LLM 工具气质，设计底座是"**可控 / 可配置 / 可用**"。

### 非目标

- **不**做企业管理后台级的信息密度（Grafana / DataDog）
- **不**做复杂的实时数据可视化
- **不**重构后端 API 除必要端点
- **不**改变产品定位（仍是"私有 AI 知识伙伴"）

### 并行 spec（独立规划）

**基础框架增强** 作为独立 spec 推进（A1 自动备份、A2 DB migration、A4 自动更新、A5 a11y、A6 i18n、B1 签名包、B2 第三方 license、Legal L1/L2 等）。本 spec 里通过**组件设计预留接口**，不阻塞基础框架 spec 的后续落地。

---

## 1. 视觉基调

### 主题策略

- **默认**：浅色（北欧风柔和色系）
- **跟随**：自动读 `prefers-color-scheme`
- **覆盖**：用户在 Settings 切 Light / Dark / Auto
- 所有颜色用 CSS 变量暴露，方便基础框架 spec 后续加入"自定义主题"

### 色板（北欧浅色系）

| 角色 | Light | Dark |
|------|-------|------|
| 背景 | `#F7F8FA`（暖调浅灰） | `#1C1F26`（深蓝灰） |
| 表面 | `#FFFFFF` | `#262A33` |
| 悬停表面 | `#F0F2F5` | `#2E323C` |
| 边框 | `#E5E7EB` | `#363B47` |
| 主文字 | `#242B37` | `#E6E6E6` |
| 次文字 | `#6C7887` | `#9CA3AF` |
| 禁用文字 | `#B4BDC9` | `#5C6578` |
| **品牌强调（Fjord Teal）** | `#5E8B8B` | `#7FA5A5`（提亮保对比度） |
| 强调悬停 | `#4A7272` | `#93B8B8` |
| 成功（苔藓绿） | `#6B9080` | `#85AB9B` |
| 警告（琥珀） | `#D4A574` | `#E5BB8D` |
| 错误（陶土红） | `#C97070` | `#DB8B8B` |
| 信息（薄雾蓝） | `#7A9CC6` | `#94B5DD` |

**参考气质**：Notion 温润 + Linear 克制 + IKEA 柔和饱和度。

### Typography

- 无衬线：`-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei"`
- 等宽：`"SF Mono", Menlo, Consolas`（代码块、Token Chip 数字）
- 六档字号：`12 / 14 / 16 / 18 / 24 / 32 px`
- 行高：正文 `1.5`，标题 `1.2`

### 密度（airy 不 dense）

- 8px 网格对齐
- 按钮高度：`32 / 36 / 44 px`
- 组件内 padding：`8 / 12 / 16 px`
- 组件间 gap：`12 / 16 / 24 px`

---

## 2. 动效分层

### 设计原则

1. **有功能才动** —— 每个动效都对应状态/反馈，零装饰性动画
2. **快** —— 主过渡 150-250ms，比 Gemini 略快
3. **统一 easing** —— 主曲线 `cubic-bezier(0.2, 0, 0, 1)`（平滑减速）
4. **respect reduced-motion** —— 尊重系统设置

### L1 · 微反馈（必做）

- 按钮悬停：背景 120ms 过渡
- 按钮按下：瞬时变深（100ms）
- 焦点环：2px Fjord Teal + 100ms fade
- 输入框 focus：边框色过渡
- 链接悬停：下划线 fade in

### L2 · 转场（必做）

- Tab / 视图切换：fade 200ms + 8px translate
- Modal 打开：`scale 0.96→1.0` + fade，200ms（用 `ease-spring` 轻弹）
- Modal 关闭：反向，150ms
- Sidebar 折叠：width + opacity，250ms
- Drawer slide-in：translateX from right，200ms

### L3 · 加载与状态（必做）

- Skeleton：1200ms pulse 循环
- Spinner：<500ms 短等待
- Progress bar：wizard / 文件上传 / embedding 队列
- 状态点：静态不闪（绿/琥珀/红）

### L4 · 内容动效（选配）

- **Chat 流式打字**（默认开启）：后端非流式，前端模拟逐字 reveal（~30ms/char），给 LLM 响应一个"正在思考"的感知
- **Wizard 检测扫描动画**：Step 3/4 分阶段 tick（"✓ 检测 Ollama... ✓ 发现 qwen2.5:3b..."）
- **Token Chip 数字递增**：300ms 平滑滚动
- **Skill Evolution toast**：右下角 1.5s 自动消失

### L5 · 数据可视化（不做，YAGNI）

- 实时图表、sparkline、3D → 全不做
- 克制 = 专业

### 技术栈

```css
:root {
  --ease-out: cubic-bezier(0.2, 0, 0, 1);
  --ease-spring: cubic-bezier(0.5, 1.5, 0.5, 1);
  --fast: 150ms;
  --base: 200ms;
  --slow: 300ms;
}

@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

**不引入** Framer Motion / GSAP / Lottie。纯 CSS + Preact hooks。

---

## 3. 首次安装向导（5 步 · Router 风）

### 容器

- **全屏 takeover**（不是 modal）
- 背景 `#F7F8FA` + 右上角 radial gradient 到 `#E9EEF2`
- 顶部：左 Attune logo + 中 5 点 stepper + 右 "跳过全部"小字
- 主区：居中白色卡片，`max-width: 640px`，圆角 16px，轻柔阴影
- 底部：左返回 · 右下一步（Fjord Teal 主按钮）

### Stepper

```
  ●━━━━○━━━━○━━━━○━━━━○
 欢迎  密码  AI    硬件   数据
```

- 已完成：Fjord Teal + ✓ 可点回
- 当前：Fjord Teal + 脉冲光晕
- 未来：灰色空心不可点

### Step 1 · 欢迎

- Hero：**Attune · 私有 AI 知识伙伴**
- Sub：本地决定，全网增强，越用越懂你的专业
- 三支柱小卡：🌱 主动进化 · 💬 对话伙伴 · 🔄 混合智能
- CTA：**开始设置**（primary）
- Secondary：**我已有 vault，导入备份** → 走 profile 导入分支

### Step 2 · Master Password（硬门槛）

- 说明：**忘记无法找回**；所有数据本地 Argon2id + AES-256-GCM 加密
- 字段：password (show/hide + 强度条) + confirm
- ☐ 同时生成 Device Secret 导出文件（**默认不勾**）
- 验证：≥12 chars，含字母+数字；弱密码 next 灰禁
- API：`POST /api/v1/vault/setup`

### Step 3 · AI 大脑

三张并排卡（horizontal radio）：

**🟢 本地 Ollama**（推荐）
- L4 扫描动画：`检测 Ollama... ✓ 发现 qwen2.5:3b, bge-m3`
- 未装：显示 `curl -fsSL ollama.com/install.sh | sh` + 复制按钮 + 重新扫描
- 选中后：默认 chat / embedding 模型下拉

**☁ 云端 API**
- Provider：OpenAI / Anthropic / DeepSeek / Qwen / 自定义
- API Key（密码字段，后端 redact）
- Model 名 + Endpoint
- **测试连接**按钮：`POST /api/v1/llm/test`

**💤 暂不配置**（演示模式）
- Chat 禁用，可浏览界面

### Step 4 · 硬件识别 + 模型推荐（最"路由器"的一步）

- L4 分阶段扫描动画：
  ```
  ✓ CPU: AMD Ryzen 7 8845H
  ✓ GPU: Radeon 780M (gfx1103, ROCm)
  ✓ NPU: AMD XDNA 16 TOPS
  ✓ RAM: 26 GB
  ```
- 结果卡（fade in）：推荐 Chat / Embedding / Summary 模型 + 每项有"换模型"按钮
- Chip：☐ 让 Ollama 自动下载（~3GB 后台）—— 触发 `POST /api/v1/models/pull`
- API：`GET /api/v1/diagnostics`（已有）+ `PATCH /api/v1/settings`（已有）

### Step 5 · 第一口知识

三选一卡片：

- **📂 绑定文件夹** · 预览可索引文件数 · **只记录路径，后台慢慢索引**（进度条在 sidebar 底部）
- **📥 导入 .vault-profile** · 拖拽上传 · 预览批注/聚类/会话数量
- **→ 跳过，先看看**

API：`POST /api/v1/index/bind`（记路径，不等索引完成）或 `POST /api/v1/profile/import`。

### 完成页（2 秒自动跳转）

- 居中 Fjord Teal 大勾 scale-in + 光晕
- 文字：**Welcome to Attune**
- 3 秒轮播小 tip：`Cmd+K 搜索` · `左栏底部切换视图` · `选字即批注`
- **2 秒后自动 redirect** 主应用（无需点击）

### 状态持久化

中途关浏览器下次恢复：

```json
{
  "wizard": {
    "complete": false,
    "current_step": 3,
    "llm_configured": true,
    "hardware_applied": false,
    "first_data_chosen": null
  }
}
```

- 启动读 `GET /settings`：
  - `vault_state == sealed` → Step 1
  - `vault_state == locked` → 登录页
  - `vault_state == unlocked + wizard.complete == false` → 回 `wizard.current_step`
  - `wizard.complete == true` → 主应用

---

## 4. 主应用布局（2 栏 chat-first）

### 整体结构

```
┌──────────────────────────────────────────────────┐
│  ┌──────────┐  ┌──────────────────────────────┐  │
│  │ Sidebar  │  │  Main                        │  │
│  │ (280px)  │  │  (flex-grow)                 │  │
│  │          │  │                              │  │
│  └──────────┘  └──────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

### Sidebar（左栏 · 5 区）

```
┌────────────────────┐
│ 🌿 Attune          │ ① 品牌 + 搜索
│ ⌘K Search...       │
├────────────────────┤
│ + 新对话            │ ② New chat CTA
├────────────────────┤
│ 📝 今天             │ ③ 会话列表（按日期分组，滚动）
│ · 专利检索讨论      │   悬停显示完整标题
│ · ...              │   右键菜单：重命名/删除/置顶
│ 📝 昨天             │
│ · ...              │
│ 📝 本周             │
│ · ...              │
├────────────────────┤
│ 📄 条目             │ ④ 次级功能（固定底部）
│ 🔗 远程目录         │   点击切换 main view
│ 📊 知识全景         │   （不打开 modal）
│ ⚙ 设置              │
├────────────────────┤
│ 👤 已解锁 · 本地    │ ⑤ Vault 状态 + 连接状态
│ 📶 Ollama ✓         │   点头像：切模型/导 profile/
│                    │           锁 vault/重跑 wizard
└────────────────────┘
```

- **折叠按钮** 右上角 `«`，折叠后只剩 icon 列（64px）
- **拖拽分隔条** 宽度可调（240-360px）
- 移动端（<768px）：默认隐藏，顶栏汉堡菜单呼出

### Main 区 5 种 view

主区按 `currentView` signal 切换，**不开 modal**：

| View | 内容 |
|------|------|
| `chat`（默认着陆） | 顶栏会话标题 + 模型 chip · 消息流 · 输入框 + Token Chip |
| `items` | 筛选条 + 表格/卡片 toggle · 点一条弹右侧 Reader drawer |
| `remote` | 已绑定目录列表 · 添加/同步/解绑 |
| `knowledge` | HDBSCAN 聚类卡片网格（不做复杂 graph） |
| `settings` | 左 tab + 右内容面板（最复杂视图） |

### Slide-in Drawer（侧滑抽屉 · 单层）

"专注深度交互但不离开 context"：

- Reader：选条目后从右滑入 640px 宽，展示全文 + 批注
- Chat 引用预览：点"📎引用"打开抽屉显示原文
- 批注 composer：drawer 内选字后弹出

特性：不盖 sidebar · 可拖左边界调宽 · ESC 关 · 背景点击关 · **仅一层**（不叠 modal）

### Settings：C 方案（混合）

- **高频开关**放账户菜单（头像点开）：切模型 / 切主题 / 注入开关 / 锁 vault
- **完整 Settings**作为 main view（左 tab + 右面板）
- 点头像菜单"更多设置..." → `currentView = 'settings'`

---

## 5. 技术架构

### 目录结构

```
rust/crates/attune-server/
├── ui/                          # 新增：Preact + Vite 子项目
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── index.html               # Vite 入口
│   ├── src/
│   │   ├── main.tsx
│   │   ├── App.tsx
│   │   ├── wizard/
│   │   ├── layout/
│   │   ├── views/
│   │   ├── components/
│   │   ├── hooks/
│   │   ├── store/
│   │   │   ├── signals.ts
│   │   │   └── api.ts
│   │   └── styles/
│   │       ├── tokens.css
│   │       ├── animations.css
│   │       └── global.css
│   └── dist/                    # Vite 产物（提交到 git）
│       └── index.html
└── src/
    └── lib.rs                   # include_str!("../ui/dist/index.html")
```

### 框架选型

- **Preact**（~3KB）+ **@preact/signals**（~1.5KB）
- **Vite** 构建（开发 HMR + 生产 bundle）
- **vite-plugin-singlefile** 把 JS + CSS + assets 全部 inline 进一个 `index.html`
- **TypeScript** 严格模式

### 构建产物

单文件 `dist/index.html`，预计 150-300KB（gzip 后 40-80KB），继续用 `include_str!()` 编译进 Rust 二进制。

### 开发工作流

| 场景 | 命令 |
|------|------|
| 前端独立开发 | `cd ui && npm run dev`（Vite dev server :5173，HMR） |
| 前后端联调 | `cargo run` + `npm run dev`（前端 proxy :18900 API） |
| 生产构建 | `cd ui && npm run build` → 更新 `dist/index.html` |
| CI 校验 | 强制检查 `dist/index.html` 已更新（否则 PR 拒绝） |

**不用 build.rs 自动化**：保持 Rust/Node 构建解耦。开发者改 UI 后需手动 `npm run build`。CI 里有 gate。

### State 管理：Preact Signals

```ts
// store/signals.ts
import { signal, computed } from '@preact/signals';

// 应用级
export const vaultState = signal<'sealed'|'locked'|'unlocked'>('sealed');
export const wizardState = signal<WizardState | null>(null);
export const settings = signal<Settings | null>(null);
export const hardware = signal<HardwareProfile | null>(null);
export const ollamaStatus = signal<'checking'|'ready'|'missing'>('checking');

// UI 级
export const currentView = signal<View>('chat');
export const sidebarCollapsed = signal(false);
export const theme = signal<'light'|'dark'|'auto'>('auto');
export const drawerContent = signal<DrawerPayload | null>(null);

// 连接层（见 §7）
export const connectionState = signal<'online'|'reconnecting'|'offline'>('online');

// 业务级
export const chatSessions = signal<ChatSession[]>([]);
export const activeSession = signal<string | null>(null);
export const messages = signal<Message[]>([]);
export const items = signal<Item[]>([]);
export const backgroundTasks = signal<Task[]>([]);

// computed
export const canChat = computed(() =>
  vaultState.value === 'unlocked' &&
  (ollamaStatus.value === 'ready' || settings.value?.llm?.endpoint) &&
  connectionState.value !== 'offline'
);
```

### 路由：Signal-based（不引 wouter）

desktop 应用范式，不用 URL 路由：

```tsx
function App() {
  return (
    <>
      {vaultState.value !== 'unlocked'
        ? (wizardState.value?.complete ? <LoginScreen /> : <Wizard />)
        : <>
            <Sidebar />
            <Main />
            <Drawer />
          </>
      }
    </>
  );
}
```

权衡：不能 URL 分享特定会话；浏览器后退无效（可选监听 popstate）。desktop 场景可接受。

### API 层

```ts
const BASE = location.origin;
const token = () => sessionStorage.getItem('attune_token');

export async function apiCall<T>(
  path: string,
  init?: RequestInit & { retry?: RetryPolicy }
): Promise<T> {
  const reqId = crypto.randomUUID();
  return withRetry(reqId, init?.retry, async () => {
    const res = await fetch(`${BASE}/api/v1${path}`, {
      ...init,
      headers: {
        'Content-Type': 'application/json',
        'X-Request-Id': reqId,
        ...(token() && { Authorization: `Bearer ${token()}` }),
        ...init?.headers,
      },
    });
    if (!res.ok) throw new ApiError(res.status, await res.text(), reqId);
    return res.json();
  });
}
```

所有 API TypeScript 类型手写（不 codegen），保持轻量。

### i18n 预留接口（由基础框架 spec 实现）

所有用户可见文案通过 `t(key)` 函数调用。本 spec 内 `t()` 是 identity stub（直接返回 key 对应的中文）：

```ts
// stub 版本
export const t = (key: string, params?: Record<string, string>) => {
  const text = zhMessages[key] ?? key;
  return params ? interpolate(text, params) : text;
};
```

基础框架 spec 里替换为真正的 i18n 引擎（如 `@lingui/core` 或手写轻量）+ `en.json`。

### Bundle 预估

| 项 | 大小 (gzip) |
|----|------------|
| Preact + Signals | ~5 KB |
| App 代码（wizard + 5 views + 组件） | ~30-50 KB |
| CSS tokens + animations + global | ~10-15 KB |
| **总 HTML（inline）** | **~50-80 KB** |

### 测试

- **Unit**：Vitest（Vite 原生）
- **Component**：`@testing-library/preact`
- **E2E**：继续 Playwright + Rust server，前端 DOM 稳定后测试脚本不变

CI 新增 `ui-test` job（Node install + vitest + build）。

### 迁移策略

**一次性全替换**（Attune 未公开发布）：
1. 建 `ui/` 子项目
2. 实施所有 §1-7 功能
3. 全 E2E 回归通过后，**删除 `assets/index.html`**（legacy vanilla）
4. `include_str!` 指向 `ui/dist/index.html`

无 feature flag、无 A/B 过渡期。

---

## 6. API 契约 + 状态流

### 新增 API

```
POST /api/v1/llm/test
  body: { endpoint?: string, api_key?: string, model: string }
  returns: { ok: bool, latency_ms: int, error?: string }
  用途：Wizard Step 3 测试云端 API 连接

POST /api/v1/models/pull
  body: { model: string }
  returns: { task_id: string }
  用途：Wizard Step 4 后台下载模型。进度推 WebSocket
```

### WebSocket 扩展

`/ws/scan-progress` → 通用 `/ws/progress`：

```json
{
  "type": "scan|embed|classify|model_pull|skill_evolve|annotation_ai",
  "task_id": "uuid",
  "progress": 0.47,
  "status": "running|done|failed",
  "message": "已处理 47/100 文件",
  "details": { /* type-specific */ }
}
```

前端全局订阅，各组件按 type 过滤显示。Sidebar 底部右侧汇总"3 个后台任务进行中"。

### Health 端点

```
GET /api/v1/health
  returns: {
    status: "starting|ok|degraded|down",
    vault_state: "sealed|locked|unlocked",
    db_ok: bool,
    ollama: "ready|missing|error",
    disk_space_gb: number,
    uptime_sec: number
  }
```

前端每 5s 调一次（独立通道），驱动连接状态机。

### Wizard 状态持久化

见 §3 "状态持久化"。字段存在 `app_settings.wizard`，不另建表。

### Auth Token

- `vault.unlock` 返回 token（已有）
- 存 `sessionStorage.attune_token`（不是 localStorage，关标签页即失效更安全）
- 每个 `apiCall` 自动附 `Authorization: Bearer ${token}`
- 401 → 清 token + 跳登录页
- vault.lock → 清 token

---

## 7. 客户端稳定性框架

### 第一层：连接状态机

```
        ping 失败
  online ────────→ reconnecting (指数回退 500→1000→2000→...→30s)
    ▲               │
    │ ping 成功      │ 连续 5 次失败 / >30s 未恢复
    │               ▼
    └────────── offline (停止自动重试，等用户触发)
```

UI 表现：
- `online`：sidebar 底部绿点 + "已连接"，正常操作
- `reconnecting`：琥珀 pulse 点 + "重连中..."，**操作不禁用**，请求进队列
- `offline`：红点 + "服务器未响应" + 手动"重试"按钮，只读视图

**从不弹全屏错误 modal**。连接问题永远表现为 sidebar 状态 + 顶部 skeleton。

### 第二层：请求重试矩阵

| 类型 | 例 | 策略 | 失败后行为 |
|------|-----|------|-----------|
| 幂等读 | GET /items, /settings, /diagnostics | 指数回退 5 次（100→200→400→800→1600ms） | 显示 cached 值 + reconnecting UI |
| 非幂等写 | POST /chat, /annotations | 回退 3 次 + 请求 ID 去重 | 队列保存 + 重连自动发送 |
| 大载荷 | POST /upload | 回退 2 次 | 上传队列可恢复（localStorage） |
| 破坏性 | DELETE /items/:id | **不自动重试** | 红字 + 手动重试按钮 |
| 心跳 | GET /health | 固定 5s 独立通道 | 驱动连接状态机 |

所有请求附 `X-Request-Id`（前端 UUID），服务端日志 + 响应都带。

### 第三层：WebSocket 自动重连

```ts
class ResilientWS {
  private ws: WebSocket | null = null;
  private backoff = 500;
  private maxBackoff = 30_000;

  connect() {
    this.ws = new WebSocket(URL);
    this.ws.onopen = () => { this.backoff = 500; };
    this.ws.onclose = () => {
      setTimeout(() => this.connect(), this.backoff);
      this.backoff = Math.min(this.backoff * 2, this.maxBackoff);
    };
    this.ws.onerror = () => this.ws?.close();
  }
}
```

断开期间漏收的 WS 推送，重连后主动拉一次 `GET /diagnostics` 同步状态。

### 第四层：服务端生命周期（Rust）

#### 进程级（分发给用户）

Linux `systemd user service`：

```ini
# docs/systemd/attune-server.service
[Unit]
Description=Attune server
After=network.target

[Service]
ExecStart=/usr/local/bin/attune-server --data-dir %h/.local/share/attune
Restart=on-failure
RestartSec=3
StartLimitBurst=5
StartLimitIntervalSec=60
MemoryMax=4G
CPUQuota=400%

[Install]
WantedBy=default.target
```

Windows/macOS 等价物在后续 Tauri 包装时做（独立 spec）。

#### 进程内

- **graceful shutdown**：SIGTERM 后停止接新请求，30s 内等现有完成
- **panic hook**：顶层 `std::panic::set_hook` 记日志 + 优雅退出（由 systemd 重启）
- **启动就绪门**：`/health` 返回 `status: starting` 直到 vault schema + tantivy + hardware detect 就位（前端看到 starting 显示 splash "Attune 正在启动...（通常 1-2 秒）"）
- **结构化日志**：`tracing` crate JSON 输出到 `~/.local/share/attune/logs/`，rolling 每 10MB，保留 7 份

### 第五层：客户端状态持久化

未送达的写操作不能丢。所有 mutating 请求进 localStorage 队列：

```ts
async function mutate<T>(path: string, body: any): Promise<T> {
  const req = { id: uuid(), path, body, createdAt: Date.now() };
  persistQueue.push(req);
  try {
    const result = await apiCall<T>(path, { method: 'POST', body: JSON.stringify(body) });
    persistQueue.remove(req.id);
    return result;
  } catch (e) {
    // 留队列，重连后 retry loop 处理
    throw e;
  }
}

onAppStart(() => {
  for (const req of persistQueue.getAll()) retryLoop(req);
});
```

**典型场景**：
- 用户发 chat → 断网 → 网回来 → 消息自动发出，UI 提示"离线时已保存，现已发送"
- 批注创建 → 服务崩溃 → systemd 3s 重启 → 用户几乎无感

### 第六层：优雅降级

永远让用户能做点什么：

| 失败维度 | 前端表现 |
|---------|---------|
| Ollama 未装 | Chat 入口禁用 + "装 Ollama 解锁 Chat"，其他功能正常 |
| Vault 锁定 | 只读：看条目但不能 ingest/批注；顶栏 Unlock CTA |
| Embedding 未就绪 | 搜索 fallback 纯 tantivy（全文） |
| 无 Chrome（网络搜索） | Chat 本地无结果时显示 "本地无" 而非错误 |
| 磁盘空间低（<2GB） | Sidebar 底部黄色 banner |
| 分类 worker 挂 | Items 正常显示，标签字段留空 |

### 第七层：可观测性（self-hosted）

- **前端错误上报**：`window.onerror` + `unhandledrejection` → `POST /api/v1/client-errors`（本地服务器记录）
- **Request ID 贯穿**：前端 → Rust → Ollama → DB
- **用户主动导出 log**：Settings > 诊断 > "导出最近 1 小时日志"（zip：server log + client log + settings redacted + system info）

### 第八层：稳定性测试矩阵

| 测试 | 工具 | 场景 |
|------|------|------|
| 网络故障注入 | Playwright `route.abort()` / `route.delay()` | 离线提交 chat / 超时重试 |
| 进程崩溃 | bash `kill -9 server` | 重启期间 UI 表现 |
| WS 抖动 | mock 每 10s 断开 | 重连恢复 |
| 资源枯竭 | rlimit / tmpfs 限磁盘 | 降级 UI |
| 启动竞态 | 浏览器先于 server 打开 | splash 正确 |
| 并发写 | 同一 item 两个批注并发 | 无数据丢失 |

---

## 8. 成功标准

### 功能验收

- 首次启动 vault sealed → 自动进入 Wizard Step 1 ✅
- 5 步 wizard 全部完成 → 自动进主应用 ✅
- Chat 发消息 · 断网 · 恢复 → 消息自动补发 ✅
- 系统进 sleep → 醒来 → 连接 reconnecting → 自动恢复 ✅
- Ollama kill → Chat 禁用有说明 · 其他功能正常 ✅
- 关浏览器中断 wizard → 下次打开恢复到中断处 ✅

### 性能指标

- 首次加载（LCP）< 1.5s（本地服务器 + 80KB bundle）
- Chat 输入 → 响应开始 < 200ms（LLM 调用前的 UI 准备时间）
- 视图切换（chat → items）< 100ms
- Bundle 总大小 gzip < 100KB

### 稳定性指标

- 网络抖动 30s 内 → UI 不报错、请求自动恢复
- server crash → systemd 3s 重启 → UI 恢复连接 < 5s
- 100 次随机操作（fuzz）无 data loss · 无 silent failure

---

## 9. 范围外（单独 spec 推进）

以下在**基础框架增强 spec**（待独立 brainstorm）中规划：

- **A1 自动备份** · **A2 DB migration 框架** · **A4 自动更新机制**
- **A5 键盘导航 + a11y** · **A6 i18n**（本 spec 只预留 `t()` 接口）
- **A7 键盘快捷键说明** · **B1 签名包** · **B2 第三方 license 聚合**
- **B3 诊断包导出**（本 spec §7 有基础，基础框架 spec 完整化）
- **B4 Privacy Policy + ToS** · **B5 In-app 帮助** · **B6 空状态教育内容**
- **B7 社区渠道** · **B8 Telemetry opt-in**
- **L1-L5 法律/合规**

---

## 10. 开放问题

1. **Tauri 桌面版**：当前仍是浏览器 → 本地 HTTP server 模式。Tauri 包装作为独立 spec，影响：systemd 换 Tauri lifecycle / 单 binary 合体 / 跨平台 installer
2. **PWA 离线**：考虑到 server 挂时 UI 能看到 cached 数据（已在 §7 第 5 层做了队列）
3. **可访问性深度**：A5 在基础框架 spec 做，但本 spec 组件设计时预留 ARIA 空间
4. **移动端**：<768px 断点已规划 sidebar 折叠，但完整的移动优化（触摸手势 / 键盘弹出） defer
