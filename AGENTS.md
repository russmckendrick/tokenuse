# AGENTS.md

## Tooling

- Before finishing code changes, run `cargo clippy -- -D warnings` as part of the local test pass.
- Run desktop `pnpm` commands non-interactively with `CI=true` (for example `CI=true pnpm run check`) so pnpm never stops on a no-TTY module purge prompt.
- If a required dependency/check command fails because the sandbox blocks registry or network access, rerun the same command with network approval instead of retrying in the sandbox.
- Update embedded pricing: `cargo run -- --refresh-prices` - never hand-edit `src/pricing/snapshot.json`.
- Update embedded currency rates: `cargo run -- --generate-currency-json` - never hand-edit `currency/rates.json`.
- Default builds include confirmed Config-page downloads for local rates/pricing files. Use `--no-default-features` for a no-download build; `ureq` remains gated behind the `refresh-prices` and `refresh-currency` features.

## Non-obvious rules

- Consult `DESIGN.md` before any UI/theme change - the color tokens, density rules, and "no rounded card styling" guidance are enforced.
- Ingest results are cached at `~/.cache/tokenuse/ingest-cache.json` (TTL 15 min). On startup a fresh cache is reused so the dashboard opens fast; a stale or missing cache falls through to a synchronous ingest. A background refresher then re-runs ingest every 15 min and on the 'r' key, writing back to the cache. There is no live file watching - the timer is the only auto-refresh signal. Subcommands like `--list-projects` always run a fresh ingest and bypass the cache.
- `DashboardData` fields are `&'static str`. Sample data uses string literals; ingested data is leaked via the `leak()` helper in `src/ingest.rs`. Do not change these to `String` without auditing every renderer.
- The dashboard reads usage files directly - no API keys, no proxy, no telemetry. Don't add network calls outside explicit Config-page downloads or maintainer refresh feature paths.

## Documentation

- Add user-visible changes that affect shipped behavior or documented workflows to `docs/releases/unreleased.md`.
- Do not bump `Cargo.toml`, edit `Cargo.lock` just for a version change, or create a numbered `docs/releases/<version>.md` unless Russ explicitly asks to prep a release.
- When Russ asks to prep a release, choose a short human-readable release name, move the relevant `unreleased.md` notes into `docs/releases/<version>.md` using that name in the title and intro, then bump `Cargo.toml` and let Cargo update `Cargo.lock` in the same change.
- Keep `docs/architecture.md`, `docs/usage.md`, and the `docs/tools/<name>.md` files in sync with the code they describe. If you change page routing, ingestion behavior, or a tool adapter's source schema, update the matching doc in the same change.
- Update `docs/README.md` whenever a new top-level doc lands so the index stays current.

## Tool Adapter Conventions

- User-facing docs and UI call Claude Code, Cursor, Codex, and Copilot **tools**. The Rust adapter trait is named `ToolAdapter` and lives under `src/tools/`.
- Each tool adapter lives in `src/tools/<name>/{mod,config,discovery,parser}.rs`. **All paths, env vars, globs, and SQL queries belong in that adapter's `config.rs`** - not in a shared top-level config.
- Adding a tool: write the four adapter files, register it in `tools::registry()` (`src/tools/mod.rs`), add a variant to `app::Tool`, update `ingest::matches_tool`, update display labels such as `tool_short_label`, and write `docs/tools/<name>.md`.
- `config::TOOL_ID` must match the literal `ingest::matches_tool` compares against - they are stringly typed across the boundary.
- Claude Code, Cursor, Codex, and Copilot all have implemented parsers. Read `docs/tools/<name>.md` for the source schema and parser caveats before changing one.
