# Token Use

`tokenuse` is a Rust TUI for exploring AI coding tool token and cost usage. It reads session files already written on your machine, appends normalized records to its own archive, and renders a dense terminal dashboard for spend by day, project, tool, model, shell command, and MCP server.

Website: [tokenuse.app](https://www.tokenuse.app/)

There is no API key, proxy, telemetry endpoint, daemon, or live file watcher. Usage ingestion stays local-only; outbound network is limited to explicit confirmed Config-page downloads or maintainer refresh flags.

## Quick Start

Install the terminal UI with Homebrew:

```bash
brew install russmckendrick/tap/tokenuse
tokenuse
```

Use a terminal at least `120x40`. Smaller terminals show a resize notice instead of the full dashboard.

If no local sessions are found, or archive sync fails before any calls are loaded, the app falls back to bundled sample data and shows that status in the title bar. Press `r` to sync sessions created after startup.

Install the Apple Silicon macOS desktop app with Homebrew Cask:

```bash
brew install --cask russmckendrick/tap/tokenuse-desktop
open -a "Token Use"
```

Linux, Windows, and manual desktop downloads are published on GitHub Releases. See [installation](docs/guides/installation.md) for platform-specific commands.

## Desktop App

The TUI remains the default app, and a Tauri v2 desktop shell lives under `desktop/` for macOS, Windows, and Linux local builds. It shares the same archive, config, currency, pricing, and export logic as the TUI.

```bash
cd desktop
pnpm install
pnpm run tauri:dev
```

See [desktop app usage](docs/guides/desktop-usage.md) for shared-data behavior, and [local development](docs/development/local-development.md) for build notes.

## Supported Tools

| Tool | Sources | Notes |
| --- | --- | --- |
| Claude Code | `~/.claude/projects/` and Claude Desktop local agent sessions | Exact token/cache usage, tool calls, Bash commands, MCP tools |
| Cursor | Cursor `state.vscdb` | Exact tokens when present; `chars / 4` estimates for Agent KV and zero-token rows |
| Codex | `~/.codex/sessions/**/rollout-*.jsonl` | Per-turn `token_count` events, cached input, reasoning output, tool calls |
| GitHub Copilot | `~/.copilot/session-state/` and VS Code Copilot Chat transcripts | Legacy output tokens when present; transcript usage is estimated |

Details for each parser live under [docs/development/tools](docs/development/tools/README.md).

## Dashboard

The dashboard shows:

- summary totals for cost, calls, sessions, cache hit rate, input, output, cache reads, and cache writes
- daily cost and call activity
- spend by project and by project/tool pair
- top sessions
- spend by model
- core tool calls, shell command heads, and MCP server usage

Project names are normalized across tools. Absolute paths are folded to the nearest existing Git root when possible, then displayed with the shortest unique suffix.

## Keyboard

The TUI and desktop app share the same checked-in shortcut definitions from `src/keymap/keymap.json`.

- `q`: quit · `Esc`: close modal / back from sub-page
- `1`–`5`: period (24 hours, 7 days, 30 days, this month, all time)
- `t`: cycle tool filter
- `p`: open project picker (type to search; Backspace to clear last char; Ctrl-U to clear)
- `Tab` / `Shift-Tab`: cycle main tabs (Overview ↔ Deep Dive ↔ Usage)
- `o`: Overview · `d`: Deep Dive · `u`: Usage / rate limits
- `c`: open configuration · `s`: open session picker (drill into a single session's calls)
- `e`: export current view (JSON, CSV, SVG, PNG, HTML, PDF) to Downloads; press `f`/`b` in the export modal to choose another folder for this session
- `r`: reload (sync archive in place; keeps prior data on failure)
- `h` or `?`: open the keybinding reference (full list of shortcuts)
- In the session page: `Up`/`Down`, `PgUp`/`PgDn`, `Home`/`End`, `Esc`/`d` back to Deep Dive
- In pickers and configuration: `Up`/`Down`, `Home`/`End`, `Enter`, `Esc`

## Configuration

The dashboard stores user settings and downloaded data in the platform config directory under `tokenuse`. The files are:

- `config.json`: user overrides, currently the display currency
- `archive.db`: durable local usage archive
- `rates.json`: latest downloaded published currency snapshot
- `pricing-snapshot.json`: latest downloaded LiteLLM-derived pricing snapshot

USD remains the default. Costs are calculated and stored internally as import-time USD, then converted for display using the configured currency. Open the TUI configuration page with `c` to pick a currency, download the latest local data, or clear and rebuild the local archive. Downloading `rates.json` asks for confirmation and updates display rates immediately; downloading LiteLLM pricing asks for confirmation and applies to newly imported calls. Clear data also asks for confirmation, deletes `archive.db`, and immediately reimports from local tool history.

Default TUI and desktop builds include the confirmed download actions. Build with `--no-default-features` when you need a no-download binary; those builds keep ingestion local-only and report Config-page downloads as unavailable.

## CLI Helper

Sync the archive and list normalized project/tool rows without opening the TUI:

```bash
tokenuse --list-projects
```

Maintainer snapshot refresh commands are documented in [local development](docs/development/local-development.md). Do not hand-edit `src/pricing/snapshot.json` or `currency/rates.json`; use the refresh commands so generated data stays consistent.

## Documentation

- [Documentation index](docs/README.md)
- [Tool ingestion details](docs/development/tools/README.md)
- [Architecture and data flow](docs/development/architecture.md)
- [Desktop app usage](docs/guides/desktop-usage.md)
- [Usage page (rolling 24h utilisation)](docs/guides/tui-usage.md#usage-page)
- [Release notes](docs/releases/)

## Development

```bash
cargo fmt --check
cargo test
```

Sample dashboard data lives in `src/data/mod.rs`. Live usage is loaded from the local archive in `src/archive.rs`, which syncs source files through the adapters in `src/tools/`.
