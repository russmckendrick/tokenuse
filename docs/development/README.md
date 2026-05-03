# Development

This section is for maintainers and contributors working on `tokenuse` itself.

## Source Layout

| Path | Purpose |
| --- | --- |
| `src/` | Rust TUI, ingestion, archive, pricing, currency, export, and shared runtime |
| `src/tools/` | Tool adapter registry plus parser implementations |
| `desktop/` | Tauri v2 desktop frontend and desktop Rust commands |
| `currency/` | Generated embedded currency snapshot data |
| `docs/` | Source docs consumed by the website |
| `.github/workflows/` | CI, release, currency refresh, and Homebrew tap automation |

## Read Next

- [Architecture](architecture.md): how calls flow from local files to archive, aggregation, and UI.
- [Local development](local-development.md): the commands to run before sending changes.
- [Source control](source-control.md): branch, generated data, version, and release-note conventions.
- [Deployments](deployments.md): release assets, desktop notarization, and Homebrew tap updates.
- [Tool parsers](tools/README.md): how adapters discover, parse, deduplicate, and price local tool files.
