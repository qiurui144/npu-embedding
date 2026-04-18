# Attune 数据基础设施 — 自动备份 · DB Migration · 崩溃上报

**Date:** 2026-04-19
**Status:** 待实施
**Scope:** Attune Rust 服务端（`rust/crates/attune-core` + `rust/crates/attune-server`）数据/进程可靠性底座
**Parallel to:** `2026-04-19-frontend-redesign-design.md` · `2026-04-19-ux-quality-design.md`
**Part of:** "产品级基础框架" 系列 3 个并行 spec 中的 2a

---

## 0. 背景与动机

### 问题

Attune 作为个人知识库，用户把 3 个月到 3 年的专业知识积累存在里面。当前存在三个**灾难级**风险：

| 风险 | 当前 | 后果 |
|------|------|------|
| **数据丢失** | 只有手动 `profile/export`，用户不点就零备份 | 磁盘故障 / 误删 / vault 损坏 → 所有积累归零 |
| **schema 升级炸老用户** | 改表结构直接 `CREATE TABLE IF NOT EXISTS`，老库字段对不上 | 升级后 vault 打不开或数据错乱 |
| **崩溃无痕** | 服务端 panic 只打 stderr；客户端 JS error 无处去 | 用户反馈 "偶尔崩"，我们无法定位问题 |

### 目标

本 spec 落地三个数据基础设施子系统：

1. **A1 自动备份** —— 每日自动备份，保留 7 日+4 周，一键恢复
2. **A2 DB migration 框架** —— `PRAGMA user_version` 驱动，失败自动回滚
3. **A3 崩溃上报** —— 服务端 panic + 客户端 error 结构化捕获，本地持久

### 非目标

- **不做**远程崩溃上报（Sentry 等）—— 违背 "私有 AI 知识伙伴" 定位
- **不做**实时增量备份（WAL shipping 之类）—— 个人场景每日快照足够
- **不做**跨设备自动同步 —— 那是独立 spec（涉及云端决策）
- **不做**云端备份存储 —— 本地备份 + 用户手动上云（NAS / iCloud 同步盘）

---

## 1. A1 · 自动备份子系统

### 备份内容

| 数据 | 来源 | 备份方式 |
|------|------|---------|
| vault.db（主 SQLite） | `~/.local/share/attune/vault.db` | WAL checkpoint + 文件拷贝 |
| tantivy 索引 | `~/.local/share/attune/tantivy/` | 整目录拷贝 |
| usearch HNSW 索引 | `~/.local/share/attune/vectors/` | 文件拷贝 |
| device secret | `~/.local/share/attune/device_secret` | 文件拷贝（加密已存在） |
| app settings | 已在 vault.db 的 `vault_meta` 表，跟主库一起 | — |
| plugin YAMLs（用户自定义） | `~/.local/share/attune/plugins/` | 整目录拷贝 |

**不备份**：
- `logs/`（日志可再生）
- `crashes/`（崩溃快照自成独立文件）
- `cache/`（embedding 队列临时文件等）

### 备份位置

- 默认：`~/.local/share/attune/backups/`（可在 Settings 改路径）
- Windows：`%APPDATA%\attune\backups\`
- macOS：`~/Library/Application Support/attune/backups/`

### 备份格式

每次备份产出一个 zip 文件 + 一个 manifest：

```
backups/
├── daily-2026-04-19.zip
├── daily-2026-04-19.manifest.json
├── daily-2026-04-18.zip
├── daily-2026-04-18.manifest.json
├── ...
├── weekly-2026-W16.zip
└── weekly-2026-W16.manifest.json
```

Manifest：

```json
{
  "version": 1,
  "backup_type": "daily|weekly|manual",
  "created_at": "2026-04-19T03:17:42Z",
  "attune_version": "0.6.0",
  "schema_version": 7,
  "files": [
    { "path": "vault.db", "size": 45678901, "sha256": "ab..." },
    { "path": "tantivy/meta.json", "size": 1234, "sha256": "cd..." },
    ...
  ],
  "total_bytes": 123456789,
  "item_count": 3421,
  "session_count": 47,
  "annotation_count": 892
}
```

### 备份调度

**daily 备份**：
- 启动时检查：若今天 `backups/daily-YYYY-MM-DD.zip` 不存在 → 调度到当日"静默时间"运行
- 静默时间：用户在 Settings 配（默认凌晨 3:00 本地时间）
- 若静默时间已过，在下次应用**空闲 >5 分钟**时触发
- 备份期间不阻塞其他操作（用只读 SQLite 连接拷贝 + WAL checkpoint）

**weekly 备份**：
- 每周日 daily 备份完成后自动 promote 为 weekly（硬链接或拷贝）

**manual 备份**：
- Settings > 数据 > "立即备份" 按钮
- CLI `attune backup --out /path/to/backup.zip`

### 保留策略（GFS: Grandfather-Father-Son 变种）

- 最近 **7 日**的 daily
- 最近 **4 周**的 weekly（周日的那一份）
- 可配置上限（默认 11 份，约 1GB 级别）

每次新备份后，删除超出保留窗口的老备份。

### 增量 / 全量策略

- **每份备份都是全量**（zip 全部文件）
- 理由：
  - 个人 vault 典型 <2GB，全量 zip 几分钟完成
  - 增量恢复复杂度飙升，不值（SQLite `VACUUM INTO` 也没法完全解决）
  - zip 压缩对 SQLite / tantivy 有 2-3x 压缩率，实际空间不大

### 备份操作原子性

```
1. 创建 temp dir `backups/.tmp-daily-2026-04-19/`
2. 对 vault.db 做 WAL checkpoint + `sqlite3_backup_init/step/finish`（热备份 API）
   → temp dir 里得到 vault.db 的一致性快照
