# Token Use

`tokenuse` is a Rust TUI for exploring AI token and cost usage across local provider session files. It ingests sessions once at startup and presents a dense terminal dashboard focused on spend by day, project, provider, model, tools, commands, and MCP servers.

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

## Development

```bash
cargo fmt --check
cargo test
```

Sample dashboard data lives in `src/data.rs`; live provider ingestion is loaded from local files at startup.
