# Token Use

`tokenuse` is a Rust TUI for exploring AI coding tool token and cost usage. It reads session files already written on your machine, appends normalized records to its own archive, and renders a dense terminal dashboard for spend by day, project, tool, model, shell command, and MCP server.

Website: [tokenuse.app](https://www.tokenuse.app/)

There is no Anthropic / OpenAI platform API key, proxy, telemetry endpoint, daemon, or live file watcher. Usage ingestion stays local-only; outbound network is limited to explicit confirmed Config-page downloads, explicit Copilot / Claude.ai / ChatGPT quota sync, or maintainer refresh flags. The Claude.ai and ChatGPT (Codex) quota-sync features are opt-in: they call those services' user-facing usage endpoints only when you trigger a sync, using a session cookie you stored locally in the OS keychain.

## Quick Start

Install the terminal UI with Homebrew:

```bash
brew install russmckendrick/tap/tokenuse
tokenuse
```

Use a terminal at least `120x40`. Smaller terminals show a resize notice instead of the full dashboard.

If no local sessions are found, or archive sync fails before any calls are loaded, the app falls back to bundled sample data and shows that status in the title bar. Press `r` to sync sessions created after startup.

To explore the dashboard with bundled sample data even when local sessions exist, launch it with `tokenuse --sample`. Press `Shift-D` to switch back to the cached live data.

Install the Apple Silicon macOS desktop app with Homebrew Cask:

```bash
brew install --cask russmckendrick/tap/tokenuse-desktop
open -a "Token Use"
```

Linux, Windows, and manual desktop downloads are published on GitHub Releases. See [installation](docs/guides/installation.md) for platform-specific commands.

## Desktop App

The TUI remains the default app, and a Tauri v2 desktop shell lives under `desktop/` for macOS, Windows, and Linux local builds. It shares the same archive, config, currency, pricing, model registry, and report logic as the TUI. The desktop diverges visually into a sidebar application with Overview, Analytics, Graph, Coach, Scrollback, Models, Projects, Tools, and Config screens plus direct per-tool pages.

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

Raw model ids from every parser resolve through the shared [model-normalisation registry](docs/development/models.md), so dated and vendor variants fold into canonical provider and family rows before display.

## Dashboard

The dashboard shows:

- summary totals for cost, calls, sessions, cache hit rate, input, output, cache reads, and cache writes
- daily cost and call activity
- spend by project and by project/tool pair
- top sessions
- spend by model
- spend by task category (By Activity)
- core tool calls, shell command heads, and MCP server usage

Beyond Overview and Deep Dive, dedicated pages cover rolling 24-hour usage and rate limits (`u`), a Coach practice report card (`k`), and Scrollback full-text search across archived session transcripts (`/`).

Project names are normalized across tools. Absolute paths are folded to the nearest existing Git root when possible, then displayed with the shortest unique suffix.

## Keyboard

The TUI and desktop app share the same checked-in shortcut definitions from `src/keymap/keymap.json`.

- `q`: quit · `Esc`: close modal / back from sub-page
- `1`–`5`: period (24 hours, 7 days, 30 days, this month, all time)
- `t`: cycle tool filter · `g`: cycle sort mode (spend, latest date, token use)
- `p`: open project picker · `m`: open model picker (type to search; Backspace to clear last char; Ctrl-U to clear)
- `Tab` / `Shift-Tab`: cycle main tabs (Overview → Deep Dive → Usage → Coach → Scrollback)
- `o`: Overview · `d`: Deep Dive · `u`: Usage / rate limits · `k`: Coach practice report
- `/`: Scrollback transcript search (type a phrase, `Enter` to search, `Enter` again to open the matching session)
- `c`: open configuration · `s`: open session picker (drill into a single session's calls)
- `e`: generate a report (HTML, PDF, SVG, PNG, JSON, Excel, or CSV folder) to Downloads; press `f`/`b` in the report modal to choose another folder for this session
- `r`: reload (sync archive in place; keeps prior data on failure)
- `Shift-D`: toggle between live and sample data
- `h` or `?`: open the keybinding reference (full list of shortcuts)
- In the session page: `Up`/`Down`, `PgUp`/`PgDn`, `Home`/`End`, `Esc`/`d` back to the page that opened the session
- In pickers and configuration: `Up`/`Down`, `Home`/`End`, `Enter`, `Esc`

## Configuration

The dashboard stores user settings and downloaded data in the platform config directory under `tokenuse`. The files are:

- `config.json`: user overrides — display currency, plan prices, background alert thresholds, desktop preferences, and MCP server settings
- `archive.db`: durable local usage archive
- `exchange-rates.json`: latest downloaded published currency snapshot
- `rates.json`: legacy local currency snapshot, still read when `exchange-rates.json` is absent
- `pricing-upstream.json` and `pricing-overrides.json`: latest downloaded pricing books
- `pricing-snapshot.json`: legacy local pricing snapshot
- `mcp-salt` and `mcp-token`: MCP project-pseudonymisation salt and the bearer token for the opt-in HTTP endpoint
- `limits/claude-code.json`: optional Claude Code status-line limit sidecar
- `limits/copilot.json`: optional Copilot quota sidecar written by confirmed sync
- `limits/claude_subscription.json` and `limits/codex_subscription.json`: optional Claude.ai / ChatGPT (Codex) quota sidecars written by opt-in sync

USD remains the default. Costs are calculated and stored internally as import-time USD, then converted for display using the configured currency. Open the TUI configuration page with `c` to pick a currency, download the latest local data, sync Claude/Copilot limit sidecars, or clear and rebuild the local archive. Downloading `exchange-rates.json` asks for confirmation and updates display rates immediately; downloading pricing books asks for confirmation and applies to newly imported calls. Copilot quota sync asks for confirmation, reads existing local Copilot credentials, writes `limits/copilot.json`, and refreshes archive limits. Clear data also asks for confirmation, deletes `archive.db`, and immediately reimports from local tool history.

Default TUI and desktop builds include the confirmed download and quota sync actions. Build with `--no-default-features` when you need a no-download binary; those builds keep ingestion local-only and report Config-page downloads and Copilot quota sync as unavailable.

## CLI Helpers

Sync the archive and answer questions without opening the TUI:

```bash
tokenuse status            # one line with 24-hour and month totals (--json)
tokenuse overview          # copy-pasteable summary of this month (--json)
tokenuse doctor            # diagnose per-tool data discovery and parsing (--json)
tokenuse report            # guided report generator, same formats as `e` in the TUI
tokenuse mcp               # read-only MCP stdio server for LLM clients
tokenuse --list-projects   # print the ingested project inventory
```

`tokenuse mcp` exposes `status`, `overview`, `projects`, and `scrollback` (transcript search) tools. Project names are pseudonymised unless you pass `--real-names`, and `--http [--port N]` swaps stdio for a loopback-only, bearer-token-gated HTTP endpoint. See [the MCP server doc](docs/development/mcp-server.md) for registration examples.

Maintainer snapshot refresh commands are documented in [local development](docs/development/local-development.md). Do not hand-edit generated cost books such as `costs/exchange-rates.json`, `costs/pricing-upstream.json`, or `src/pricing/snapshot.json`; use the refresh commands so generated data stays consistent.

## Documentation

- [Documentation index](docs/README.md)
- [Tool ingestion details](docs/development/tools/README.md)
- [Model normalisation](docs/development/models.md)
- [Architecture and data flow](docs/development/architecture.md)
- [Desktop app usage](docs/guides/desktop-usage.md)
- [TUI usage guide (pages, keyboard, Scrollback, Usage)](docs/guides/tui-usage.md)
- [MCP server](docs/development/mcp-server.md)
- [Release notes](docs/releases/)

## Development

```bash
cargo fmt --check
cargo test
```

Bundled sample rows live in `src/data/sample_data.json` and are loaded through `src/data/mod.rs`. Live usage is loaded from the local archive in `src/archive.rs`, which syncs source files through the adapters in `src/tools/`.
