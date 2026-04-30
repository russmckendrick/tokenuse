# Architecture

`tokenuse` is intentionally local: read local session files, append normalized records to its own archive, aggregate in memory, and render a terminal dashboard. There is no daemon and no file watcher.

## Startup Flow

```mermaid
flowchart TD
    A[cargo run] --> B[handle CLI flags]
    B -->|--list-projects| C[sync archive and print inventory]
    B -->|--refresh-prices| D[refresh pricing snapshot when feature is enabled]
    B -->|--generate-currency-json| L[generate currency snapshot when feature is enabled]
    B -->|no flag| M[load config.json and rates.json]
    M --> N[open archive.db]
    N --> O{archive has rows?}
    O -->|yes| P[load archive into Ingested]
    O -->|no| Q[import legacy ingest-cache if present]
    Q --> R[sync local tool sources]
    R --> S[append new ParsedCall and LimitSnapshot rows]
    S --> P
    P --> H{any calls or limits?}
    H -->|yes| I[DataSource::Live]
    H -->|no| J[DataSource::Sample]
    I --> K[render TUI]
    J --> K
    K --> T[background sync every 15 min and on r]
```

The durable archive lives at `<config dir>/tokenuse/archive.db`. If it already has rows, startup loads it immediately and queues an incremental background sync so the dashboard opens without reparsing every source. If the archive is empty, startup imports the legacy `~/.cache/tokenuse/ingest-cache.json` snapshot when present, performs one synchronous source sync, then renders from the archive. If the archive cannot be opened or migrated, the app falls back to raw `ingest::load()` for that run.

New sessions written while the dashboard is open are visible after archive sync — press `r` (Dashboard, Usage, or Session pages) to sync on a background thread. The dashboard stays responsive: the status bar shows `reloading…` while it runs, the next tick of the main loop drains completed results via `App::poll_reload`, and the status flips to `reloaded · N calls`. The refresher runs one sync at a time; if several results complete between UI ticks, the latest result wins. Failures or empty sync results keep the prior data unchanged.

Individual adapter discovery or parse errors are skipped so one malformed source does not stop the whole dashboard. If the archive has no calls or limits after sync, the UI shows sample data and a status message.

## Normalized Record

Every adapter emits `ParsedCall` from `src/tools/types.rs`. The important fields are:

| Field | Meaning |
| --- | --- |
| `tool` | Stable internal tool id such as `claude-code`, `cursor`, `codex`, or `copilot` |
| `model` | Raw or inferred model name before display shortening |
| `input_tokens`, `output_tokens` | Billable input/output buckets after adapter-specific normalization |
| `cache_creation_input_tokens`, `cache_read_input_tokens` | Cache write/read buckets when the tool exposes them |
| `cached_input_tokens` | Cached input reported inside `input_tokens`, currently used for OpenAI-style records |
| `reasoning_tokens` | Reasoning bucket when exposed or estimated |
| `web_search_requests` | Server-side web search request count when exposed |
| `cost_usd` | Calculated from the configured pricing snapshot at import time |
| `tools`, `bash_commands` | Tool call names and split shell commands |
| `timestamp`, `session_id`, `project` | Aggregation and filtering keys |
| `dedup_key` | Per-call key used by the shared run-level dedup set |

## Aggregation

```mermaid
flowchart LR
    A[Vec ParsedCall] --> B[period filter]
    B --> C[tool filter]
    C --> D[project filter]
    D --> E[summary totals]
    D --> F[daily activity]
    D --> G[projects]
    D --> H[project/tool rows]
    D --> I[sessions]
    D --> J[models]
    D --> K[core tools]
    D --> L[shell commands]
    D --> M[MCP servers]
```

The dashboard panels are built from the filtered call set:

- Summary: cost, call count, tool-qualified session count, cache hit rate, input, output, cache reads, and cache writes.
- Daily Activity: cost and calls by local date.
- By Project: top projects by cost, average cost per session, and top tool spend mix.
- Top Sessions: highest-cost sessions, keyed by `tool:session_id`.
- Project Spend by Tool: project/tool rows sorted by project total, then tool spend.
- By Model: model display name, cost, calls, and cache percentage.
- Core Tools: normalized assistant tool calls.
- Shell Commands: first word of split Bash commands.
- MCP Servers: tool names shaped like `mcp__server__tool`, grouped by server.

## Pages And Modals

The TUI is a small state machine over five pages (Overview, Deep Dive, Usage, Config, Session) plus five overlay modals. The first three pages are reachable through the tab strip via `Tab` / `Shift-Tab` or their direct keys; Config and Session are sub-pages opened from any tab. All routing lives in `src/app.rs`; rendering is dispatched from `src/ui.rs`.

