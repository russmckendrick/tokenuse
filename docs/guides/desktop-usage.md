# Desktop App Usage

The desktop app is a Tauri v2 and Svelte frontend over the same local Rust core as the TUI. It shares the archive, configuration, currency and pricing books, sample data, refresh worker, session details, quota snapshots, and report generator. It does not send usage telemetry or require provider API keys.

## Install And Open

Install on Apple Silicon macOS with Homebrew Cask:

```bash
brew install --cask russmckendrick/tap/tokenuse-desktop
open -a "Token Use"
```

The macOS app also ships as a signed and notarized Apple Silicon DMG. Linux builds are published as unsigned AppImage, deb, and rpm assets for AMD64 and ARM64. Windows builds are published as unsigned AMD64 NSIS and MSI installers. Verify the matching `.sha256` file before running an unsigned asset.

## Application Shell

The desktop app uses a persistent left sidebar rather than the TUI tab strip. Every primary screen and every supported tool has a direct entry:

- Overview
- Analytics
- Tools
- Models
- Projects
- Claude Code, Cursor, Codex, Copilot, and Gemini
- Config

Use **Collapse** at the bottom of the sidebar to reduce it to an icon rail. The choice is remembered locally. The active screen remains highlighted in either state.

The five direct tool rows dynamically order themselves from highest to lowest rolling 24-hour call activity, so the tools currently driving usage stay closest to the main views. Primary screen and Config positions never move.

The header holds the controls that apply to the current screen in one toolbar: title, period, contextual tool/project/sort filters, refresh, then report. At compact window widths the filter labels collapse to icons while their current values remain visible. Overview and Analytics expose period, tool, sort, and project filters. Dedicated tool pages expose period and sort. Models uses the active period for ranking and details while keeping all five ranges visible in its table. Projects exposes period, sort, and project. The parent Tools screen is a fixed rolling 24-hour capacity view, so its period control is intentionally hidden.

The footer shows live or sample source, currency, and context-sensitive shortcut hints. Refresh, report, configuration, and sync results appear as temporary bottom-right toasts instead of permanently consuming header space.

Long tables and result lists have bounded heights, sticky column headings, and their own scrollbars. Selecting All Time therefore adds scrollable rows without stretching every panel on the page.

## Screens

### Overview

Overview is the at-a-glance command center. It leads with cost, calls, sessions, cache hit rate, and input/output totals, followed by current utilisation grouped by tool, a chronological activity pulse, top projects, and top models.

Utilisation gauges come from the latest non-stale plan snapshots. Each compact tool module starts with its mark inside a ring showing the most constrained active window, followed by a two-column matrix retaining the exact percentage or credits remaining and reset timing for its primary limits. Claude Extra Usage and Codex Spark model-specific windows stay available on their dedicated tool pages rather than appearing in this summary. A single limit spans both columns, and the modules stack only at narrow window sizes. The section is omitted when no current snapshot exists.

### Analytics

Analytics is the time and distribution workspace. It includes:

- chronological activity for the selected period;
- daily spend stacked by tool;
- an hour-by-weekday activity heatmap;
- provider and tool share donuts;
- cache efficiency;
- ranked projects, models, sessions, project/tool rows, core tools, shell commands, and MCP servers.

Charts use the same token-driven colors and relative ranking language as the TUI. Hovered chart values are exact for that bucket; bars, heat intensity, and rank strips are relative to the visible dataset.

### Coach

Coach turns your local usage history into a practice report card. It is entirely on-device — the analysis is deterministic Rust over the archive, with no network or AI calls (see [Coach engine](../development/coach.md) for the algorithms and attribution).

