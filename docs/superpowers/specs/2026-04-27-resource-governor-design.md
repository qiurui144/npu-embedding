# Resource Governor (H1) Design

**Date**: 2026-04-27
**Roadmap**: 12-week strategy v2, Phase 1 W1 F-P0a
**Depends on**: nothing (foundation layer)
**Depended by**: every Phase 1 P0 background task (A1 / G1 / G2 / etc.); H2 profile picker; H3 topbar Pause; H4 auto-throttle; H5 `attune --diag`; H6 telemetry chart

[English](2026-04-27-resource-governor-design.md) · [简体中文](2026-04-27-resource-governor-design.zh.md)

---

## 1. Why

All current attune background tasks (embedding queue, SkillEvolver, file scanners, WebDAV sync, patent scanner, browser-driven web search, AI annotator) call `std::thread::spawn` directly with no CPU/RAM/IO ceiling. Symptoms:

- 100-file embedding burst causes desktop UI hitching
- Background work drains laptop battery on the go
- No user-visible Pause when presenting / gaming / streaming
- README claims "production-ready" without hard evidence

Negative competitor reference: Obsidian / Logseq are widely complained about for index-rebuild lag. Making "system-friendly" an explicit product promise + visible thresholds turns this into an attune differentiator.

## 2. Core Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Granularity | **Per-task governor** (one per background worker) | Global budget creates priority inversion (embedding starves SkillEvolver); per-task allows green-light for hot path, red-light for batch |
| Threading model | **Keep `std::thread::spawn` + `Arc<AtomicBool>`** — governor adapts to it, no forced Tokio migration | Full Tokio migration is a 2-4 week refactor, far beyond W1 budget |
| Sampling source | `sysinfo` crate (cross-platform), **global CPU%** not per-process | sysinfo 0.32 `Process::cpu_usage()` for self-process is unreliable on multiple platforms; `sys.cpus()` global usage is stable. **Semantic also better**: "system busy → back off" is closer to good-citizen intent than per-task tracking. Multiple governors share one global metric, sidestepping the per-task budget summing > 100% problem. |
| Three presets | Conservative / Balanced / Aggressive | Matches 1Password / Logseq user mental model; Balanced default; battery → auto-Conservative (H4) |
| Global pause | Central registry + each governor checks | Topbar one-click pause stops all atomically |
| Telemetry | Local-only, never reported | Hard alignment with D1 No-telemetry mode |

## 3. Module Layout

```
rust/crates/attune-core/src/resource_governor/
├── mod.rs           # public API: TaskGovernor, GovernorRegistry, Profile
├── budget.rs        # CpuBudget / RamBudget / IoBudget structs
├── profiles.rs      # Conservative / Balanced / Aggressive presets
├── governor.rs      # TaskGovernor: should_run / after_work / sample
├── monitor.rs       # CPU/RAM/IO sampling (wraps sysinfo)
├── registry.rs      # global GovernorRegistry: register / list / pause-all
└── tests.rs         # unit tests with MockMonitor (no real CPU needed)
```

## 4. Core Types

