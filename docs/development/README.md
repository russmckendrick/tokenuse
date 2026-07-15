# Development

This section is for maintainers and contributors working on `tokenuse` itself.

## Source Layout

| Path | Purpose |
| --- | --- |
| `src/` | Rust TUI, ingestion, archive, pricing, currency, reports, and shared runtime |
| `src/tools/` | Tool adapter registry plus parser implementations |
| `src/models/` | Shared model identity registry, provider metadata, and fallback naming |
| `desktop/` | Tauri v2 desktop frontend and desktop Rust commands |
| `costs/` | Generated embedded cost data and pricing refresh config |
| `docs/` | Source docs consumed by the website |
| `.github/workflows/` | CI, release, currency/pricing refresh, and Homebrew tap automation |

## Read Next

- [Architecture](architecture.md): how calls flow from local files to archive, aggregation, and UI.
- [Coach engine](coach.md): the desktop Coach page's rules, scoring, and analyzers, with per-rule thresholds and attribution.
- [Model normalisation](models.md): how raw model ids become canonical model, provider, and family identities.
- [Pricing and cache rates](pricing.md): provider evidence, cache-read multipliers, parser caveats, and pricing book refresh behavior.
- [Local development](local-development.md): the commands to run before sending changes.
- [Source control](source-control.md): branch, generated data, version, and release-note conventions.
- [Deployments](deployments.md): release assets, desktop notarization, and Homebrew tap updates.
- [Tool parsers](tools/README.md): how adapters discover, parse, deduplicate, and price local tool files.
