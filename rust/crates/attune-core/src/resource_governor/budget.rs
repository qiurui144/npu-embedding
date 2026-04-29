// 资源 budget 数据结构 — 任务级 CPU / RAM / IO 上限定义。

use serde::{Deserialize, Serialize};

/// IO 优先级。仅 Linux 通过 `ioprio_set` 真正生效；
/// 其他平台 best-effort 记录但不强制。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IoPriority {
    Idle,
    BestEffort,
    Realtime,
}

/// 单个后台任务的资源预算。
///
/// 字段语义：
/// - `cpu_pct_max`：**系统全局 CPU 占用阈值**（0.0–100.0）— 当 monitor 采样的全局
///   CPU% 高于此值时，本任务的 `should_run()` 返回 false 让 worker 退让。
///   注意这是"系统忙就让让"协作式语义，**不是单进程占用上限**。多个任务共享
///   同一全局指标，自动避免每任务 budget 累加 > 100% 的失真。详见
///   [`crate::resource_governor::monitor`] 模块头注释。
/// - `ram_bytes_max`：当前进程 RSS 上限。仅作记录与 H5 `attune --diag` 暴露，
///   不直接强制（Rust 无法跨平台限制单线程 RSS）。
/// - `io_priority`：见 [`IoPriority`]。
/// - `throttle_on_exceed_ms`：超 CPU 阈值时的退让时间。
/// - `llm_calls_per_hour`：LLM 调用类任务（SkillEvolution / MemoryConsolidation）的额外门槛，
///   `None` = 不限速。governor 维护一个滑动窗口计数。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Budget {
    pub cpu_pct_max: f32,
    pub ram_bytes_max: u64,
    pub io_priority: IoPriority,
    pub throttle_on_exceed_ms: u64,
    pub llm_calls_per_hour: Option<u32>,
}

impl Budget {
    /// 一个永远不限制的预算 — 仅用于测试或显式禁用治理。
    pub const fn unlimited() -> Self {
        Self {
            cpu_pct_max: 100.0,
            ram_bytes_max: u64::MAX,
            io_priority: IoPriority::BestEffort,
            throttle_on_exceed_ms: 0,
            llm_calls_per_hour: None,
        }
    }
}
