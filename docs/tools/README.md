# Tool Ingestion

`tokenuse` reads usage data **directly from local files** written by AI coding tools. There is no proxy, no API key, no telemetry endpoint, and no live watcher.

The UI calls these sources **tools**. Internally each tool is implemented as a `Provider` adapter under `src/providers/<name>/`.

## Supported Tools

| Tool | Status | Source format | Token quality | Doc |
| --- | --- | --- | --- | --- |
| Claude Code | implemented | JSONL session files under `~/.claude/projects/` and Claude Desktop agent sessions | exact usage, cache reads/writes, tool calls | [claude-code.md](claude-code.md) |
| Cursor | implemented | SQLite `state.vscdb` | exact when `tokenCount` exists; estimated fallback otherwise | [cursor.md](cursor.md) |
| Codex | implemented | JSONL rollouts under `~/.codex/sessions/` | exact per-turn token-count deltas | [codex.md](codex.md) |
| GitHub Copilot | implemented | JSONL events from legacy CLI and VS Code Copilot Chat transcripts | legacy output exact when present; transcripts estimated | [copilot.md](copilot.md) |

## Data Path

```mermaid
flowchart LR
    A["local tool files"] --> B["adapter.discover()"]
    B --> C["Vec<SessionSource>"]
    C --> D["adapter.parse(source, seen)"]
    D --> E["Vec<ParsedCall>"]
    E --> F["ingest::load()"]
    F --> G["DashboardData"]
    D -. "dedup_key" .-> H["shared seen set"]
```

The same `seen: &mut HashSet<String>` is shared across every tool adapter during one run, so re-reading the same local record only contributes once.

## Internal Adapter Contract

All tool adapters implement the same trait in `src/providers/mod.rs`:

```rust
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn discover(&self) -> Result<Vec<SessionSource>>;
    fn parse(
        &self,
        source: &SessionSource,
        seen: &mut HashSet<String>,
    ) -> Result<Vec<ParsedCall>>;

    fn model_display(&self, model: &str) -> String { /* default */ }
    fn tool_display(&self, tool: &str) -> String { /* default */ }
}
```

`ParsedCall` from `src/providers/types.rs` is the normalized record every adapter emits and every dashboard aggregator consumes. See [architecture.md](../architecture.md) for field meanings and aggregation behavior.

## Pricing

`src/pricing/snapshot.json` is an embedded LiteLLM-derived price table. The default build never fetches it at runtime.

```text
cost = multiplier * (
    input_tokens * input_rate
  + output_tokens * output_rate
  + cache_creation_input_tokens * cache_write_rate
  + cache_read_input_tokens * cache_read_rate
  + web_search_requests * web_search_rate
)
```

Model lookup canonicalizes model names, resolves aliases such as `cursor-auto`, `anthropic-auto`, and `openai-auto`, then falls back to a default Sonnet row if no match exists. Claude Opus fast mode applies the row's `fast_multiplier`.

Refresh the embedded snapshot with:

```bash
cargo run --features refresh-prices -- --refresh-prices
```

## Adding a New Tool

1. Create `src/providers/<name>/{mod.rs, config.rs, discovery.rs, parser.rs}`.
2. Put every path, env var, glob, SQL query, and source constant in `config.rs`.
3. Implement `Provider` in `mod.rs` and register it in `providers::registry()`.
4. Add a variant to `app::Tool`, update its label and cycle order, and update `ingest::matches_tool`.
5. Add display names in aggregation helpers such as `provider_short_label` when needed.
6. Write `docs/tools/<name>.md` and add it to the supported tools table above.
7. Add parser tests for source validation, token mapping, deduplication, project detection, and tool/bash extraction.

## Verification

- `cargo test` runs parser unit tests, pricing lookup tests, aggregation tests, and render smoke tests.
- `cargo run` launches the TUI and falls back to sample data when no local calls are ingested.
- `cargo run -- --list-projects` prints normalized project/tool inventory rows for debugging source attribution.
