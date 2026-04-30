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
pnpm run tauri:build:app
cd src-tauri
cargo check
```

## Generated Pricing And Currency Data

Refresh the embedded LiteLLM-derived pricing snapshot:

```bash
cargo run -- --refresh-prices
```

Refresh the checked-in Frankfurter-derived currency snapshot:

```bash
cargo run -- --generate-currency-json
```

Do not hand-edit `src/pricing/snapshot.json` or `currency/rates.json`; use the refresh commands so generated data stays consistent.

## No-Download Builds

Default TUI and desktop builds include confirmed Config-page downloads for `rates.json` and `pricing-snapshot.json`. Build with `--no-default-features` when you need a no-download binary; those builds keep ingestion local-only and report Config-page downloads as unavailable.