3. 对 tantivy 目录 rsync 到 temp dir（只读文件，可直接拷贝）
4. 对 usearch 索引同上
5. 计算 manifest（遍历 + sha256 每个文件）
6. zip 打包 temp dir → `backups/.daily-2026-04-19.zip.partial`
7. `rename(.partial → daily-2026-04-19.zip)` （原子 rename）
8. 写 manifest 文件
9. 删 temp dir
10. 删旧备份
```

失败任一步 → 清 temp dir + 丢弃 partial + log warning。**绝不**覆盖已有好备份。

### 恢复

CLI：

```bash
attune restore --backup /path/to/daily-2026-04-19.zip
```

流程：
1. 解压到 temp dir
2. 验证 manifest 的 sha256
3. 若校验失败 → 退出，报告损坏文件
4. 暂停 attune-server（如运行中）
5. 把当前 `~/.local/share/attune/` 重命名为 `...attune.broken-YYYYMMDD/`（保留，不删）
6. 把恢复目录 move 到 `~/.local/share/attune/`
7. 启动 attune-server
8. 校验：vault 能 unlock、item_count 匹配 manifest

Web UI：
- Settings > 数据 > 恢复
- 列出所有可用备份（ranked by date）
- 点某份 → 弹确认（"当前数据将被 move 到 broken 目录，可人工恢复"）→ 执行

### 备份完整性定期校验

- 每周一次，随机选一份备份 → 只解压到 temp 目录 → 验证 manifest sha256 → 不实际恢复
- 失败 → sidebar 黄色 banner 提示："2026-04-19 备份校验失败，建议重新生成"

### 用户体验

**Settings > 数据 > 备份**：
- 静默时间 picker
- 保留数量（daily 7 / weekly 4）
- 备份路径（默认 + 改路径）
- 列表显示最近备份：日期 + 大小 + 状态 (✓ OK / ⚠ 校验失败 / 🔄 进行中)
- 每条：[恢复] [验证] [删除] 操作
- 顶部 CTA："立即备份"

**Sidebar 底部**：
- 显示上次备份时间（悬停显示完整）
- >48h 无新备份 → 黄色警告

### 配置项

```json
"backup": {
  "enabled": true,
  "daily_time": "03:00",        // HH:MM 本地时间
  "retention_daily": 7,
  "retention_weekly": 4,
  "path": null,                  // null = 默认路径
  "verify_weekly": true
}
```

### 实现

- 新模块：`attune-core/src/backup.rs`（~400 行）
- 新 CLI 命令：`attune backup` + `attune restore`
- 新 API：
  ```
  POST /api/v1/backup           body: {} -> 触发即时备份，返回 task_id (进度推 WS)
  GET  /api/v1/backup           -> 列所有备份
  POST /api/v1/backup/verify    body: { id } -> 校验一份
  DELETE /api/v1/backup/:id     -> 删一份
  POST /api/v1/backup/restore   body: { id } -> 恢复 (同步阻塞，完成后返回)
  ```

---

## 2. A2 · DB Migration 框架

### 设计原则

1. **版本号驱动**：SQLite `PRAGMA user_version = N`
2. **migrations 单调递增**：001, 002, 003, ...
3. **每个 migration 是 SQL 脚本**（嵌入二进制 via `include_str!`）
4. **启动时自动应用**：若 `user_version < 当前 N`，按序跑剩余 migrations
5. **原子性**：每个 migration 在事务里跑；失败回滚到 migration 前的 snapshot
6. **前移植备份**：应用 migration 前自动触发一次 manual 备份（防灾难）

### 目录结构

```
rust/crates/attune-core/src/migrations/
├── mod.rs              # 框架代码
├── sql/
│   ├── 001_initial.sql         # 初始 schema（等同当前 CREATE TABLE 集合）
│   ├── 002_annotations.sql     # 加 annotations 表（批注系统）
│   ├── 003_chunk_summaries.sql # 加 chunk_summaries 表（上下文压缩缓存）
│   ├── 004_skill_signals.sql   # 加 skill_signals 表（技能进化）
│   └── ...
```

### 核心 API

```rust
// migrations/mod.rs
pub fn run_migrations(conn: &mut Connection) -> Result<AppliedMigrations> {
    const MIGRATIONS: &[(u32, &str, &str)] = &[
        (1, "initial", include_str!("sql/001_initial.sql")),
        (2, "annotations", include_str!("sql/002_annotations.sql")),
        (3, "chunk_summaries", include_str!("sql/003_chunk_summaries.sql")),
        (4, "skill_signals", include_str!("sql/004_skill_signals.sql")),
    ];

    let current_version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let target_version = MIGRATIONS.last().map(|(v, _, _)| *v).unwrap_or(0);

    if current_version == target_version {
        return Ok(AppliedMigrations::empty());
    }
    if current_version > target_version {
        return Err(VaultError::Migration(format!(
            "DB version {} is newer than binary supports {}; please upgrade Attune",
            current_version, target_version
        )));
    }

    // 备份（灾难恢复兜底）
    backup::backup_now("pre-migration").ok();  // 非关键失败不阻塞

    let mut applied = Vec::new();
    for (version, name, sql) in MIGRATIONS {
        if *version <= current_version { continue; }
        let tx = conn.transaction()?;
        tx.execute_batch(sql).map_err(|e| VaultError::Migration(
            format!("migration {} ({}) failed: {}", version, name, e)
        ))?;
        tx.execute(&format!("PRAGMA user_version = {}", version), [])?;
        tx.commit()?;
        applied.push((*version, name.to_string()));
        tracing::info!("Applied migration {}: {}", version, name);
    }
    Ok(AppliedMigrations { applied, from: current_version, to: target_version })
}
```

### 回滚策略

**不支持自动 down migration**（过度设计）。若 migration 失败：

1. 事务回滚保证 DB 一致性（migration 里的 DDL 在某些 SQLite 版本可能不事务化，但 DDL 失败通常 DB 不可用）
2. pre-migration backup 可恢复到 migration 前状态
3. 错误信息清晰："migration 5 (add_plugins) failed at line 42: column X already exists"
4. 用户选项：
   - 恢复备份 → 当前 binary 不能用此 vault，需降级二进制
   - 人工编辑 SQL → 一般不推荐
   - 提 issue 带 crash report

### migration 编写规范

**强约束**：
- 每个 migration 只做一件事（加表 / 加列 / 改索引）
- 文件名严格按序：`001_xxx.sql`, `002_xxx.sql`
- 不删已 ship 的 migration（会破坏幂等）
- 未 ship 的 migration 可改动；ship 之后只能加新 migration 打补丁

**编写示例**：

```sql
-- 005_add_plugin_install_history.sql
-- 新增插件安装历史表（从 PluginHub 下载的插件记录）

