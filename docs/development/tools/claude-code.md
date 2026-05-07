# Claude Code

Claude Code records every assistant message — including token usage and tool calls — to JSONL files on disk. `tokenuse` reads these directly.

> Status: implemented (`src/tools/claude_code/`).

## Where the data lives

| Platform | Path |
| --- | --- |
| All (CLI, projects) | `~/.claude/projects/<sanitized-cwd>/*.jsonl` |
| macOS (Desktop, agent mode) | `~/Library/Application Support/Claude/local-agent-mode-sessions/**/projects/<dir>/*.jsonl` |
| Linux (Desktop, agent mode) | `~/.config/Claude/local-agent-mode-sessions/**/projects/<dir>/*.jsonl` |
| Windows (Desktop, agent mode) | `%APPDATA%/Claude/local-agent-mode-sessions/**/projects/<dir>/*.jsonl` |

Subagent transcripts live in a `subagents/` subdirectory under each project and are read in addition to the main `*.jsonl` files.

**Env var override:** `CLAUDE_CONFIG_DIR` replaces the default CLI roots for the projects path. It can contain one path or a comma-separated list of config roots, each expected to contain a `projects/` directory. Without an override, `tokenuse` checks both `$XDG_CONFIG_HOME/claude/projects` (or `~/.config/claude/projects`) and `~/.claude/projects`.

Claude entries include a top-level `cwd` field, and that is the authoritative project path for parsed calls. The project directory name is only a lossy fallback: names like `-Users-me-Code-ai-commit-dev` cannot distinguish path separators from real hyphens, so never treat the directory-derived value as canonical when `cwd` is present.

**Discovery rules** (`src/tools/claude_code/discovery.rs`):
- Enumerate immediate subdirectories of every configured CLI `projects/` root.
- Walk the Desktop sessions tree to depth 8 looking for any directory named `projects`; treat each child as a session source.
- Skip `node_modules` and `.git` while walking.

```mermaid
flowchart TD
    A[claude projects dir] --> C[project directory]
    B[desktop local agent sessions] --> D[projects directory]
    C --> E[main jsonl files]
    C --> F[subagents jsonl files]
    D --> E
    D --> F
    E --> G[parse JSONL entries]
    F --> G
    G --> H[user updates last user text]
    G --> I[assistant with usage emits ParsedCall]
    J[tokenuse limits sidecar] --> K[statusLine rate_limits]
    K --> L[emit LimitSnapshot]
```

## Record format

Each `*.jsonl` is one JSON object per line. Two entry types matter:

```jsonc
// User turn
{
  "type": "user",
  "timestamp": "2026-04-26T10:00:00Z",
  "sessionId": "session-uuid",
  "message": {
    "role": "user",
    "content": "refactor the parser"            // string OR array of {type:"text", text:"..."}
  }
}

// Assistant turn — the only entry type that produces a ParsedCall
{
  "type": "assistant",
  "timestamp": "2026-04-26T10:00:01Z",
  "sessionId": "session-uuid",
  "message": {
    "role": "assistant",
    "id": "msg_01ABC...",                       // dedup key
    "model": "claude-opus-4-7-20250514",
    "usage": {
      "input_tokens": 100,
      "output_tokens": 50,
      "cache_creation_input_tokens": 1000,
      "cache_read_input_tokens": 5000,
      "speed": "fast",                          // optional, "standard" | "fast"
      "server_tool_use": {
        "web_search_requests": 0
      }
    },
    "content": [
      { "type": "tool_use", "name": "Bash", "input": { "command": "ls -la | grep foo" } },
      { "type": "tool_use", "name": "Edit",  "input": { /* ... */ } },
      { "type": "text", "text": "Done." }
    ]
  }
}
```

## Token & cost mapping

| `ParsedCall` field | Source |
| --- | --- |
| `input_tokens` | `message.usage.input_tokens` |
| `output_tokens` | `message.usage.output_tokens` |
| `cache_creation_input_tokens` | `message.usage.cache_creation_input_tokens` |
| `cache_read_input_tokens` | `message.usage.cache_read_input_tokens` |
| `cached_input_tokens` | `0` — Anthropic reports cache reads separately (not included in input) |
| `reasoning_tokens` | `0` — Claude does not report a separate reasoning bucket |
| `web_search_requests` | `message.usage.server_tool_use.web_search_requests` |
| `speed` | `message.usage.speed` (default `Standard`) |
| `model` | `message.model` (preserved verbatim; pricing canonicalizes) |
| `cost_usd` | `pricing::cost(model, &call, speed)` |

