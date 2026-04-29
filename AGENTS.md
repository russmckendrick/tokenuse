# AGENTS.md

## Tooling

- Update embedded pricing: `cargo run --features refresh-prices -- --refresh-prices` — never hand-edit `src/pricing/snapshot.json`.
- Default build has zero network deps; `ureq` is gated behind the `refresh-prices` feature.

## Non-obvious rules

- Consult `DESIGN.md` before any UI/theme change — the color tokens, density rules, and "no rounded card styling" guidance are enforced.
- Ingest runs **once at startup**. There is no live file watching; re-launch to pick up new sessions.
- `DashboardData` fields are `&'static str`. Sample data uses string literals; ingested data is leaked via the `leak()` helper in `src/ingest.rs`. Do not change these to `String` without auditing every renderer.
- The dashboard reads local files directly — no API keys, no proxy, no telemetry. Don't add network calls outside the `refresh-prices` feature.

## Provider conventions

- Each provider lives in `src/providers/<name>/{mod,config,discovery,parser}.rs`. **All paths, env vars, globs, and SQL queries belong in that provider's `config.rs`** — not in a shared top-level config.
- Adding a provider: write the four files, register in `providers::registry()` (`src/providers/mod.rs`), add a variant to `app::Provider`, update `ingest::matches_provider`, and write `docs/providers/<name>.md`.
- `config::PROVIDER_ID` must match the literal `ingest::matches_provider` compares against — they're stringly-typed across the boundary.
- Cursor, Codex, and Copilot are scaffolds: discovery + config are real, parsers return `Ok(vec![])`. Read `docs/providers/<name>.md` for the schema before implementing.