- **Report card**: a full-width row of document-style tabs switches between Report, Findings, AI Output, and Activity. Report opens with your overall letter grade (the rules-weighted mean of the four practice scores) in a radial gauge, alongside rules-clean, findings, day-streak, and code-output tiles. These large summary rows stay out of the deeper analysis tabs so those views can use the full window.
- **Practice scores**: four cards (Prompt Quality, Session Hygiene, Code Review, Tool Mastery), each 0–100 with a letter-grade badge, a weekly trend sparkline, week-over-week and month-over-month deltas, and the heaviest triggered rule.
- **Report tab**: the highest-penalty findings surface first as advice cards; selecting one opens that rule directly in the Findings evidence detail. Equal-depth Flow and Pace panels follow. Flow uses a score, four practical KPIs, and a recent sparkline; Pace shows streaks, late-night and weekend gauges, and a burnout-risk badge with specific alerts.
- **Findings tab**: severity and occurrence KPIs sit above a filterable master/detail explorer. Pick a finding to inspect why the rule matters, its denominator and trigger rate, the suggested next move, and every representative example captured for that signal.
- **AI Output tab**: total output, active days, daily average, peak day, top language, and top model appear first as comparable KPIs. A full-width chart uses 30-minute buckets for 24 Hours, hourly buckets for 7 Days, complete daily timelines for 30 Days and This Month, and monthly buckets across the full recorded history for All Time. Generated-code bars combine with a matching three-bucket moving average and selectable interval details. The language/model/project ranking remains directly below it.
- **Activity tab**: Work Hours shows the hour×weekday intensity grid, weekday/weekend hourly profile, and period trend. Calendar is a daily activity bar strip (turns per day) that follows the selected period: scoped periods show a trailing ~2-month window with out-of-period bars dimmed (still clickable), All Time shows the trailing year. Picking a day exposes the session table — one row per session with its time range, project, tool, turns, cost, and timeline track — and selecting a row opens the fixed call inspector. Projects ranks the active projects with spend, calls, sessions, AI LoC, and tool coverage.

Findings respect the header tool and project filters. Rules only count tools that can actually produce a signal — a tool whose logs lack, say, cancellation events never inflates a cancellation rate. Older archived calls whose source files are gone are excluded from rule denominators rather than counted as clean.

### Tools

The parent Tools screen shows one rolling 24-hour console for each supported tool. Every tool stays visible, including idle tools, so it is clear which sources were checked. A console combines recent cost, calls, tokens, last-seen time, plan-limit gauges, and top models.

The direct tool entries open dedicated pages with the selected time range. Each page has larger cost, call, session, and cache summaries, the tool's current utilisation console, top projects, top models, and sessions. Tool marks are displayed without decorative icon boxes so the summary row has more room for data.

Copilot AI Credits rows show exact used and remaining/total credits, reset time, plan, and additional-usage status. Business or Enterprise payloads that hide per-seat credits show an organization-managed row rather than a blank console. A limit whose reset has passed, or whose reset-less snapshot is over a week old, dims as stale and is hidden one week later.

### Models

Models is a provider-grouped catalog across every tool. Each model row carries its canonical display name, family, cache hit rate, and cost/call totals for 24 Hours, 7 Days, 30 Days, This Month, and All Time.

The active header period controls row ranking and the expanded details. Select a row with a click, `Enter`, or `Space` to reveal the per-tool split for that period. Equivalent dated ids and vendor paths fold into one row; automatic routers retain their actual provider attribution, such as **OpenAI (auto)** or **Anthropic (auto)**.

### Projects

Projects is a drill-down from project to session to call. Select a project to list its sessions, then select a session to open its call rows. Call details include the full stored prompt, model, token buckets, pricing rates, tools, reasoning/web-search counts, shell commands, interaction mode, and exact/estimated timestamp and token quality when those fields were available locally. Modern Cursor Agent sessions appear here as one call per user request rather than separate overlapping bubble, AgentKv, and transcript rows.

The project picker can narrow the page to one normalized project identity. Project labels use the shortest unique path suffix, while the archive retains the raw project value for debugging and reports.

### Config

Config groups shared data settings and desktop-only behavior:

- display currency;
- live/sample **Sample Data** toggle;
- confirmed currency and pricing-book downloads;
- Claude Code status-line setup and Claude/Copilot limit sync;
- optional Claude.ai and ChatGPT quota-cookie sync when the build supports it;
- on-demand per-tool data-source diagnostics (**Data Sources**);
- clear and rebuild local archive;
- report folder;
- open at login and Dock/taskbar visibility;
- Windows/Linux update checks.

Turning on Sample Data changes only the visible source. Any live snapshot remains cached, background refreshes continue updating it, and turning Sample Data off returns to that live snapshot.

The **Data Sources** panel runs the same read-only diagnostics as `tokenuse doctor`, on demand. **Run diagnostics** reports, for every tool adapter, the locations it probes (found or missing), the environment overrides in effect, how many session and limit sources discovery found, and a bounded parse sample, ending in an OK / NOTHING FOUND / ERRORS / DISCOVERY FAILED verdict with the likely cause. Because the checks re-walk the filesystem and parse sample files, they run only when the button is pressed; the result and its timestamp stay in the window until the next run and are never part of the background snapshot poll.

When any model in the archive is billed at the fallback pricing rate, the pricing row's warning (the affected `tool · model` pairs and the fix hint) is shown in the warning tone so guessed costs stand out from real ones.

## Keyboard

Desktop navigation is resolved in the Svelte shell; data actions call typed Rust commands directly. Shortcuts are ignored while typing in an input, select, or text area, except for `Esc`.