CREATE TABLE IF NOT EXISTS plugin_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    installed_at TEXT NOT NULL DEFAULT (datetime('now')),
    source TEXT NOT NULL DEFAULT 'pluginhub',
    sha256 TEXT NOT NULL,
    UNIQUE(plugin_id, version)
);

CREATE INDEX IF NOT EXISTS idx_plugin_history_id ON plugin_history(plugin_id);
```

### 测试

每个新加的 migration 必须有：
- Unit test：空库跑到最新 → 检查 schema 正确
- Unit test：老版本（version = N-1） → 运行 → 到 version = N 无错
- Golden：某固定版本的 vault.db fixture，能一路升级到最新

### UI 表现

- **启动期间**：health 返回 `{"status": "starting", "migrating": true, "progress": "3/5"}`
- 前端 splash 显示："Attune 升级中（3/5）"，typical 几秒
- 完成 → 正常进入
- **失败时**：返回 `{"status": "down", "error": "..."}`
- 前端显示专属 "升级失败" 全屏页，含错误详情 + 恢复备份 CTA + 诊断导出 CTA

### 现有数据的初始化

```
001_initial.sql 包含当前所有 CREATE TABLE 语句（从 store.rs 的 SCHEMA_SQL 抽取）。
首次启动空库 → user_version=0 → 跑 001 → 设 user_version=1。
已有 vault（当前已 ship 的）：视为已在 user_version=N（需 detect 或补一个"legacy import"migration）。
```

**兼容已 ship 用户**：
- 首次启动时 detect：若 `user_version=0` 但 `items` 表已存在 → 视为老库（未使用 migration 框架）
- 跑一个 `000_legacy_import.sql`（no-op，只 SET `PRAGMA user_version=4`），让后续 migration 从 4 开始
- 新用户直接从 001 开始

---

## 3. A3 · 崩溃上报

### 设计原则

- **本地优先**：所有崩溃信息写本地文件，不外传
- **结构化**：JSON 格式便于后续导出到诊断包
- **隐私**：除用户显式 opt-in 外，绝不通过网络发送
- **配套**：和 B3 诊断包导出配合（用户可选择附带崩溃日志发给支持）

### 服务端 panic 捕获

```rust
// attune-server/src/main.rs
use std::panic;

