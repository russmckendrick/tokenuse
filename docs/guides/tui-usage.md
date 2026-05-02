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
- **Session**: drill into one `tool:session_id`, inspect per-call timestamp, model, cost, token buckets, tools, and prompt snippet, then open a call detail modal for the full stored prompt and metadata.
- **Config**: display currency and confirmed local downloads for currency rates and LiteLLM pricing snapshots.

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
| `e` | Export the current view as JSON, CSV, SVG, or PNG |
| `f` / `b` in export modal | Choose another export folder for this session |
| `r` | Sync the local archive in place |
| `h` or `?` | Open the keybinding reference |

In the session page, use `Up` / `Down`, `PgUp` / `PgDn`, `Home` / `End` to move through calls, `Enter` or a mouse click to open call details, and `Esc` or `d` to return to Deep Dive. In pickers and configuration, use `Up` / `Down`, `Home` / `End`, `Enter`, and `Esc`.

## Usage Page

The Usage page is always a rolling 24-hour view. Opening it automatically selects the 24 Hours period. The page ignores the project filter; the tool filter can still narrow the visible console sections.
The active sort mode controls the order of tool sections and model rows; rate-limit rows keep their scope/window order.

Each tool section includes:

- One 24-hour pulse graph plus calls, tokens, cost, and last seen time.
- Zero or more limit gauge rows from imported `LimitSnapshot` records. Currently Codex is the only adapter that imports plan rate-limit snapshots.
- Up to three top model rows for that tool's 24-hour slice.

## Configuration

Runtime settings live in the platform config directory under `tokenuse`:

| File / directory | Purpose |
| --- | --- |
| `config.json` | User overrides, currently display currency |
| `archive.db` | Durable local usage archive |
| `rates.json` | Optional local currency snapshot |
| `pricing-snapshot.json` | Optional local LiteLLM-derived pricing snapshot |
| `exports/` | Fallback export directory |

USD remains the default display currency. Costs are calculated and stored internally as import-time USD, then converted for display using the configured currency.

## Export

Press `e` on Overview, Deep Dive, Usage, or Session to export. JSON, CSV, SVG, and PNG export the current filtered dashboard view; HTML and PDF export self-contained workbook reports. Output defaults to the user's Downloads folder, falling back to `~/Downloads` and then `<config dir>/tokenuse/exports/`.

Exports are timestamped and slugged with the active period, tool, and project filter, so prior runs are not overwritten.

| Format | Output |
| --- | --- |
| JSON | One pretty-printed dashboard data file |
| CSV | One directory with one CSV per dashboard panel |
| SVG | One multi-panel dashboard render |
| PNG | Same dashboard render as SVG, rasterized |
| HTML | One self-contained print-friendly workbook with dashboard panels and the selected Session's full call detail when a session is open |
| PDF | One browserless PDF render of the same branded HTML workbook, including selected Session detail |