Anthropic-specific quirk: cache reads are billed at 10% of the input rate in current bundled rows, cache writes use the 5-minute 125% rate, and `cache_read_input_tokens` is **not** included in `input_tokens`. The pricing formula handles this directly — do **not** sum the buckets together before pricing. See [Pricing and cache rates](../pricing.md) for source evidence.

## Deduplication

`dedup_key = message.id` if present, otherwise `claude:<timestamp>`.

Re-reading the same JSONL across runs is normal; the shared `seen: &mut HashSet<String>` ensures every assistant message contributes once per process.

## Tools / bash extraction

Walk `message.content[]` and collect `name` from every `{ "type": "tool_use" }` block.
- `mcp__server__tool` names are kept in `tools` and surface separately in the dashboard's MCP servers panel (split on `__`).
- For `Bash` and `BashOutput` tool calls, parse `input.command` and split on unquoted `;`, `|`, `&&`, `||`. Each split is a separate command (`tools::jsonl::split_bash_commands`). The dashboard then groups by first word (`first_word`).

```mermaid
flowchart LR
    A[assistant content array] --> B{tool_use block}
    B -->|name only| C[tools]
    B -->|Bash or BashOutput| D[input.command]
    D --> E[split_bash_commands]
    E --> F[bash_commands]
```

## Known limitations

- The user message captured per call is the most recent user turn before the assistant response, truncated to 500 chars. If a user sends multiple messages in rapid succession before any assistant reply, only the last is recorded.
- Synthetic models (`<synthetic>`, used by Claude Code for placeholder rows) hit the pricing fallback — they cost `$0` because their token counts are zero, but they still count toward call totals.
- No live file watching: press `r` or wait for the 15-minute background archive sync to pick up new sessions.

## Rate-limit snapshots

Claude Code does not write plan-window rate-limit usage into the transcript JSONL. It does expose the data to status-line commands for Claude.ai Pro/Max subscribers after the first API response in a session. `tokenuse` imports that data from:

```text
<config dir>/tokenuse/limits/claude-code.json
```

The default sidecar path is platform-specific:

| Platform | Sidecar path |
| --- | --- |
| macOS | `~/Library/Application Support/tokenuse/limits/claude-code.json` |
| Linux | `${XDG_CONFIG_HOME:-~/.config}/tokenuse/limits/claude-code.json` |
| Windows | `%APPDATA%\tokenuse\limits\claude-code.json` |

The sidecar should contain the JSON object Claude Code passes to the configured `statusLine` command. `tokenuse` reads `rate_limits.five_hour.used_percentage`, `rate_limits.five_hour.resets_at`, `rate_limits.seven_day.used_percentage`, and `rate_limits.seven_day.resets_at`, then emits one `LimitSnapshot` with 5h and weekly windows. `resets_at` is a Unix epoch timestamp in seconds.

### Recommended: install the wrapper from the desktop app

Open the **Config** page and click **Install** on the *Claude statusLine* row. Token Use will:

1. Detect whatever is already in `~/.claude/settings.json` (e.g. `cship`).
2. Write a wrapper script under `<config>/tokenuse/statusline/claude-code.sh` (or `.ps1` on Windows) that tees the JSON Claude Code passes on stdin into the sidecar path *and* pipes the same JSON through the previously detected command — so the visible status line is unchanged.
3. Back up `~/.claude/settings.json` to `settings.json.bak.<unix-ts>` and rewrite `statusLine.command` to point at the wrapper.

If you prefer not to let the app touch `settings.json`, choose **Generate wrapper only** in the second dialog. The app will write the wrapper script and tell you the exact path to paste into your settings yourself. **Uninstall** restores the previous `statusLine.command` and removes the wrapper; the sidecar JSON file is left in place.

### Manual setup

A minimal macOS/Linux wrapper can write the sidecar while preserving status output:

```bash
#!/bin/bash
input=$(cat)
if [ "$(uname)" = "Darwin" ]; then
  dir="$HOME/Library/Application Support/tokenuse/limits"
else
  dir="${XDG_CONFIG_HOME:-$HOME/.config}/tokenuse/limits"
fi
mkdir -p "$dir"
printf '%s\n' "$input" > "$dir/claude-code.json"
echo "Claude"
```

A minimal Windows PowerShell wrapper writes the same file under `%APPDATA%`:

```powershell
$inputJson = [Console]::In.ReadToEnd()
$dir = Join-Path $env:APPDATA "tokenuse\limits"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Set-Content -Path (Join-Path $dir "claude-code.json") -Value $inputJson -NoNewline -Encoding UTF8
"Claude"
```

After configuring Claude Code to use the wrapper as its `statusLine` command, run at least one Claude Code request. Claude only includes `rate_limits` after an API response, so the Config page will continue to show the setup hint until that sidecar exists.
