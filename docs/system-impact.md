# Attune System Impact

[English](system-impact.md) · [简体中文](system-impact.zh.md)

Attune is designed to be a **good citizen** on your machine. Every background task — embedding generation, file scanning, LLM-driven classification, skill evolution — runs under a per-task resource governor that yields when your system is busy.

## Three System Impact Tiers

Pick the tier that matches how you use the machine. Switch any time from Settings → System Impact.

| Tier | When to pick it | Behavior |
|------|----------------|---------|
| **Conservative** | Battery / older laptops / video calls | Background work only when system CPU < 5–30% (depending on task); LLM-evolution capped to 5 calls/hour |
| **Balanced** (default) | Plugged-in laptop / typical desktop | System CPU < 15–50%; LLM-evolution 10 calls/hour |
| **Aggressive** | Idle desktop / dedicated NAS | System CPU < 30–80%; LLM-evolution 30 calls/hour; minimal throttle |

## What the Numbers Mean

`cpu_pct_max` is a **system-wide CPU threshold**, not a per-task cap. "EmbeddingQueue Balanced 25%" means: *the embedding worker pauses when total system CPU usage exceeds 25%*. All workers share one global view, so multiple Attune workers running together never accidentally saturate your machine.

## Top-Bar Pause Button

A single click in the top bar pauses **every** background worker — embedding queue, file scanners, LLM classification, skill evolution, browser-driven web search, browse-signal ingestion. Resume is also one click. Use it before presenting, gaming, or running benchmarks.

## What's Governed

| Worker | Default (Balanced) CPU threshold | Memory cap |
|--------|---------------------------------|------------|
| Embedding queue | 25% | 1 GB |
| Skill evolution (LLM) | 20% | 512 MB (10 LLM calls/h) |
| File scanner | 20% | 512 MB |
| WebDAV sync | 15% | 256 MB |
| Browser-driven web search | 50% | 1.5 GB |
| AI annotator | 20% | 512 MB |
| Browse signal ingest (G1) | 10% | 128 MB |
| Auto-bookmark (G2) | 20% | 512 MB |
| Memory consolidation (A1) | 25% | 1 GB (10 LLM calls/h) |

Full preset table: [`rust/crates/attune-core/src/resource_governor/profiles.rs`](../rust/crates/attune-core/src/resource_governor/profiles.rs).

## What's NOT Governed

- **User-triggered actions** (chat queries, search, manual upload) bypass the governor — your active interactions are always responsive.
- **Per-request HTTP handlers** (Axum routes) are not background workers; they are short-lived tokio tasks.
- **GPU / NPU usage** is delegated to Ollama, which has its own resource controls.

## Privacy: Local-Only Telemetry

The governor records CPU / RAM samples in-process for diagnostics (`attune --diag` H5, future H6 chart). This data **never leaves your device** and is not persisted across restarts. Combined with the No-Telemetry toggle (D1, also planned), you can verify zero outbound network calls.

## Verifying It Works

```bash
# Start a 100-file embedding burst, then in another terminal:
attune --diag

# Output (sample):
# embedding_queue       profile=Balanced  paused=false  cpu=18.3%  rss=421MB  budget=25%/1024MB
# file_scanner          profile=Balanced  paused=false  cpu=2.1%   rss=89MB   budget=20%/512MB
# skill_evolution       profile=Balanced  paused=false  cpu=0.0%   rss=12MB   budget=20%/512MB (10 LLM/h)
```

If you see CPU usage consistently exceed the budget, file an issue with the diag output — that indicates a bug, not expected behavior.
