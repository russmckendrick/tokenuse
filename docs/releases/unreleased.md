# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Highlights

- Added Gemini CLI ingestion from local `~/.gemini/tmp/<project_hash>/chats/session-*.json|jsonl` files, including exact token usage, cached input reads, thoughts, tool calls, Bash command extraction, and Gemini filters across TUI, desktop, and exports.
- Added Cursor Agent transcript ingestion from `~/.cursor/projects/**/agent-transcripts`, including Composer JSONL, legacy text transcripts, subagent transcripts, approximate token estimates, tool extraction, Bash command extraction, and `CURSOR_AGENT_HOME` for copied Cursor data folders.
- Improved Cursor project attribution by using transcript path hints and `ai_code_hashes.fileName` metadata to reassign older `cursor-workspace` rows when a better project path is available.

## Notes
