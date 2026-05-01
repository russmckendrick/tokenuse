# TUI Usage

The terminal UI is the default `tokenuse` experience. It scans local session files, stores normalized calls in `<config dir>/tokenuse/archive.db`, and renders spend by day, project, tool, model, shell command, and MCP server.

```bash
tokenuse
```

If no local sessions are found, or archive sync fails before any calls are loaded, the app falls back to bundled sample data and shows that status in the title bar.

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

- **Overview**: the everyday landing page with KPIs, daily activity, models, project/tool spend, shell commands, and MCP servers.
- **Deep Dive**: the full panel set, including top sessions and core tool counts.
- **Usage**: rolling 24-hour per-tool activity, optional plan rate-limit windows, and top models.
- **Session**: drill into one `tool:session_id`, inspect per-call timestamp, model, cost, token buckets, tools, and prompt snippet, then open a call detail modal for the full stored prompt and metadata.
- **Config**: display currency and confirmed local downloads for currency rates and LiteLLM pricing snapshots.

## Keyboard

The keyboard reference, footer hints, and shortcut behavior come from the shared embedded keymap used by both the TUI and desktop app.

| Key | Action |
| --- | --- |
| `q` | Quit |
| `Esc` | Close modal or go back from a sub-page |
| `1`-`5` | Period: today, 7 days, 30 days, this month, all time |
| `t` | Cycle tool filter |
| `g` | Cycle sort mode: spend, latest date, token use |
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

The Usage page is always a rolling 24-hour view. It ignores the active period, project filter, and tool filter so every supported tool gets its own section.
The active sort mode controls the order of tool sections and model rows; rate-limit rows keep their scope/window order.

Each tool section includes:

- One 24-hour activity row with calls, tokens, cost, and last seen time.
- Zero or more limit rows from imported `LimitSnapshot` records. Today Codex is the only adapter that imports plan rate-limit snapshots.
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

Press `e` on Overview or Deep Dive to export the current filtered dashboard view. Output defaults to the user's Downloads folder, falling back to `~/Downloads` and then `<config dir>/tokenuse/exports/`.

Exports are timestamped and slugged with the active period, tool, and project filter, so prior runs are not overwritten.

| Format | Output |
| --- | --- |
| JSON | One pretty-printed dashboard data file |
| CSV | One directory with one CSV per dashboard panel |
| SVG | One multi-panel dashboard render |
| PNG | Same dashboard render as SVG, rasterized |
