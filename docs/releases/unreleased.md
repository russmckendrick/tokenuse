# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Highlights

- Added Gemini CLI ingestion from local `~/.gemini/tmp/<project_hash>/chats/session-*.json|jsonl` files, including exact token usage, cached input reads, thoughts, tool calls, Bash command extraction, and Gemini filters across TUI, desktop, and exports.
- Added Cursor Agent transcript ingestion from `~/.cursor/projects/**/agent-transcripts`, including Composer JSONL, legacy text transcripts, subagent transcripts, approximate token estimates, tool extraction, Bash command extraction, and `CURSOR_AGENT_HOME` for copied Cursor data folders.
- Added a confirmed clear-data action to both Config pages that deletes `archive.db` and immediately reimports local tool history while keeping config, rates, pricing snapshots, and exports.
- Improved Cursor project attribution by using transcript path hints and `ai_code_hashes.fileName` metadata to reassign older `cursor-workspace` rows when a better project path is available.
- Fixed Cursor Agent project labels on Windows so restored home-directory paths use stable `/` separators instead of mixed path separators.
- Improved adapter coverage for Claude alternate config roots, Codex cumulative token-only records and model hints, Gemini `tool`/`total` token buckets plus `GEMINI_DIR`, and Copilot VS Code Insiders/server transcripts with better workspace labels and reasoning-token estimates.

## Notes
