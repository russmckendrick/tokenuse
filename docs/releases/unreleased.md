# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Added

- The desktop Coach page gains an advisory Setup panel: unused configured MCP servers, CLAUDE.md files over 200 lines after `@import` expansion, repeated file re-reads inside sessions, and reads under build/dependency directories — each with a heuristic token-savings estimate. Setup findings never affect the practice grade.
- Spend is now broken down by activity: every call is classified into one of thirteen deterministic task categories (coding, debugging, feature dev, refactoring, testing, exploration, planning, delegation, git ops, build & deploy, brainstorming, conversation, general) from its tool usage and prompt — no LLM involved. A "By Activity" panel appears on the desktop Analytics page and in `tokenuse overview`.
- Models silently billed at the fallback pricing rate are now surfaced instead of blending in: the Config page (TUI and desktop) warns with the affected `tool · model` pairs and the fix hint, and report metadata (JSON, Excel, CSV) carries a `fallback_priced_models` row.
- `tokenuse status` and `tokenuse overview`: scriptable summaries without the TUI. `status` is a one-liner with rolling-24-hour and month totals; `overview` is a copy-pasteable month summary (totals, tokens, per-tool spend, top models and projects, daily table). Both respect the configured currency and take `--json` for machine output.
- `tokenuse doctor`: read-only per-tool diagnostics. Shows every location each adapter probes (and whether it exists), the environment overrides in effect, discovered session/limit source counts, and a bounded parse sample, ending in an `OK` / `NOTHING FOUND` / `ERRORS` / `DISCOVERY FAILED` verdict with the likely cause. `--json` emits the same report machine-readable.
- Codex sessions archived from the Codex UI (`~/.codex/archived_sessions/`) are now discovered and ingested, so archiving a session no longer removes its history from the dashboard.
- Copilot in VS Code now reads the Copilot Chat extension's OpenTelemetry span store (`agent-traces.db`) — the one VS Code source with real input, output, and cache token counts — plus VS Code core chat-session journals (`chatSessions/`, real prompt/output counts) and the global empty-window journals. Per VS Code variant the most authoritative source wins, so estimates never double count real data. VSCodium storage paths are now covered alongside VS Code and VS Code Insiders.
- Legacy Copilot CLI sessions now recover real input and cache token counts from their `session.shutdown` per-model rollups; previously those sessions reported input as 0 and their cost was output-only.
- Cursor input tokens now come from Cursor's own per-conversation context meter (`promptTokenBreakdown`) when bubbles carry no explicit counts — credited once per conversation on a stable anchor instead of chars/4 guesses per turn. The meter is a latest-snapshot figure, so totals can undercount the Cursor admin console but never double count.
- Cursor conversations now resolve their project from VS Code workspace storage (the workspace folder each composer was opened in), so state-backed conversations group under their real repository instead of the generic workspace label.
- Cursor's Composer house models now all price at Cursor's published rates: `composer-1` from the live pricing table, and the retired `composer-1.5` / `composer-2` pinned in the sources manifest. Previously anything older than Composer 2.5 fell through to the global fallback model.
- Claude Code advisor runs are now counted: each `advisor_message` iteration inside a message's usage payload is a separately billed API call with its own model and token buckets, and now appears as its own call in totals, model breakdowns, and session views.
- Claude Code 1-hour cache writes now price at Anthropic's published premium (2x base input versus 1.25x for the 5-minute TTL). Sessions using long-TTL prompt caching were previously underpriced by 1.6x on their cache-write spend.
- Session-scoped Claude Code subagent transcripts (`<project>/<session-id>/subagents/agent-*.jsonl`, written by newer Claude Code builds) are now discovered and ingested.

## Changed

- Claude Code parsing now merges every streamed JSONL line of an assistant message into one call. Claude Code writes one line per content block (same message id and usage on each), and the previous first-line-wins deduplication silently dropped the tool calls, shell commands, edited/read files, and response text carried on later lines. Token counts and costs are unchanged; Core Tools, MCP Servers, shell command counts, and Coach signals become noticeably more complete after the automatic reparse.
- Nested Claude Code workflow subagent transcripts (`subagents/workflows/<workflow>/agent-*.jsonl`) are now ingested; previously only the top level of each `subagents/` directory was read, so workflow runs under-reported spend.
- Codex calls are deduplicated by session lineage (content-addressed cumulative totals) instead of file path. Rollouts forked in the Codex UI replay the parent session's history into the new file, which the old path-scoped keys double-counted; replays now collide with the parent's rows and usage written within the fork-creation window is dropped. Existing archives migrate automatically on the next sync: each reparsed call retires its legacy row and keeps that row's import-time cost, so history is not repriced.

## Removed

No removals recorded yet.
