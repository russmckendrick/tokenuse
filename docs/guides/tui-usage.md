# TUI Usage

The terminal UI is the default `tokenuse` experience. It scans local session files, stores normalized calls in `<config dir>/tokenuse/archive.db`, and renders spend by day, project, tool, model, shell command, and MCP server.

```bash
tokenuse
```

If no local sessions are found, or archive sync fails before any calls are loaded, the app falls back to bundled JSON sample data and shows that status in the title bar.

## Dashboard

The dashboard shows:

- Summary totals for cost, calls, sessions, cache hit rate, input, output, cache reads, and cache writes.
- Daily cost and call activity.
- Spend by project and by project/tool pair.
- Top sessions and spend by model.
- Core tool calls, shell command heads, and MCP server usage.

Project names are normalized across tools. Absolute paths are folded to the nearest existing Git root when possible, then displayed with the shortest unique suffix.

Press `g` to cycle the dashboard sort mode between spend, latest date, and token use. The active sort applies to dashboard tables, pickers, the Usage page ordering, and session call rows.

## Pages

- **Overview**: the everyday command center with KPIs, an activity pulse graph, project/tool spend, model spend, shell commands, and MCP servers.
- **Deep Dive**: the analysis workbench with the full panel set, a larger chronological activity trend, top sessions, project rankings, model efficiency, core tools, shell commands, and MCP servers.
- **Usage**: rolling 24-hour per-tool consoles with a prominent pulse graph, calls/tokens/cost/last-seen totals, optional rate-limit gauges, and top models. Opening this tab automatically selects the 24 Hours period so the visible filter matches the console window.
- **Session**: drill into one `tool:session_id`, inspect per-call timestamp, model, cost, token buckets, tools, and prompt snippet, then open a call detail modal for the full stored prompt, cache price rates, and metadata.
- **Config**: display currency, confirmed local downloads for currency rates and pricing books, and a confirmed clear-data action that rebuilds the archive.

## Tab Guide

Overview is the fast read. Start there when you want to know whether current spend is normal, which project/tool pair is hot, and which models, commands, or MCP servers are shaping the session mix. The **Activity Pulse** graph is chronological and ignores the active table sort, so the line keeps showing usage over time even when ranked tables are sorted by spend, date, or tokens.

Deep Dive is the comparison view. The **Activity Trend** panel uses the same chronological timeline as Overview, then the surrounding tables rank projects, project/tool pairs, sessions, models, tools, commands, and MCP servers by the active sort. Use it when you need to explain why a period changed or decide which project/session to inspect next.

Usage is the live capacity view. Each tool gets its own console, and entering the tab switches the visible period selector to 24 Hours. The **24h pulse** line shows hourly relative activity for that tool, followed by totals for calls, tokens, cost, and last seen. Limit rows are gauges from imported plan snapshots when available; model rows are ranked bars for that tool's rolling 24-hour slice.

Activity Pulse and Activity Trend use hourly buckets for 24 Hours and 7 Days. This Month also uses hourly buckets during the first 14 days of the month, then switches to daily buckets from the 15th onward. 30 Days and All Time use daily buckets. The 24 Hours period is rolling from the current time, not a calendar-day midnight cutoff. Graph bars and pulse lines are relative to the visible panel. They are designed for quick comparison inside the terminal, not exact accounting; use the adjacent numeric columns for exact cost, call, token, reset, and plan values.

## Keyboard

The keyboard reference, footer hints, and shortcut behavior come from the shared embedded keymap used by both the TUI and desktop app.

| Key | Action |
| --- | --- |
| `q` | Quit |
| `Esc` | Close modal or go back from a sub-page |
| `1`-`5` | Period: 24 hours, 7 days, 30 days, this month, all time |
| `t` | Cycle tool filter |
| `g` | Cycle sort mode: spend, latest date, token use |
| `Shift-D` | Toggle between live and sample data |
| `p` | Open project picker |
| `Tab` / `Shift-Tab` | Cycle Overview, Deep Dive, and Usage |
| `o` | Open Overview |
| `d` | Open Deep Dive |
| `u` | Open Usage / rate limits |
| `c` | Open Configuration |
| `s` | Open session picker and drill into a single session |
| `e` | Generate a project or all-projects report |
| `f` / `b` in report modal | Choose another report folder for this session |
| `r` | Sync the local archive in place |
| `h` or `?` | Open the keybinding reference |

