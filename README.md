# Token Use

`tokenuse` is a local-only Rust TUI for exploring AI coding tool token and cost usage. It reads session files already written on your machine, ingests them once at startup, and renders a dense terminal dashboard for spend by day, project, tool, model, shell command, and MCP server.

There is no API key, proxy, telemetry endpoint, or background watcher. The default build has no network dependency; the only networked paths are the explicit pricing and currency refresh features.

## Quick Start

```bash
cargo run
```

Use a terminal at least `120x40`. Smaller terminals show a resize notice instead of the full dashboard.

If no local sessions are found, or ingestion fails before any calls are loaded, the app falls back to bundled sample data and shows that status in the title bar. Re-launch the app to pick up sessions created after startup.

## Supported Tools

| Tool | Sources | Notes |
| --- | --- | --- |
| Claude Code | `~/.claude/projects/` and Claude Desktop local agent sessions | Exact token/cache usage, tool calls, Bash commands, MCP tools |
| Cursor | Cursor `state.vscdb` | Exact tokens when present; `chars / 4` estimates for Agent KV and zero-token rows |
| Codex | `~/.codex/sessions/**/rollout-*.jsonl` | Per-turn `token_count` events, cached input, reasoning output, tool calls |
| GitHub Copilot | `~/.copilot/session-state/` and VS Code Copilot Chat transcripts | Legacy output tokens when present; transcript usage is estimated |

Details for each parser live under [docs/tools](docs/tools/README.md).

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

- `q` or `Esc`: quit
- `1`: today
- `2`: 7 days
- `3`: 30 days
- `4`: this month
- `5`: all time
- `t`: cycle tool filter
- `p`: open project picker
- In the project picker: `Up` / `Down`, `Home`, `End`, `Enter`, `Esc`

## CLI Helpers

List normalized project/tool rows without opening the TUI:

```bash
cargo run -- --list-projects
```

Refresh the embedded LiteLLM-derived pricing snapshot:

```bash
cargo run --features refresh-prices -- --refresh-prices
```

Refresh the checked-in Frankfurter-derived currency snapshot:

```bash
cargo run --features refresh-currency -- --refresh-currency
```

Do not hand-edit `src/pricing/snapshot.json` or `currency/rates.json`; use the refresh commands so generated data stays consistent.

## Documentation

- [Documentation index](docs/README.md)
- [Tool ingestion details](docs/tools/README.md)
- [Architecture and data flow](docs/architecture.md)

## Development

```bash
cargo fmt --check
cargo test
```

Sample dashboard data lives in `src/data.rs`. Live ingestion is loaded from local files at startup and normalized through the adapters in `src/tools/`.