```mermaid
flowchart LR
    O[Overview] -- d / Tab --> DD[Deep Dive]
    O -- u --> U[Usage]
    O -- c --> Cfg[Config]
    O -- s --> SP[Session picker]
    SP -- Enter --> Sess[Session page]
    DD -- o / Shift-Tab --> O
    DD -- u --> U
    DD -- s --> SP
    DD -- c --> Cfg
    U -- o --> O
    U -- d --> DD
    U -- c --> Cfg
    Cfg -- Esc/d --> DD
    Cfg -- o --> O
    Cfg -- u --> O
    Sess -- Esc/d --> DD
    O -- p --> Pick[Project picker]
    DD -- p --> Pick
    O -- e --> Exp[Export picker]
    DD -- e --> Exp
    Cfg -- Enter on currency --> Curr[Currency picker]
    O -- h/? --> Help[Help modal]
    DD -- h/? --> Help
    U -- h/? --> Help
    Sess -- h/? --> Help
    Cfg -- h/? --> Help
```

- **Overview** (`Page::Overview`): default landing page. Compact KPI strip plus daily activity, models, project/tool spend, shell commands, and MCP servers. Acts as the at-a-glance landing for everyday use.
- **Deep Dive** (`Page::DeepDive`): every panel listed under [Aggregation](#aggregation), including top sessions and core tool counts that are not on Overview.
- **Usage** (`Page::Usage`): per-tool 24-hour activity histogram, optional plan-side rate limit windows, and top-3 models per tool. Built from `Ingested::limits` over the same `ParsedCall` set plus `LimitSnapshot` records. Period and project filters are deliberately ignored. See [`docs/usage.md`](usage.md).
- **Session** (`Page::Session`): drill-down for one `tool:session_id`. Rendered from `SessionDetailView`, computed by filtering `Ingested.calls` by `session_key(call) == key` and sorting by timestamp. Live data shows per-call timestamp, model, cost, in/out tokens, cache, tools used, and a 120-char single-line prompt snippet. Sample mode shows a privacy note since per-call records are not bundled.
- **Config** (`Page::Config`): currency override + local data refresh actions (rates, LiteLLM pricing).
- **Project picker, Currency picker, Session picker** (`*Modal` structs): each holds `options`, a typeable `query`, and a `filtered: Vec<usize>` mapping; all three share the same case-insensitive substring filter pattern. The project picker pins `All` regardless of query.
- **Export picker** (`ExportModal`): four-row chooser (`JSON`, `CSV`, `SVG`, `PNG`); on `Enter` it calls `export::write` with the active period, tool, and project filter.
- **Help** (`help_open: bool`): full keybinding reference, openable from any page with `h` or `?`. Closes with `h`, `?`, or `Esc`.

The five modals are checked in priority order in `App::handle_key`: help, currency, project, session, then export. Only one is open at a time.

## Project Identity

Raw project strings come from each tool's local data. Before display, `tokenuse`:

1. normalizes path separators and trims trailing slashes
2. folds absolute paths to the nearest existing Git root when one exists
3. groups costs by that identity across tools
4. displays the shortest unique suffix, such as `tokens` or `dvr/tokens`

`cargo run -- --list-projects` syncs the archive, then prints both the compact project label and the raw project value so ingestion mistakes are easier to spot.

## Archive And Sync

`src/archive.rs` owns the SQLite archive. It stores full `ParsedCall` rows, append-only limit snapshots, and per-source fingerprints in `source_state`. Calls are unique on `(tool, dedup_key)`, so a changed source can be reparsed safely without duplicating historical calls. Source deletion never removes archive rows; once tokenuse has imported a call, it remains available even if the original tool history is later cleared.

The source fingerprint hook defaults to file metadata for file-backed sources and recursive directory metadata for directory-backed sources. When a source fingerprint has not changed, sync skips parsing it. When it changes, sync parses the source, inserts only new call keys, stores any new limit snapshots, and updates the fingerprint.

The old JSON ingest cache is now legacy seed input only. New runs do not write `~/.cache/tokenuse/ingest-cache.json`.

## Deduplication

A single shared `HashSet<String>` is passed through every adapter during a run. Each parser creates a stable `dedup_key` for the call shape it understands:

- Claude Code: message id, falling back to timestamp
- Cursor bubbles: conversation id, timestamp, and token counts
- Cursor Agent KV: request id
- Codex: rollout path, token event timestamp, and cumulative token totals
- Copilot: session id and message id

Session counts are tool-qualified, so `claude-code:s1` and `codex:s1` remain separate sessions even if the raw session id text matches.

## Pricing

`src/pricing/snapshot.json` is embedded at compile time. At runtime, `PriceTable::configured()` first looks for a local `pricing-snapshot.json` in the tokenuse config directory, then falls back to the embedded snapshot.

```mermaid
flowchart LR
    A[raw model name] --> B[canonicalize]
    B --> C{exact model?}
    C -->|yes| D[price row]
    C -->|no| E{alias?}
    E -->|yes| D
    E -->|no| F{prefix match?}
    F -->|yes| D
    F -->|no| G[fallback model]
    D --> H[cost_usd]
    G --> H
```

Canonicalization lowercases model names, drops a vendor prefix such as `anthropic/`, strips an `@pin` suffix, and removes trailing `-YYYYMMDD` date stamps. Aliases such as `cursor-auto`, `anthropic-auto`, and `openai-auto` resolve through the snapshot.

The pricing formula is:

```text
cost = multiplier * (
    input_tokens * input_rate
  + output_tokens * output_rate
  + cache_creation_input_tokens * cache_write_rate
  + cache_read_input_tokens * cache_read_rate
  + web_search_requests * web_search_rate
)
```

Claude Opus fast mode uses the model row's `fast_multiplier` when present. The CLI refresh command fetches LiteLLM pricing, filters to relevant model families, adds local aliases, and rewrites the embedded snapshot:

```bash
cargo run --features refresh-prices -- --refresh-prices
```

The TUI configuration page can also pull the LiteLLM-derived snapshot into the local config directory when built with `refresh-prices`. Because the archive stores `cost_usd` at import time, refreshed pricing applies to newly imported calls; existing historical rows keep their original USD cost.

## Export

Press `e` on the Dashboard to open the export picker. Output lands in `<config dir>/tokenuse/exports/`, never overwriting prior runs — every filename is timestamped with `YYYYMMDDTHHMMSS` and slugged with the active period, tool, and project filter (for example `tokenuse-20260429T160054-week-claude-allprojects.json`).

Exports always reflect the **current filtered view** (period + tool + project). The data shape and the filter slug are computed from the same `DashboardData` the dashboard is rendering.

| Format | Output | Notes |
| --- | --- | --- |
| JSON | one `.json` file | Pretty-printed `DashboardData` (summary, daily, projects, project_tools, sessions, models, tools, commands, mcp_servers). All `&'static str` panel cells serialize as strings. |
| CSV | a directory of `.csv` files | One file per panel: `summary.csv`, `daily.csv`, `projects.csv`, `project_tools.csv`, `sessions.csv`, `models.csv`, `tools.csv`, `commands.csv`, `mcp_servers.csv`. Hand-written RFC 4180 escaping (commas, quotes, newlines). |
| SVG | one `.svg` file | Multi-panel render of the dashboard at 1800×1500. |
| PNG | one `.png` file | Same render as SVG, rasterized via `plotters`' bitmap backend. |

Both image formats are produced by the same `render_dashboard_chart` function in `src/export.rs`, so they always look identical. The palette is loaded from constants that mirror `src/theme.rs` and `DESIGN.md`. Tests serialize chart rendering through a process-wide `Mutex` because plotters' macOS font lookup is not thread-safe.

The export pipeline depends on `plotters` (with the `svg_backend`, `bitmap_backend`, `bitmap_encoder`, `line_series`, and `ttf` features) and the existing `serde_json`. There is no network dependency on this path.

## Configuration And Currency

Runtime settings live in the platform config directory under `tokenuse`:

| File / directory | Purpose |
| --- | --- |
| `config.json` | User overrides, currently the display currency |
| `archive.db` | Durable local usage archive loaded by the dashboard |
| `rates.json` | Locally downloaded copy of the published currency snapshot |
| `pricing-snapshot.json` | Locally downloaded LiteLLM-derived pricing snapshot |
| `exports/` | Output directory for `e`-key exports (JSON, CSV, SVG, PNG) |

USD is the default display currency. The dashboard still stores calculated spend as `cost_usd`; aggregation sums USD and formats the final display values through the active currency table.

`currency/rates.json` is the embedded fallback snapshot. The TUI configuration page can pull the latest published copy from:

```text
https://raw.githubusercontent.com/russmckendrick/tokenuse/refs/heads/main/currency/rates.json
```

That local rates pull is enabled only when built with `refresh-currency`; the default build remains local-only and has no network dependency.

The snapshot is generated from Frankfurter's USD-based v2 rates endpoint, filtered to fiat display currencies, and refreshed by a nightly GitHub Action:

```bash
cargo run --features refresh-currency -- --generate-currency-json
```