| Key | Action |
| --- | --- |
| `Tab` / `Shift-Tab` | Cycle Overview, Analytics, Coach, Tools, Models, Projects, and Config. |
| `o` | Open Overview. |
| `d` | Open Analytics. |
| `h` | Open Coach. |
| `u` | Open Tools. |
| `c` | Open Config. |
| `1`–`5` | Select 24 Hours, 7 Days, 30 Days, This Month, or All Time where the period is available. |
| `t` | Cycle tool on Overview or Analytics. |
| `g` | Cycle spend, latest-date, and token-use sorting where sorting is available. |
| `p` | Open the project picker on Overview, Analytics, or Projects. |
| `s` | Open the session picker. |
| `e` | Open report generation. |
| `r` | Refresh the archive. |
| `Shift-D` | Toggle live and sample data. |
| `Esc` | Close the active detail/modal, or return from Session to Analytics. |

Clickable model, project, session, and call rows also support `Enter` and `Space`.

## Time Ranges And Charts

24 Hours is a rolling window from the current time, not a calendar day. Activity charts use hourly buckets for 24 Hours and 7 Days. This Month is hourly through day 14 and daily from day 15 onward; 30 Days and All Time use daily buckets.

Spend bars use the warm chart series and call cadence uses the cool series. Ranked bars and heat cells show relative magnitude within the visible panel. Use the adjacent numeric values for exact cost, call, token, reset, and plan values.

## Tray And Background Alerts

Closing the main window keeps Token Use running. Left-click the tray or menu-bar icon for Quick View, or use the tray menu when the desktop environment does not support left-click activation. Quick View shows compact 24-hour cost, call, token, and cache totals plus the four most urgent current utilisation windows with percentages or credits remaining and reset times. Choose **Open** to restore the full app; choose **Quit Token Use** from the tray menu to stop it.

The backend continues draining completed archive refreshes while the window is hidden. Automatic refreshes can send native notifications when new live usage crosses a configured cost, token, or call threshold. Manual refreshes reset the alert baseline without notifying. Visible filters and sample mode do not change the all-live-data alert baseline.

Background alert and desktop defaults live in `config.json`:

```json
{
  "currency": "USD",
  "background_alerts": {
    "enabled": true,
    "min_cost_usd": 1.0,
    "min_tokens": 100000,
    "min_calls": 25,
    "cooldown_minutes": 30
  },
  "desktop": {
    "open_at_login": false,
    "show_dock_or_taskbar_icon": true
  }
}
```

Windows notifications are most reliable from an installed build. On Windows and Linux, Config can check GitHub Releases for updates. Windows uses the NSIS installer and Linux in-app updates target AppImage installs; deb/rpm users update through their package workflow. macOS updates continue through Homebrew Cask or a new DMG.

## Refresh, Reports, And Local Data

Use the header refresh button or `r` to sync the archive in the background. The previous snapshot remains visible if a refresh fails. Clear Data asks for confirmation, deletes `archive.db`, and immediately reimports existing local history. Configuration, rates, pricing books, limit sidecars, and reports are retained; archive-only history is lost if its original source files no longer exist.

Report generation has independent period, project/all-projects, format, and redaction controls. It writes executive HTML/PDF decks, SVG/PNG visual summaries, JSON, Excel, or a CSV folder. Reports include all tools for the selected period and project scope. Folder selection uses the native dialog and applies to the running session.

The desktop app and TUI share the platform config directory under `tokenuse`:

| File / directory | Purpose |
| --- | --- |
| `config.json` | Currency, background alerts, and desktop preferences. |
| `archive.db` | Durable normalized calls, limits, and source fingerprints. |
| `exchange-rates.json` / `rates.json` | Current and legacy local currency snapshots. |
| `pricing-upstream.json` / `pricing-overrides.json` | Optional local pricing books. |
| `pricing-snapshot.json` | Legacy local pricing snapshot. |
| `limits/` | Claude Code, Copilot, and optional subscription-quota sidecars. |
| `reports/` | Fallback report directory. |

Changing currency, refreshing or clearing the archive, downloading books, or syncing limit sidecars affects the same persistent state the TUI reads. The Sample Data toggle is intentionally runtime-only for the current desktop process.

The Claude Code status-line setup creates an OS-specific wrapper under `<config>/tokenuse/statusline/`, backs up `~/.claude/settings.json`, and wraps any existing status-line command so its visible output is unchanged. **Generate wrapper only** leaves user configuration untouched; **Uninstall** restores the prior command and removes the wrapper while retaining the last sidecar JSON.
