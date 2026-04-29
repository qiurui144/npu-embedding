// TaskGovernor — 单个后台任务的"协作式调度"决策点。
//
// Worker loop 在每次迭代头部调用 [`TaskGovernor::should_run`]，
// 在每次工作完成后调用 [`TaskGovernor::after_work`] 决定下次 sleep 时长。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::budget::Budget;
use super::monitor::{ResourceMonitor, Sample};
use super::profiles::{Profile, TaskKind};

/// LLM 调用滑动窗口大小（以小时为单位记数）。
const LLM_WINDOW_SECS: u64 = 3600;

/// 单任务调度器。Clone 友好：内部全是 `Arc<...>`。
pub struct TaskGovernor {
    pub kind: TaskKind,
    profile: Arc<RwLock<Profile>>,
    budget: Arc<RwLock<Budget>>,
    paused: Arc<AtomicBool>,
    monitor: Arc<dyn ResourceMonitor>,
    last_sample: Arc<Mutex<Sample>>,
    /// LLM 调用时间戳滑动窗口（单调时间，秒）。
    llm_calls: Arc<Mutex<VecDeque<u64>>>,
    /// 用于计算 `Sample::captured_secs` 的基准。
    start: Instant,
}

impl TaskGovernor {
    pub fn new(
        kind: TaskKind,
        profile: Profile,
        monitor: Arc<dyn ResourceMonitor>,
    ) -> Self {
        let budget = profile.budget_for(kind);
        Self {
            kind,
            profile: Arc::new(RwLock::new(profile)),
            budget: Arc::new(RwLock::new(budget)),
            paused: Arc::new(AtomicBool::new(false)),
            monitor,
            last_sample: Arc::new(Mutex::new(Sample::default())),
            llm_calls: Arc::new(Mutex::new(VecDeque::new())),
            start: Instant::now(),
        }
    }

    /// Worker loop 头部调用。返回 false → worker 应短 sleep 后重试。
    ///
    /// 决策顺序：
    /// 1. 全局/本任务被 pause → false
    /// 2. CPU 已超 budget.cpu_pct_max → false
    /// 3. 否则 → true
    pub fn should_run(&self) -> bool {
        if self.paused.load(Ordering::SeqCst) {
            return false;
        }
        let sample = self.monitor.sample_self();
        if let Ok(mut last) = self.last_sample.lock() {
            *last = sample;
        }
        let budget = match self.budget.read() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        };
        if sample.cpu_pct > budget.cpu_pct_max {
            return false;
        }
        true
    }

    /// 完成一批工作后调用。返回 worker 应当 sleep 的时长。
    ///
    /// - 若上次采样接近 budget（>80%），返回 throttle 上限
    /// - 否则返回最小退让（10ms，让出 CPU）
    pub fn after_work(&self) -> Duration {
        let sample = match self.last_sample.lock() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        };
        let budget = match self.budget.read() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        };
        if sample.cpu_pct > budget.cpu_pct_max * 0.8 {
            Duration::from_millis(budget.throttle_on_exceed_ms)
        } else {
            Duration::from_millis(10)
        }
    }

    /// LLM 调用类任务（SkillEvolution / MemoryConsolidation）每次调用 LLM 前 check。
    /// 返回 false → 已超过本小时配额，调用方应跳过本次。
    pub fn allow_llm_call(&self) -> bool {
        let budget = match self.budget.read() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        };
        let Some(limit) = budget.llm_calls_per_hour else {
            return true;
        };
        let now = self.start.elapsed().as_secs();
        let mut calls = match self.llm_calls.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        // 清理过期窗口
        while let Some(&t) = calls.front() {
            if now.saturating_sub(t) > LLM_WINDOW_SECS {
                calls.pop_front();
            } else {
                break;
            }
        }
        if (calls.len() as u32) >= limit {
            return false;
        }
        calls.push_back(now);
        true
    }

    pub fn set_profile(&self, p: Profile) {
        if let Ok(mut g) = self.profile.write() {
            *g = p;
        }
        let new_budget = p.budget_for(self.kind);
        if let Ok(mut g) = self.budget.write() {
            *g = new_budget;
        }
    }

    pub fn current_profile(&self) -> Profile {
        match self.profile.read() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        }
    }

    pub fn current_budget(&self) -> Budget {
        match self.budget.read() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        }
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn last_sample(&self) -> Sample {
        match self.last_sample.lock() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        }
    }
}