```rust
// budget.rs
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub cpu_pct_max: f32,        // **system-wide CPU% threshold** (0.0-100.0): worker pauses when global CPU > this
    pub ram_bytes_max: u64,      // resident set size cap (recorded only; not enforced cross-platform)
    pub io_priority: IoPriority, // Linux only; best-effort elsewhere
    pub throttle_on_exceed_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum IoPriority { Idle, BestEffort, Realtime }

// profiles.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile { Conservative, Balanced, Aggressive }

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum TaskKind {
    EmbeddingQueue,
    SkillEvolution,
    FileScanner,
    WebDavSync,
    PatentScanner,
    BrowserSearch,
    AiAnnotator,
    BrowseSignalIngest,  // G1
    AutoBookmark,        // G2
    MemoryConsolidation, // A1
}

// governor.rs
pub struct TaskGovernor {
    pub id: TaskKind,
    profile: Arc<RwLock<Profile>>,
    budget: Arc<RwLock<Budget>>,
    paused: Arc<AtomicBool>,
    monitor: Arc<dyn ResourceMonitor>,
    last_sample: Arc<Mutex<Sample>>,
}

impl TaskGovernor {
    /// Call at the top of each worker loop iteration.
    /// Returns false → worker should skip this round and short-sleep.
    pub fn should_run(&self) -> bool;

    /// Call after each batch. Returns sleep duration to honor throttle.
    pub fn after_work(&self) -> Duration;

    pub fn set_profile(&self, p: Profile);
    pub fn pause(&self);
    pub fn resume(&self);
}

// monitor.rs
pub trait ResourceMonitor: Send + Sync {
    fn sample_self(&self) -> Sample;
}
pub struct SysinfoMonitor { /* ... */ }
pub struct MockMonitor { /* test-only */ }

// registry.rs
pub struct GovernorRegistry { /* ... */ }
impl GovernorRegistry {
    pub fn register(&self, kind: TaskKind) -> Arc<TaskGovernor>;
    pub fn pause_all(&self);
    pub fn resume_all(&self);
    pub fn set_profile(&self, p: Profile);
    pub fn snapshot(&self) -> Vec<TaskStatus>;  // feeds H5 attune --diag
}
pub fn global_registry() -> &'static GovernorRegistry;
```

## 5. Worker Retrofit Pattern

Reference: `rust/crates/attune-core/src/queue.rs` before/after.

```rust
// BEFORE
std::thread::spawn(move || {
    while running.load(Ordering::SeqCst) {
        match Self::process_batch(...) {
            Ok(0) => std::thread::sleep(POLL_INTERVAL),
            Ok(_) => { /* immediate next round */ }
            Err(_) => std::thread::sleep(POLL_INTERVAL),
        }
    }
});

// AFTER
let governor = global_registry().register(TaskKind::EmbeddingQueue);
std::thread::spawn(move || {
    while running.load(Ordering::SeqCst) {
        if !governor.should_run() {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        match Self::process_batch(...) {
            Ok(0) => std::thread::sleep(POLL_INTERVAL),
            Ok(_) => std::thread::sleep(governor.after_work()),
            Err(_) => std::thread::sleep(POLL_INTERVAL),
        }
    }
});
```

Each retrofit: ~10 LOC delta (register + loop-head check + loop-tail sleep swap). Existing logic untouched.

## 6. Default Preset Values (v1)

> **Semantics**: `cpu_pct_max` is the **system-wide CPU% threshold**. A row reading
> "EmbeddingQueue Balanced 25%" means "this worker pauses when global system CPU
> usage exceeds 25%", **not** "this worker is capped at 25% of CPU". All workers
> share the same global metric.

| Task | Conservative | Balanced (default) | Aggressive |
|------|--------------|-------------------|-----------|
| EmbeddingQueue | 15% / 512MB / Idle / 2000ms | 25% / 1GB / BestEffort / 1000ms | 60% / 2GB / BestEffort / 100ms |
| SkillEvolution | 10% / 256MB / Idle / 5000ms (LLM 5/h) | 20% / 512MB / Idle / 2000ms (10/h) | 40% / 1GB / BestEffort / 500ms (30/h) |
| FileScanner | 10% / 256MB / Idle / 1000ms | 20% / 512MB / Idle / 500ms | 50% / 1GB / BestEffort / 100ms |
| WebDavSync | 10% / 128MB / Idle / 5000ms | 15% / 256MB / Idle / 2000ms | 30% / 512MB / BestEffort / 500ms |
| BrowserSearch | 30% / 1GB / BestEffort / 1000ms | 50% / 1.5GB / BestEffort / 500ms | 80% / 2GB / BestEffort / 100ms |
| AiAnnotator | 10% / 256MB / Idle / 3000ms | 20% / 512MB / Idle / 1000ms | 50% / 1GB / BestEffort / 200ms |
| BrowseSignalIngest (G1) | 5% / 64MB / Idle / 5000ms | 10% / 128MB / Idle / 2000ms | 20% / 256MB / Idle / 500ms |
| AutoBookmark (G2) | 10% / 256MB / Idle / 5000ms | 20% / 512MB / Idle / 2000ms | 40% / 1GB / BestEffort / 500ms |
| MemoryConsolidation (A1) | 15% / 512MB / Idle / 10000ms (LLM 5/h) | 25% / 1GB / Idle / 5000ms (10/h) | 50% / 2GB / BestEffort / 1000ms (30/h) |

