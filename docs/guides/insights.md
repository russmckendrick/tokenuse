# Insights

The Insights page surfaces deterministic Signals computed locally over your ingested usage. Signals do not make network calls and do not use an LLM — every rule reads the same `ParsedCall` records that drive the rest of the dashboard.

Open it in the TUI by pressing `i` from any page or by cycling tabs with `Tab`. In the desktop app, click the Insights tab or press `i`.

In the TUI, Insights has two internal views: Advice and Signals. Use `Left` / `Right` (or `[` / `]`) to switch views, `Up` / `Down` or the mouse wheel to scroll the active view, and `PageUp` / `PageDown` for larger jumps.

Each Signal card carries:

- a **severity** stripe (Risk / Warn / Info),
- a **category** label,
- the **scope** the rule fired against (project, tool, model, or session),
- an estimated **savings** in your currency where applicable, with the assumption stated below.

## Manual LLM advice

The Insights page can also generate LLM Advice on demand. Advice is never triggered by the application. In the desktop app, click **Generate Advice**, then choose either a redacted summary or limited prompt snippets for that run. In the TUI, press `a` for redacted summary advice or `A` for prompt-snippet advice. Advice generation runs in the background; the status line shows when the job is running and updates when the saved run is available.

V1 supports Codex, Claude Code, and Gemini. Pick the advice tool from Config. Token Use runs the selected CLI from an app-owned working directory named `Token Use App`, then refreshes the archive so the call appears in normal usage data under that project label when the tool exposes local usage logs.

Advice runs and items are stored locally in `archive.db` (`advice_runs` and `advice_items`). Failed or invalid JSON responses are stored with raw output and error details, so previous advice can be reviewed instead of disappearing.

Prompt text lives in files, not Rust or Svelte code. Shipped templates are copied on first use into:

```text
$CONFIG/tokenuse/advice-prompts/system.md
$CONFIG/tokenuse/advice-prompts/user-redacted.md
$CONFIG/tokenuse/advice-prompts/user-snippets.md
```

Those files are user-editable. Templates use variables such as `{signals_json}`, `{pricing_context}`, `{data_scope}`, `{prompt_snippets_json}`, and `{output_schema}`.

Accuracy guardrails:

- The LLM receives deterministic Signal evidence and pricing context; it explains, prioritizes, and proposes next steps.
- Advice items must cite local signal ids, sample counts, baseline windows, and confidence.
- Raw prompt snippets are included only when the per-run prompt-snippets option is selected.

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
- **Signal cards are deterministic.** They regenerate from current data on every render. Advice item workflow state is stored separately in `archive.db`.
- **Estimated savings are heuristic.** The assumption line on each card states the basis (target model, multiplier, conservative haircut) so you can sanity-check before acting.

## Architecture pointers

- Engine entry: [`crate::insights::compute_insights`](../../src/insights/mod.rs)
- Manual advice: `src/advice.rs`
- Per-rule modules: `src/insights/rules/{model_rightsizing,cache,anomalies,quota}.rs`
- Statistical baselines: `src/insights/baselines.rs` (in-memory only)
- Tauri snapshot fields: `DesktopSnapshot.insights` and `DesktopSnapshot.advice` in `desktop/src-tauri/src/snapshot.rs`
- All copy lives under the top-level `insights` block in `src/copy/copy.json`
