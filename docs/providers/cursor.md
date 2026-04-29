# Cursor

Cursor stores its conversation history in a single SQLite database per install. `tokenuse` reads it via `rusqlite` (bundled — no system SQLite required).

> Status: discovery + config implemented (`src/providers/cursor/`). Parser is a scaffold; this doc is the implementation plan.

## Where the data lives

| Platform | Path |
| --- | --- |
| macOS | `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb` |
| Linux | `~/.config/Cursor/User/globalStorage/state.vscdb` |
| Windows | `%APPDATA%/Cursor/User/globalStorage/state.vscdb` |

`tokenuse` caches parse results at `~/.cache/tokenuse/cursor-results.json` keyed on the database's mtime. Re-parse only when `state.vscdb` is newer than the cache.

## Record format

Cursor uses two storage layouts in the `cursorDiskKV` table. Both are JSON blobs stored under string keys.

### V2 bubbles

Query: `SELECT key, value FROM cursorDiskKV WHERE key LIKE 'bubbleId:%'`

Each row's `value` is JSON of the form:

```jsonc
{
  "type": 0,                              // 1 = user, 0 = assistant
  "createdAt": 1731539400000,             // ms since epoch
  "tokenCount": {
    "inputTokens": 412,
    "outputTokens": 188
  },
  "modelInfo": { "modelName": "claude-sonnet-4-5" },
  "codeBlocks": [{ "language": "rust" }, { "language": "ts" }],
  "text": "...assistant or user message..."
}
```

### Agent KV (newer Cursor Agent)

Query: `SELECT key, value FROM cursorDiskKV WHERE key LIKE 'agentKv:blob:%'`

These rows carry a series of `{role, content}` pairs without explicit token counts. Estimate tokens with `chars / 4.0` and treat the row's `requestId` as the dedup key (`cursor:agentKv:<requestId>`).

## Token & cost mapping

| `ParsedCall` field | Source |
| --- | --- |
| `input_tokens` | `tokenCount.inputTokens` (or `chars / 4` for AgentKv) |
| `output_tokens` | `tokenCount.outputTokens` (or `chars / 4` for AgentKv) |
| `cache_*` | `0` — Cursor does not surface cache breakdown |
| `model` | `modelInfo.modelName` after alias resolution |
| `timestamp` | `DateTime::from_timestamp_millis(createdAt)` |
| `tools` | `codeBlocks[].language` (treated as informational, not tool calls) |

**Token quirk:** Cursor v3 sometimes records zero tokens. When `tokenCount.inputTokens + tokenCount.outputTokens == 0`, fall back to character-count estimation.

**Model resolution:**
- `"default"` → `claude-sonnet-4-5` (alias in pricing snapshot)
- `"cursor-auto"` → `claude-sonnet-4-5` (alias in pricing snapshot)
- Unknown model name → fallback to Sonnet rate (`pricing::PriceTable::lookup` handles this)

## Deduplication

- V2 bubbles: `cursor:bubble:<key>` (the full `bubbleId:...` row key).
- AgentKv: `cursor:agentKv:<requestId>`.

A single Cursor row is one user message *or* one assistant message; only assistant rows produce `ParsedCall`s.

## Tools / bash extraction

Cursor does not expose tool-call names in a structured form on these tables. We do **not** populate `tools` or `bash_commands` from Cursor.

## Known limitations

- All Cursor activity rolls up under a synthetic `cursor-workspace` project — Cursor stores the cwd separately per-bubble and the linkage isn't reliable enough to set per-call.
- AgentKv chars/4 estimation undercounts code blocks (which compress more in tokenization). Treat the cost as approximate.
- The DB is locked while Cursor is running. Open with `SQLITE_OPEN_READ_ONLY` and add `?immutable=1` to the URI to avoid blocking.
