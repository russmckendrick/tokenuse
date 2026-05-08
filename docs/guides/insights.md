# Insights

The Insights page surfaces heuristic recommendations computed locally over your ingested usage. There is no network call, no LLM, and no telemetry — every rule reads the same `ParsedCall` records that drive the rest of the dashboard.

Open it in the TUI by pressing `i` from any page or by cycling tabs with `Tab`. In the desktop app, click the Insights tab or press `i`.

Each card carries:

- a **severity** stripe (Risk / Warn / Info),
- a **category** label,
- the **scope** the rule fired against (project, tool, model, or session),
- an estimated **savings** in your currency where applicable, with the assumption stated below.

## Categories

### Model right-sizing

| Rule | Trigger | Savings basis |
| --- | --- | --- |
| `short_output_sonnet` | ≥ 50 Sonnet calls in a project with `output_tokens < 200`, `input_tokens < 4000`, no reasoning, **and** ≥ 30% of the project's Sonnet calls | Re-priced through `claude-haiku-4-5`, scaled to weekly |
| `fast_mode_opus_excess` | Project's fast-mode Opus spend > 2× standard-mode Opus and ≥ $5 | Each call's fast-mode multiplier overhead, scaled to weekly |
| `reasoning_heavy_o_series` | ≥ 20 Codex o-series calls with `reasoning_tokens / output_tokens > 3` and reasoning > 40% of cost | Conservative half of the reasoning bill |

### Cache efficiency

Cache rules run only against tools that report cache metrics: Claude Code, Codex, and Gemini. For Cursor and Copilot, an Info card explains that their local logs don't expose cache buckets.

| Rule | Trigger | Savings basis |
| --- | --- | --- |
| `cache_hit_trend_drop` | 7-day hit rate vs prior 30-day baseline drops > 15 pp from a baseline ≥ 50% | Missing cache reads × (input − cache_read) per token |
| `cache_write_overhead` | Cache write/read ratio > 0.5 across ≥ 100 events | Excess writes × (cache_write − cache_read) |
| `low_hit_project_outlier` | Project hit rate < 0.5× the tool-wide median across ≥ 5 sessions | Gap to median × project input tokens × delta |

### Anomalies

| Rule | Trigger | Savings basis |
| --- | --- | --- |
| `outlier_session_cost` | Session cost > P95 or Q3+1.5·IQR over the last 30 days, baseline ≥ 20 sessions | None (Info; click through to the Session page) |
| `day_over_day_spend_zscore` | Today's z-score > 2.5 vs the trailing-30-day mean (≥ 14 non-zero days) | None |
| `project_mom_growth` | Project up > 50% MoM, ≥ $10 in the current month, both months ≥ 10 calls | None |

### Quota / pacing

Reads `LimitSnapshot::primary` from the existing Usage data flow.

| Rule | Trigger | Savings basis |
| --- | --- | --- |
| `claude_weekly_forecast` | Projected weekly use ≥ 90% (Risk at ≥ 100%) | None |
| `copilot_premium_pacing` | Projected cycle use ≥ 80% | None |
| `limit_recently_hit` | `rate_limit_reached_type` set within the last 24h | None (Risk) |

## Limitations

- **No latency data.** No "use a faster model for the same cost" recommendations. Don't add latency rules without first ingesting latency.
- **Cursor and Copilot cache.** Their local logs don't expose `cache_read` / `cache_creation` tokens, so cache rules can't compute savings for them. The Info card makes that gap visible.
- **No dismissal in v1.** Cards regenerate from the current data on every render. Dismissals would need a small persisted table in `archive.db` and migrations — deferred until v2.
- **Estimated savings are heuristic.** The assumption line on each card states the basis (target model, multiplier, conservative haircut) so you can sanity-check before acting.

## Architecture pointers

- Engine entry: [`crate::insights::compute_insights`](../../src/insights/mod.rs)
- Per-rule modules: `src/insights/rules/{model_rightsizing,cache,anomalies,quota}.rs`
- Statistical baselines: `src/insights/baselines.rs` (in-memory only)
- Tauri snapshot field: `DesktopSnapshot.insights` in `desktop/src-tauri/src/snapshot.rs`
- All copy lives under the top-level `insights` block in `src/copy/copy.json`