/// 单任务对外快照 — 用于 [`super::registry::GovernorRegistry::snapshot`] 与
/// `attune --diag` (H5)。
#[derive(Debug, Clone, Serialize)]
pub struct TaskStatus {
    pub id: &'static str,
    pub profile: Profile,
    pub paused: bool,
    pub last_cpu_pct: f32,
    pub last_rss_bytes: u64,
    pub budget_cpu_pct_max: f32,
    pub budget_ram_bytes_max: u64,
}

impl TaskStatus {
    pub fn from_governor(g: &TaskGovernor) -> Self {
        let s = g.last_sample();
        let b = g.current_budget();
        Self {
            id: g.kind.as_str(),
            profile: g.current_profile(),
            paused: g.is_paused(),
            last_cpu_pct: s.cpu_pct,
            last_rss_bytes: s.rss_bytes,
            budget_cpu_pct_max: b.cpu_pct_max,
            budget_ram_bytes_max: b.ram_bytes_max,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_governor::monitor::MockMonitor;

    fn governor_with_cpu(kind: TaskKind, profile: Profile, cpu_pct: f32) -> TaskGovernor {
        let m = Arc::new(MockMonitor::new(Sample {
            cpu_pct,
            rss_bytes: 100 * 1024 * 1024,
            captured_secs: 0,
        }));
        TaskGovernor::new(kind, profile, m)
    }

    #[test]
    fn should_run_when_cpu_below_budget() {
        let g = governor_with_cpu(TaskKind::EmbeddingQueue, Profile::Balanced, 10.0);
        // Balanced EmbeddingQueue cap = 25%, sample = 10% → 应允许
        assert!(g.should_run());
    }

    #[test]
    fn should_not_run_when_cpu_exceeds_budget() {
        let g = governor_with_cpu(TaskKind::EmbeddingQueue, Profile::Balanced, 99.0);
        assert!(!g.should_run());
    }

    #[test]
    fn pause_stops_should_run_immediately() {
        let g = governor_with_cpu(TaskKind::FileScanner, Profile::Aggressive, 1.0);
        assert!(g.should_run());
        g.pause();
        assert!(!g.should_run());
        g.resume();
        assert!(g.should_run());
    }

    #[test]
    fn after_work_returns_throttle_when_near_budget() {
        // 设置 cpu_pct = 24（接近 Balanced EmbeddingQueue 的 25 上限的 96%）
        let g = governor_with_cpu(TaskKind::EmbeddingQueue, Profile::Balanced, 24.0);
        // 触发一次 sample 写入 last_sample
        let _ = g.should_run();
        let d = g.after_work();
        assert_eq!(d, Duration::from_millis(1000));  // Balanced throttle
    }

    #[test]
    fn after_work_minimal_when_far_from_budget() {
        let g = governor_with_cpu(TaskKind::EmbeddingQueue, Profile::Balanced, 5.0);
        let _ = g.should_run();
        assert_eq!(g.after_work(), Duration::from_millis(10));
    }

    #[test]
    fn set_profile_recomputes_budget() {
        let g = governor_with_cpu(TaskKind::EmbeddingQueue, Profile::Balanced, 0.0);
        assert_eq!(g.current_budget().cpu_pct_max, 25.0);
        g.set_profile(Profile::Aggressive);
        assert_eq!(g.current_budget().cpu_pct_max, 60.0);
        assert_eq!(g.current_profile(), Profile::Aggressive);
    }

    #[test]
    fn allow_llm_call_unlimited_when_no_cap() {
        let g = governor_with_cpu(TaskKind::EmbeddingQueue, Profile::Balanced, 0.0);
        // EmbeddingQueue 无 LLM cap
        for _ in 0..1000 {
            assert!(g.allow_llm_call());
        }
    }

    #[test]
    fn allow_llm_call_caps_at_limit() {
        // Conservative SkillEvolution = 5/h
        let g = governor_with_cpu(TaskKind::SkillEvolution, Profile::Conservative, 0.0);
        for _ in 0..5 {
            assert!(g.allow_llm_call(), "first 5 calls should succeed");
        }
        assert!(!g.allow_llm_call(), "6th call should be denied");
    }

    #[test]
    fn task_status_round_trips_state() {
        let g = governor_with_cpu(TaskKind::FileScanner, Profile::Conservative, 7.5);
        let _ = g.should_run();  // 触发 sample 写入
        let s = TaskStatus::from_governor(&g);
        assert_eq!(s.id, "file_scanner");
        assert_eq!(s.profile, Profile::Conservative);
        assert_eq!(s.last_cpu_pct, 7.5);
        assert_eq!(s.budget_cpu_pct_max, 10.0);
    }
}