In the session page, use `Up` / `Down`, `PgUp` / `PgDn`, `Home` / `End` to move through calls, `Enter` or a mouse click to open call details, and `Esc` or `d` to return to Deep Dive. In pickers and configuration, use `Up` / `Down`, `Home` / `End`, `Enter`, and `Esc`.

## Usage Page

The Usage page is always a rolling 24-hour view. Opening it automatically selects the 24 Hours period. The page ignores project filters and renders all tool console sections so plan-limit gauges stay comparable across tools.
The active sort mode controls the order of tool sections and model rows; rate-limit rows keep their scope/window order.

Each tool section includes:

- One 24-hour pulse graph plus calls, tokens, cost, and last seen time.
- Zero or more limit gauge rows from imported `LimitSnapshot` records. Codex imports snapshots from rollout JSONL; Claude Code and Copilot import optional local sidecars from the tokenuse config directory.
- Up to three top model rows for that tool's 24-hour slice.

## Configuration

Runtime settings live in the platform config directory under `tokenuse`:

| File / directory | Purpose |
| --- | --- |
| `config.json` | User overrides, currently display currency |
| `archive.db` | Durable local usage archive |
| `exchange-rates.json` | Optional local currency snapshot |
| `rates.json` | Legacy local currency snapshot |
| `pricing-upstream.json` | Optional local broad pricing book |
| `pricing-overrides.json` | Optional local official overrides and aliases |
| `pricing-snapshot.json` | Legacy local pricing snapshot |
| `limits/claude-code.json` | Optional Claude Code status-line limit sidecar |
| `limits/copilot.json` | Optional Copilot quota sidecar written by confirmed sync |
| `reports/` | Fallback report directory |

USD remains the default display currency. Costs are calculated and stored internally as import-time USD, then converted for display using the configured currency.

The Config page lists the published rates and pricing book URLs next to the local file paths, so users can inspect exactly what the download actions fetch before confirming. The pricing row also shows the active book source and its latest checked/generated date. The Claude limits row imports an existing local sidecar and shows a setup hint until Claude Code's `statusLine` writes the OS-specific sidecar path. The Copilot limits row asks for confirmation before reading local Copilot credentials and fetching current quota state from GitHub.

The Config page's clear-data action asks for confirmation, deletes `archive.db`, and immediately reimports from local tool history. Config, exchange rates, pricing books, limit sidecars, legacy pricing snapshots, and reports are kept. Archive-only rows disappear if the original source files are gone, and rebuilt rows use the current configured pricing.

## Reports

Press `e` on Overview, Deep Dive, Usage, or Session to generate a report. The report modal chooses format, period, project scope, and redaction. Reports always include all tools for the chosen period and project or all-projects scope. Output defaults to the user's Downloads folder, falling back to `~/Downloads` and then `<config dir>/tokenuse/reports/`.

Run `tokenuse report` to generate reports without opening the dashboard. The guided command asks for time range, project scope, one or more report formats, output folder, redaction, and final confirmation. It writes live local-session reports only; if no local sessions are found, it exits without generating sample reports.

Reports are timestamped and slugged with the chosen period and project scope, so prior runs are not overwritten. HTML/PDF are client-ready executive report decks, SVG/PNG are one-page executive visual summaries, JSON serializes the full report dataset, Excel writes a multi-sheet workbook, and CSV writes one file per report area.

| Format | Output |
| --- | --- |
| HTML | One self-contained executive report deck |
| PDF | One browserless A4 landscape render of the same executive deck |
| SVG | One 16:9 visual summary with KPI strip, heatmap/trend, and top highlights |
| PNG | Same one-page visual summary as SVG, rasterized |
| JSON | One pretty-printed full report dataset |
| Excel | One multi-sheet workbook with summary and raw data sheets |
| CSV | One directory with one CSV per report area |