Numbers are reasoned from RK3588 / Radeon 780M / typical-laptop tiers; final tuning happens in `docs/system-impact.md` after baseline runs.

## 7. Cross-Platform

| OS | CPU sample | RAM sample | IO Priority |
|----|-----------|-----------|-------------|
| Linux | sysinfo `/proc/self/stat` | `/proc/self/status` VmRSS | `ioprio_set` (libc) |
| Windows | sysinfo PDH | `GetProcessMemoryInfo` | best-effort (no ioprio, record only) |
| macOS | sysinfo task_info | task_info | best-effort |

**Fallback**: `IoPriority::Idle` on unsupported platforms is recorded but not enforced — never errors.

## 8. Test Strategy (per project test规范)

**Unit (`rust/crates/attune-core/src/resource_governor/tests.rs`)**:
- `MockMonitor` injects fixed samples → assert `should_run()` / `after_work()` decisions
- profile switch → budget effect immediate
- pause/resume state transitions
- preset value snapshot test (catches accidental tuning regressions)
- All deterministic inputs + precise assertions, **no random data**

**Integration (`rust/tests/governor_integration.rs`)**:
- Real `SysinfoMonitor` + real `std::thread` worker
- Spin a CPU-bound task under governor → measured CPU% ≤ budget cap
- Concurrent 5 workers + `pause_all()` → all stop within 100ms
- Real Store + tempfile, no mock DB

**Corpus Integration (W4 enhanced)**:
- 100 files (rust-lang/book pinned tag) batch embedding under governor → average CPU% ≤ 25 over the run

## 9. Documentation Output

- `docs/system-impact.md` + `.zh.md` — user-facing: three-tier explainer + measured occupancy commitments
- `rust/DEVELOP.md` + `.zh.md` section — developer: how to retrofit a new worker (5 steps)
- `RELEASE.md` + `.zh.md` — Phase 1 W1 changelog entry

## 10. Out of Scope (avoid scope creep)

- ❌ No cgroups / namespace isolation (Linux-only, high complexity — defer)
- ❌ No GPU / NPU governance (inference goes through Ollama; Ollama controls itself)
- ❌ No cross-process coordination (single attune process; attune-mcp-server runs separately and self-governs)
- ❌ No dynamic preset learning from observed load — defer until after H6

## 10.1. W1 Scope (Workers Retrofitted)

W1 retrofits the **production-active workers** in `attune-server/src/state.rs`:
- `start_classify_worker` → `TaskKind::AiAnnotator` (LLM classification path)
- `start_rescan_worker` → `TaskKind::FileScanner` (30-min directory rescan)
- `start_queue_worker` → `TaskKind::EmbeddingQueue` (production embedding path)
- `start_skill_evolver` → `TaskKind::SkillEvolution` (4h LLM expansion cycle, with `allow_llm_call` quota check)

Plus the `attune-core::queue::QueueWorker` (test-only / library path, retrofit for completeness).

**NOT in W1** (deferred to W2/W3 per their feature week):
- `web_search_browser.rs` — on-demand function called by `web_search.rs`, not a persistent worker. Will retrofit when network search becomes a long-running poller.
- `scanner_webdav.rs` / `scanner_patent.rs` — invoked from rescan_worker iteration, inherit governor via the calling worker.
- `ai_annotator.rs` — called from `routes/upload.rs` async tasks (per-request short-lived); H1 governs only persistent background workers.
- `workflow/` — orchestrated by routes; per-execution lifecycle, not a long-running worker.

## 11. W1 Acceptance Checklist

- [ ] `cargo test -p attune-core resource_governor::` all green
- [ ] `cargo test --test governor_integration` all green
- [ ] `cargo test -p attune-core queue::` still green after retrofit
- [ ] `docs/system-impact.md` + `.zh.md` completed
- [ ] `tests/MANUAL_TEST_CHECKLIST.md` "H1 governor verification" section added
- [ ] git commit + push develop, report SHA
