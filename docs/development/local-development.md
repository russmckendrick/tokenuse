# Local Development

## TUI

Run the terminal UI from the repository root:

```bash
cargo run
```

Run standard Rust checks:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features
```

List normalized project/tool rows without opening the TUI:

```bash
cargo run -- --list-projects
```

## Desktop App

Install desktop frontend dependencies once:

```bash
cd desktop
pnpm install
```

Launch the Tauri development app:

```bash
pnpm run tauri:dev
```

Run desktop checks and builds:

```bash
pnpm run check
pnpm run build
pnpm tauri build --target aarch64-apple-darwin --bundles app --ci
cd src-tauri
cargo check
```

## Generated Pricing And Currency Data

Refresh the embedded pricing books:

```bash
cargo run -- --refresh-prices
```

Refresh the checked-in Frankfurter-derived currency snapshot:

```bash
cargo run -- --generate-currency-json
```

Do not hand-edit `costs/exchange-rates.json`, `costs/pricing-upstream.json`, or `src/pricing/snapshot.json`; use the refresh commands so generated data stays consistent. Curated pricing overrides and source extraction rules live in `costs/pricing-overrides.json` and `costs/pricing-sources.json`.

The pricing books are also refreshed by `.github/workflows/refresh-pricing.yml` weekly and on manual dispatch. The currency snapshot uses `.github/workflows/refresh-currency.yml` weekly and on manual dispatch.

## No-Download Builds

Default TUI and desktop builds include confirmed Config-page downloads for `exchange-rates.json`, `pricing-upstream.json`, and `pricing-overrides.json`, plus confirmed Copilot quota sync. Build with `--no-default-features` when you need a no-download binary; those builds keep ingestion local-only and report Config-page downloads and Copilot quota sync as unavailable.
