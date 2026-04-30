# Desktop App Usage

The desktop app is a Tauri v2 + Svelte frontend over the same Rust core as the TUI. It shares the local archive, config, currency, pricing, refresh, session drill-down, and export logic.

## Install And Open

Install on macOS with Homebrew Cask:

```bash
brew install --cask russmckendrick/tap/tokenuse
open -a "Token Use"
```

The desktop app currently ships as a signed and notarized universal macOS DMG. Linux and Windows desktop installers are intentionally deferred; use the TUI binaries on those platforms.

## Main Tabs

- **Overview**: KPIs, daily activity, model spend, project spend, and common commands.
- **Deep Dive**: denser tables for projects, sessions, models, tools, commands, and MCP servers.
- **Usage**: rolling 24-hour per-tool activity with Codex limit snapshots when available.
- **Config**: currency selection and confirmed local downloads for currency and pricing snapshots.

The desktop header mirrors the TUI filters: period, tool, and project. The app polls snapshots in the background so completed refreshes appear without blocking the UI.

## Refresh

Use the refresh button or keyboard shortcut `r` to sync the archive. Refreshes use the same background archive refresher as the TUI and keep the previous data visible if a sync fails.

## Project, Session, Currency, And Export Pickers

The desktop app uses native dialogs where that makes sense:

- Project and session pickers include search.
- Currency selection writes the same `config.json` setting used by the TUI.
- Export format selection writes JSON, CSV, SVG, or PNG from the current filtered view.
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
