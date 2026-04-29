# AGENTS.md

## Tooling

- Update embedded pricing: `cargo run --features refresh-prices -- --refresh-prices` - never hand-edit `src/pricing/snapshot.json`.
- Update embedded currency rates: `cargo run --features refresh-currency -- --generate-currency-json` - never hand-edit `currency/rates.json`.
- Default build has zero network deps; `ureq` is gated behind the `refresh-prices` and `refresh-currency` features.

## Non-obvious rules

- Consult `DESIGN.md` before any UI/theme change - the color tokens, density rules, and "no rounded card styling" guidance are enforced.
- Ingest runs **once at startup**. There is no live file watching; re-launch to pick up new sessions.
- `DashboardData` fields are `&'static str`. Sample data uses string literals; ingested data is leaked via the `leak()` helper in `src/ingest.rs`. Do not change these to `String` without auditing every renderer.
- The dashboard reads local files directly - no API keys, no proxy, no telemetry. Don't add network calls outside the `refresh-prices` feature.

## Tool Adapter Conventions

- User-facing docs and UI call Claude Code, Cursor, Codex, and Copilot **tools**. The Rust adapter trait is named `ToolAdapter` and lives under `src/tools/`.
- Each tool adapter lives in `src/tools/<name>/{mod,config,discovery,parser}.rs`. **All paths, env vars, globs, and SQL queries belong in that adapter's `config.rs`** - not in a shared top-level config.
- Adding a tool: write the four adapter files, register it in `tools::registry()` (`src/tools/mod.rs`), add a variant to `app::Tool`, update `ingest::matches_tool`, update display labels such as `tool_short_label`, and write `docs/tools/<name>.md`.
- `config::TOOL_ID` must match the literal `ingest::matches_tool` compares against - they are stringly typed across the boundary.
- Claude Code, Cursor, Codex, and Copilot all have implemented parsers. Read `docs/tools/<name>.md` for the source schema and parser caveats before changing one.
