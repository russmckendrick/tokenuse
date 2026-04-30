# Usage Page

The Usage page (`u`) is a per-tool snapshot of the **last 24 hours** of activity, with optional plan rate-limit windows layered on top. Where the Overview and Deep Dive pages aggregate cost across the active period filter, the Usage page is always rolling-24h and ignores the period and project filters.

## Opening And Closing

| Key | Action |
| --- | --- |
| `u` from Overview, Deep Dive, or Session | Open the Usage page |
| `o` | Back to Overview |
| `d` | Back to Deep Dive |
| `c` | Open Configuration |
| `Esc` | Back to the previous tab page |
| `r` | Reload (syncs the local archive on a background thread) |
| `h` or `?` | Help modal |
| `q` | Quit |

The project picker (`p`) and currency picker (open via `c` → Enter) still work on this page, but the project filter does not affect the rolling 24h aggregation — only the displayed currency is honored.

## Layout

The page is split into four equal-height sections, one per supported tool. Tools are listed top-to-bottom **sorted descending by 24h token volume**; ties keep the canonical order Codex, Claude Code, Cursor, Copilot. Each section is a small table with three row groups:

```text
┌─ Codex · 24h usage + models ────────────────────────────────────┐
│ usage   24h total   ▓▓▓▓▓▓▓▓░░    142   1.2M   $4.71   12m       │
│ limit   Codex 5h    ▓▓▓░░░░░░░    27% left     14:30   Pro       │
│ limit   Codex weekly▓▓▓▓▓░░░░░    51% left     03 May  Pro       │
│ gpt-5-codex          ▓▓▓▓▓▓▓░░    98     900K   $3.10            │
│ gpt-5-codex-spark    ▓▓▓░░░░░░    44     320K   $1.61            │
└──────────────────────────────────────────────────────────────────┘
```

### Usage Row

One per tool. Computed in `build_tool_limit_sections` (`src/ingest.rs`) over every `ParsedCall` whose `timestamp` falls within the last 24 hours of the local clock.

| Column | Meaning |
| --- | --- |
| Bar | Sparkline of 24 hourly buckets, scaled so the largest bucket is 100. Empty hours render as gaps. |
| `calls` | Number of calls in the window |
| `tokens` | Sum of `input + output + cache_creation + cache_read` for the window, formatted compactly (`1.2M`, `420K`) |
| `cost` | Sum of `cost_usd`, formatted in the active display currency |
| `last_seen` | Time since the most recent call: `now`, `42m`, `5h`, or `3d` |

Calls without a timestamp are skipped. Calls with timestamps in the future are also skipped (clock skew safety).

### Limit Rows

Zero or more per tool, derived from `LimitSnapshot` records emitted by adapters during archive sync. Today only the **Codex** adapter parses plan rate-limit data — see `src/tools/codex/parser.rs`. Other tools show only the usage row and model rows.

When multiple snapshots exist for the same `(tool, limit_id)`, only the latest by `observed_at` is kept (`limit_is_newer`). Each surviving snapshot can contribute up to two rows: one for `primary` and one for `secondary`.

| Column | Meaning |
| --- | --- |
| `scope` | `LimitSnapshot.limit_name` if present, otherwise the tool's short label |
| `window` | Pretty form of `window_minutes`: `5h`, `weekly`, `1d`, `2h`, or raw minutes |
| Bar | Used percentage, clamped to `0..=100` |
| `% left` | `100 - used_percent`, also clamped |
| `reset` | Local time when the window resets — `HH:MM` for today, `DD MMM HH:MM` otherwise, `-` if unknown |
| `plan` | Normalized plan label (`Pro`, `Plus`, `Pro Lite`, `Free`, …), `-` if unknown |

Within a tool, limit rows are sorted by scope, then by window rank (`5h` < `weekly` < everything else), then by raw window string.

### Top Models

Up to three rows per tool, taken from the same 24h call set:

| Column | Meaning |
| --- | --- |
| `name` | Model display name from the tool adapter's `model_display()` |
| Bar | Token volume scaled against the top model in this tool's 24h slice |
| `calls`, `tokens`, `cost` | Sums for that model in the 24h window |

Models are sorted descending by tokens, then cost, then calls, then name. The biggest model row's bar always reaches 100; smaller rows scale proportionally. Tools with zero 24h calls show no model rows.

## Source Data

Usage data is built from the same `Vec<ParsedCall>` the Dashboard uses, plus the archived `Vec<LimitSnapshot>` from `Ingested.limits`. Both are loaded from `<config dir>/tokenuse/archive.db`. Startup loads an existing archive immediately; `r` forces an archive sync and updates the page on the next tick.

The Usage page **does not** filter by the active period (`1`–`5`) or the active project filter (`p`). The active tool filter (`t`) is also ignored: every supported tool gets its own section regardless. Currency selection from the Config page does flow through the cost columns.

## Sample Mode

When archive sync finds no local sessions, the dashboard falls back to bundled sample data and the Usage page renders with synthetic values from `data::limits_data` (`src/data.rs`). Sample mode preserves the row shape so screenshots and tests are stable, but the figures are not real and the title bar shows the sample-mode banner.

## Render Path

1. `App::usage()` calls `Ingested::limits(tool, currency)` (live) or `data::limits_data(tool)` (sample).
2. `build_limits_data` keeps the latest `LimitSnapshot` per `(tool, limit_id)` and turns each window into a `LimitMetric`.
3. `build_tool_limit_sections` walks `ParsedCall` to build per-tool 24h `RecentUsageMetric` and the top-3 `RecentModelMetric` rows, then attaches the `LimitMetric`s by tool short label.
4. `ui::sections::render_limits` (`src/ui/sections.rs`) lays out one quarter-height table per section in the canonical order produced by step 3.

The page has no per-frame work beyond the lookup — sections, limits, and models are all computed up-front in the `LimitsData` struct.