fn install_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let location = info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".into());
        let message = info.payload()
            .downcast_ref::<&str>().map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string payload>".into());
        let backtrace = std::backtrace::Backtrace::force_capture();

        let report = CrashReport {
            timestamp: chrono::Utc::now().to_rfc3339(),
            kind: "panic",
            location,
            message,
            backtrace: backtrace.to_string(),
            version: env!("CARGO_PKG_VERSION").into(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            // 不包含：vault password、api keys、用户内容
        };

        let path = crashes_dir().join(format!("panic-{}.json", report.timestamp));
        let _ = std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap());

        // 继续走默认 hook（输出到 stderr，供 systemd 记录）
        default_hook(info);
    }));
}
```

### 服务端其他错误捕获

非 panic 但严重的错误（数据库连接失败、索引损坏等）通过 `tracing` 记录 + 额外写 crash report：

```rust
pub fn report_error(kind: &str, err: &dyn std::error::Error, context: &str) {
    let report = CrashReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        kind: kind.into(),  // "db-error" | "index-corrupt" | "ollama-oom"
        message: err.to_string(),
        context: context.into(),
        ...
    };
    let _ = std::fs::write(crashes_dir().join(...), ...);
}
```

### 客户端 error 捕获

```ts
// ui/src/store/error-capture.ts
window.addEventListener('error', (e) => {
  reportClientError({
    kind: 'uncaught-error',
    message: e.message,
    filename: e.filename,
    line: e.lineno,
    column: e.colno,
    stack: e.error?.stack,
    user_agent: navigator.userAgent,
    url: location.href,
    timestamp: new Date().toISOString(),
  });
});

window.addEventListener('unhandledrejection', (e) => {
  reportClientError({
    kind: 'unhandled-rejection',
    message: String(e.reason),
    stack: e.reason?.stack,
    ...
  });
});

