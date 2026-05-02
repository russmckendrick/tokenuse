# Desktop App Usage

The desktop app is a Tauri v2 + Svelte frontend over the same Rust core as the TUI. It shares the local archive, config, currency, pricing, refresh, session drill-down, and export logic.

## Install And Open

Install on macOS with Homebrew Cask:

```bash
brew install --cask russmckendrick/tap/tokenuse-desktop
open -a "Token Use"
```

The macOS desktop app also ships as a signed and notarized universal DMG. Linux desktop builds are published as unsigned AppImage, deb, and rpm assets for AMD64 and ARM64. Windows desktop builds are published as unsigned AMD64 NSIS and MSI installers. Verify the matching `.sha256` file before running or installing unsigned assets.

## Main Tabs

- **Overview**: command-center view with KPIs, a chronological Activity Pulse, project/tool spend, model spend, shell commands, and MCP servers.
- **Deep Dive**: analysis workbench with a larger activity trend, project rankings, top sessions, project/tool spend, model efficiency, core tools, shell commands, and MCP servers.
- **Session**: per-call session drill-down with clickable rows for full stored prompt, tool, command, and token metadata.
- **Usage**: four per-tool consoles with 24-hour activity pulses, call/token/cost summaries, plan limit gauges when available, and top model bars.
- **Config**: currency selection and confirmed local downloads for currency and pricing snapshots.

The desktop header mirrors the TUI filters: period, tool, sort mode, and project. In-window keyboard shortcuts are resolved through the same embedded keymap as the TUI; sort mode can be changed from the header or with `g`, and cycles between spend, latest date, and token use. `Shift-D` toggles between live and bundled sample data. The app polls snapshots in the background so completed refreshes appear without blocking the UI.

Dashboard sections render the full sorted row set. Sections with more rows than fit in the current window scroll inside the section so the header, filters, and footer remain visible.

## Reading The Dashboard

The **Activity Pulse** and **Activity Trend** panels use two stacked graph lines. Orange/red bars show relative spend, cyan/blue bars show relative call volume, and the footer summarizes the visible range, peak bucket, latest bucket, and total calls. 24 Hours and 7 Days use hourly buckets so short views do not collapse into one or two bars; 30 Days, Month, and All Time use daily buckets. The 24 Hours period is rolling from the current time, not a calendar-day midnight cutoff.

Ranked table bars use the same stepped color ramp as the TUI: blue is lower relative volume, yellow/orange is hotter, and red marks the current high end of the table. These bars are relative to the visible table, not exported pixel charts.

Usage consoles ignore the dashboard period and project filters because they are rolling 24-hour tool monitors. Empty tools stay visible with compact idle rows so you can still confirm that Codex, Claude Code, Cursor, and Copilot were checked.

## Background Alerts

Closing the desktop window keeps Token Use running in the background. Use the tray or menu-bar icon to show the window again, or choose **Quit Token Use** from that menu to stop the app. Opening Token Use while it is already running restores the existing window instead of launching a second copy.

While the app is running, the desktop backend keeps polling completed archive refreshes even if the window is hidden. If an automatic refresh imports a significant amount of new usage since the last alert baseline, Token Use sends a native desktop notification. Notifications are driven by all live imported usage, independent of the visible period, tool, project, or sort filters. Manual refreshes reset the alert baseline without sending a notification.

Background alert thresholds are configured in the shared `config.json` file:

```json
{
  "currency": "USD",
  "background_alerts": {
    "enabled": true,
    "min_cost_usd": 1.0,
    "min_tokens": 100000,
    "min_calls": 25,
    "cooldown_minutes": 30
  }
}
```

The defaults are conservative: notify after at least `$1.00`, `100k` tokens, or `25` calls of new usage, with a 30-minute cooldown. Linux tray click behavior depends on the desktop environment, so use the tray menu when a left-click does not restore the window. Windows notifications are most reliable from an installed build.

## Refresh

Use the refresh button or keyboard shortcut `r` to sync the archive. Refreshes use the same background archive refresher as the TUI and keep the previous data visible if a sync fails.

If sample data is selected manually with `Shift-D`, refreshes update the cached live data without switching the visible dashboard back until `Shift-D` is pressed again.

## Project, Session, Currency, And Export Pickers

The desktop app uses native dialogs where that makes sense:

- Project and session pickers include search.
- Session call rows open a detail modal with `Enter`, `Space`, or a mouse click.
- Currency selection writes the same `config.json` setting used by the TUI.
- Export format selection writes JSON, CSV, SVG, PNG, or a self-contained HTML/PDF workbook report from the current filtered view. Workbook exports also include the selected Session's full call detail when a session is open.
- Folder selection uses the Tauri dialog plugin and is runtime-only, matching TUI behavior.

## Shared Local Data

The desktop app and TUI share the platform config directory under `tokenuse`:

| File / directory | Shared purpose |
| --- | --- |
| `config.json` | User overrides, currently display currency |
| `archive.db` | Durable local usage archive |
| `rates.json` | Optional local currency snapshot |
| `pricing-snapshot.json` | Optional local LiteLLM-derived pricing snapshot |
| `exports/` | Fallback export directory |

Changing currency, refreshing the archive, or downloading local rates/pricing from the desktop app affects the same data the TUI reads.
