# Codex

OpenAI Codex writes one JSONL "rollout" file per session under a year/month/day tree. Each rollout starts with a `session_meta` envelope and ends with a `token_count` event carrying the final usage.

> Status: discovery + config implemented (`src/providers/codex/`). Parser is a scaffold; this doc is the implementation plan.

## Where the data lives

| Path | Notes |
| --- | --- |
| `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` | One file per session |

**Env var override:** `CODEX_HOME` replaces `~/.codex`.

**Validation:** before parsing, read the first line of each file. Treat it as a Codex rollout only if it has `type: "session_meta"` and the originator field identifies Codex (some other tools have copied the format). This avoids ingesting unrelated JSONL.

**Discovery rules** (`src/providers/codex/discovery.rs`):
- Walk `sessions_root()` recursively (no max depth — date tree is shallow).
- Match files whose name starts with `rollout-` and ends with `.jsonl`.
- Use the `YYYY/MM/DD` portion of the relative path as the project label.

## Record format

A rollout is heterogeneous JSONL. The interesting entry types:

```jsonc
// Session envelope (must be first line)
{ "type": "session_meta", "session_id": "...", "cwd": "/Users/me/widgets",
  "info": { "originator": "codex", "model": "gpt-5", "model_name": "gpt-5" } }

// Mid-session model change
{ "type": "turn_context", "model": "o3" }

// Tool call
{ "type": "response_item", "name": "exec_command", "arguments": { "command": "cargo test" } }
{ "type": "response_item", "name": "read_file",   "arguments": { "path": "src/lib.rs" } }
{ "type": "response_item", "name": "apply_patch", "arguments": { /* ... */ } }

// Usage events — the source of token counts
{ "type": "event_msg", "msg": { "type": "token_count",
    "last_token_usage":      { "input_tokens": 12000, "cached_input_tokens": 8000,
                                "output_tokens": 800,  "reasoning_output_tokens": 200 },
    "total_token_usage":     { "input_tokens": 80000, "output_tokens": 4200 } } }
```

`response_item` names map to canonical tool names:

| Codex name | Normalized |
| --- | --- |
| `exec_command` | `Bash` |
| `read_file` | `Read` |
| `write_file`, `apply_diff`, `apply_patch` | `Edit` |
| `web_search` | `WebSearch` |

## Token & cost mapping

Two parsing modes — pick one consistently:

**Mode A (preferred): per-call from `last_token_usage`.**
Each `event_msg/token_count` is one `ParsedCall`. Tokens come straight from `last_token_usage`.

**Mode B: cumulative from `total_token_usage`.**
Diff successive `total_token_usage` snapshots to recover per-turn deltas. Used when `last_token_usage` is missing.

| `ParsedCall` field | Source (Mode A) |
| --- | --- |
| `input_tokens` | `last_token_usage.input_tokens` − `last_token_usage.cached_input_tokens` |
| `output_tokens` | `last_token_usage.output_tokens` |
| `cached_input_tokens` | `last_token_usage.cached_input_tokens` |
| `cache_read_input_tokens` | `last_token_usage.cached_input_tokens` (priced as cache read) |
| `reasoning_tokens` | `last_token_usage.reasoning_output_tokens` |
| `model` | most recent `turn_context.model`, falling back to `session_meta.info.model` then `info.model_name`, then `gpt-5` |

**Critical quirk:** OpenAI reports cached tokens **inside** `input_tokens`. Subtract `cached_input_tokens` from `input_tokens` before pricing or the cache read is double-billed.

## Deduplication

`dedup_key = format!("codex:{path}:{timestamp}:{cumulative_total_input + cumulative_total_output}")`

Including the cumulative total prevents collapsing two consecutive turns that happen to share a timestamp, while still catching re-reads of the same file.

## Tools / bash extraction

Walk `response_item` entries between `token_count` events. Aggregate the normalized names into `tools`. For `exec_command`, run `arguments.command` through `providers::jsonl::split_bash_commands` and collect each piece into `bash_commands`.

## Known limitations

- Files use UTC timestamps with millisecond precision — `chrono::DateTime::parse_from_rfc3339` is sufficient.
- `session_meta.cwd` is the only reliable project signal. If absent, fall back to the date label.
- Codex rolls models mid-session (`turn_context`); the parser must track the most-recently-set model so each turn is priced correctly.
- Reasoning tokens are billed at the output rate by OpenAI — pricing currently includes them in the `output_tokens` bucket via the `output_per_token` rate. If you want them accounted separately, add a `reasoning_per_token` field to the snapshot schema first.