// 不阻塞业务：失败静默
async function reportClientError(report: ClientErrorReport) {
  try {
    await fetch('/api/v1/client-errors', {
      method: 'POST',
      body: JSON.stringify(report),
    });
  } catch { /* ignore */ }
}
```

### 服务端存储

```
~/.local/share/attune/crashes/
├── panic-2026-04-19T03-17-42Z.json       # 服务端 panic
├── db-error-2026-04-19T10-22-11Z.json    # 服务端其他错误
├── client-2026-04-19T15-34-07Z.json      # 客户端 JS error
└── ...
```

新增 API：
```
POST /api/v1/client-errors   body: ClientErrorReport  -> 保存
GET  /api/v1/crashes         -> 列最近 N 条
GET  /api/v1/crashes/:id     -> 详情
DELETE /api/v1/crashes       -> 清空
```

### 保留 & 轮转

- 每种 kind 最多保留最近 **100 条**
- 超过 → 删最早的
- 手动清空：Settings > 诊断 > "清空崩溃日志"

### Redaction（脱敏）

崩溃上报**不包含**以下字段（白名单写入）：
- vault password / master password
- api keys (OpenAI/Anthropic tokens)
- 用户知识内容（items 内容、chat messages）
- file 绝对路径（可能含用户名 → 做 `/home/{user}/... → /home/<user>/...` 替换）

### UI 表现

**Settings > 诊断 > 崩溃日志**：
- 列表显示最近 100 条
- 每条：时间 + kind + message 首行
- 点开详情 → JSON pretty view
- 顶部：`[清空]` `[导出诊断包]`（B3 功能）

**用户提示**：
- 发生服务端崩溃 → systemd 重启后首次启动 → 右下角 toast："检测到上次异常退出，[查看详情]"
- 客户端 JS error → 不打扰（太频繁会烦），仅日志

### 测试

- Unit：触发假 panic，检查 crash JSON 写入正确
- Unit：redaction 覆盖关键字段（password 等）
- Integration：runtime 发一个未捕获 Promise reject → 客户端 `/api/v1/client-errors` 收到

---

## 4. 跟 B3 诊断包导出的关系

B3（诊断包导出工具）在基础框架 spec 系列的 **2c** 中完整实施，但 2a 必须**预留接口**：

诊断包内容（B3 规划）：
1. 最近 100 条崩溃日志（本 spec 产出）
2. 最近 1 小时 server logs
3. settings（api keys redacted）
4. 系统信息（OS + CPU + RAM）
5. 最近 7 次 migration 记录
6. Attune 版本 + 依赖版本（Cargo.lock hash）
7. 备份列表（不含备份内容，只 manifest）

2a 提供端点：
```
GET /api/v1/diagnostic-data   -> 返回上述所有数据的 JSON
```

B3（2c）会增加前端 UI 把这份 JSON + zip 一起打包给用户。

---

## 5. 测试策略

### A1 备份

- Unit：backup::create_daily() 产出有效 zip + manifest（sha256 正确）
- Unit：restore 验证 sha256 校验，损坏文件时拒绝
- Integration：完整 backup → restore 回到原样，item_count 一致
- Integration：retention 保留策略（30 天后只剩 7 daily + 4 weekly）

### A2 Migration

- Unit：每个 migration（N-1 → N）的 schema 变化符合预期
- Unit：重复运行 migration 幂等（第二次 no-op）
- Unit：失败 migration 事务回滚到 pre-migration 状态
- Integration：从版本 1 一路升级到最新，数据完整保留
- Golden：固定 vault.db fixture 升级后 item 内容不变

### A3 崩溃

- Unit：panic hook 写入文件路径 + 内容正确
- Unit：redaction 不泄露敏感字段
- Integration：客户端 uncaught error → `/api/v1/client-errors` 收到并落盘
- Manual：触发真实 panic，systemd 重启后 UI 正确提示

---

## 6. 成功标准

### 功能验收

- [ ] 启动 24h 后自动产生一份 daily 备份
- [ ] 连续 30 天后 backups 目录内有 7 份 daily + 4 份 weekly
- [ ] `attune restore --backup <zip>` 完整恢复，item_count 与备份前一致
- [ ] 升级 Attune（比如 v0.6 → v0.7）时自动应用 migration，用户无感知
- [ ] Migration 失败时 UI 有清晰错误 + 恢复建议
- [ ] Panic 后 `crashes/panic-*.json` 文件存在 + 内容无敏感信息
- [ ] 客户端 error → 服务端本地存储（但不外传）

### 性能指标

- 备份 1GB vault 耗时 <2 分钟（SSD，不压缩过狠）
- Migration 典型 <2 秒（DDL 主导）
- 崩溃报告写盘 <10ms（panic 发生后）

### 稳定性指标

- 备份期间 attune-server 仍可接受读请求（只读快照）
- Migration 失败自动恢复 pre-migration 备份（零数据丢失）
- 100 次 panic 压力测试，每次都有对应 crash 文件 + systemd 重启

---

## 7. 范围外（其他 spec 推进）

- **B3 诊断包导出 UI**（在 2c）
- **B8 telemetry opt-in**（在 2c，完全独立于 crash reporter）
- **远程崩溃上报 / Sentry 集成** → 永不做（违背定位）
- **跨设备备份同步** → 独立 spec（涉及云端决策）

---

## 8. 开放问题

1. **Migration 失败时要不要尝试"跳过"**？
   - 当前设计：强失败，用户手动恢复备份
   - 备选：提供 `--skip-failed-migration` CLI flag 跳过
   - **决策**：不做（跳过容易导致数据不一致隐患），保持严格模式

2. **备份压缩强度**？
   - zip default（level 6）平衡速度和体积
   - 不做 zstd（引入依赖，收益不显著）

3. **恢复操作要不要"预演" dry-run 模式**？
   - 当前设计：直接执行 + 旧数据保留到 `.broken` 目录
   - 备选：dry-run 只解压不切换
   - **决策**：当前方案 sufficient（broken 目录保留是实质的 undo）
