# Attune 人工验收清单

> 自动化测试无法覆盖的人工 UX 验证项。每条都是勾选式可执行步骤。
> 自动化测试见 `docs/TESTING.md`。

## H1 资源治理框架（2026-04-27）

设计稿：`docs/superpowers/specs/2026-04-27-resource-governor-design.md`
用户面文档：`docs/system-impact.md`

### Linux 桌面验证

- [ ] **基线**：Settings → System Impact 默认显示 "Balanced" 档
- [ ] **三档切换**：切到 "Conservative" → 后台 embedding 速度肉眼变慢；切到 "Aggressive" → 速度回升
- [ ] **顶栏 Pause**：开始 100 文件批量 embedding → 点顶栏 Pause → 1 秒内 embedding 队列停止处理（pending count 不再下降）
- [ ] **Resume**：再点 Resume → 处理立刻恢复（pending count 继续下降）
- [ ] **CPU 阈值**：在 Balanced 档跑 100 文件批量 embedding → 同时 `top -p $(pgrep attune)` 观察 → 进程 CPU% 不会持续打满（≤ 50% 大致符合 25% 全局阈值在多核机的反映）
- [ ] **diag 命令**：`attune --diag`（H5 实现后）显示所有已注册 governor 的当前 profile / paused / 最近 sample

### Windows 验证

- [ ] 同上 6 条在 Windows MSI 安装的 Attune 上重跑
- [ ] 资源管理器 → 进程 → attune.exe 的 CPU 列与 diag 输出一致（差距 ≤ 5%）

### 跨场景验证

- [ ] **演示场景**：开 zoom 全屏共享屏幕 → 顶栏 Pause → 演示期间 attune 后台零打扰
- [ ] **全屏游戏场景**（H4 实现后）：启动全屏游戏 → governor 自动降到 Conservative → 游戏 FPS 不受 attune 影响
- [ ] **电池场景**（H4 实现后）：拔电源切电池 → governor 自动切 Conservative → 续航不显著缩短

## 注意事项

- 任何一项失败 → 提 issue + 附 `attune --diag` 输出 + 本机 CPU/核数信息
- "演示场景"是核心，必须每次发版前手动验
