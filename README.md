# tokenuse

`tokenuse` is a Rust TUI prototype for exploring AI token and cost usage. This MVP uses dummy data and focuses on the dashboard layout, terminal styling, and basic keyboard interaction shown in the reference screens.

## Run

```bash
cargo run
```

The dashboard is designed for a wide terminal. Use at least `120x40` for the full layout.

## Keyboard

- `q` or `Esc`: quit
- `1`: today
- `2`: week
- `3`: 30 days
- `4`: month
- `5`: all time
- `p`: toggle provider
- `o`: mark optimize view
- `c`: mark compare view
- `Tab`, `<`, or `>`: cycle view state

## Development

```bash
cargo fmt --check
cargo test
```

The current dashboard data is static and lives in `src/data.rs`; real ingestion, persistence, and provider integrations are intentionally out of scope for this prototype.
