# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.
- Added an Agent Setup page to the TUI and desktop app. It refreshes a local-only, redacted audit snapshot of agent home folders, archive usage, project coverage, tool knowledge files, MCP declarations, and recent context hygiene into `agent-audit.json`.
- Fixed Codex usage parsing to derive call usage from cumulative token deltas when available, avoiding over-counting duplicate `token_count` snapshots with unchanged totals while keeping tool attribution isolated to emitted calls.
- Fixed Codex model labels so GPT-family models keep their full identity in model breakdowns (for example `GPT-5.6 Sol`, `GPT-5.3 Codex Spark`, `GPT-4o Mini`) instead of collapsing into generic `GPT-5`.
