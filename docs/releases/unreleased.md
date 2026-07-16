# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Added

- **Coach page (desktop).** A new sidebar report-card workspace (shortcut `h`) with dedicated report-style tabs for Report, Findings, AI Output, and Activity. Report owns the composite grade and four practice-score cards, brings priority advice forward with direct links to each rule's evidence detail, and pairs equal-height KPI/sparkline Flow and Pace panels, including a visible single-day Flow state. Findings is a filterable master/detail evidence explorer. AI Output leads with six outcome KPIs and a full-width period-aware bar + moving-average chart before the language/model/project breakdown, using 30-minute detail for 24 Hours, hourly detail for 7 Days, full daily timelines for 30 Days and This Month, and monthly history for All Time. Activity splits into Work Hours, Calendar, and Projects: hourly pattern charts and period trend; a trailing-year calendar with session lanes and a fixed call inspector; and ranked project activity with spend, calls, sessions, AI LoC, and tool mix. Detection is a Rust-native port of 27 rules from Microsoft's MIT-licensed AI-Engineering-Coach (see `NOTICE` and `docs/development/coach.md`); everything is computed locally with no network or LLM calls. Sample mode (Shift+D) includes a demo coach payload.
- **Desktop page toolbar.** Tool, project, sort, and other page-scoped controls now sit in the primary header immediately before refresh and report instead of occupying a separate filter row. Compact windows switch the filter labels to icons while keeping the active values visible.
- **Archive v4 enrichment columns.** The `calls` table gains `is_canceled`, `prompt_chars`, `response_chars`, `elapsed_ms`, `code_blocks_json`, `edited_files_json`, and `referenced_files_json`. The migration clears `source_state` once so history still on disk re-parses and backfills the new columns (fills once, never clobbers); rows whose source files are gone stay `NULL` and are excluded from rule denominators.
- **Parser extraction for coach signals.** Claude Code: full prompt length, interrupts (`[Request interrupted by user`), turn latency, response length, code blocks from fences and Write/Edit/MultiEdit/NotebookEdit payloads, edited/referenced files. Codex: user/agent messages, `turn_aborted`, and `patch_apply_end` changes (edited files + added-line LoC). Copilot and Gemini get prompt/response lengths and fenced code where their logs carry text; Cursor rows stay estimate-only by design. Per-tool signal matrices are documented in `docs/development/tools/<name>.md`.

## Changed

- Codex session details now capture user prompts (`user_message` was previously empty for Codex rows).
- Claude Code user-line handling no longer records slash-command wrappers (`<command-...>`) or interrupt markers as the turn's prompt.
- Ingest cache format bumped to v2 for the new per-call fields; older cache files are discarded and regenerated automatically.
- Claude Code, Codex, Copilot (transcripts), and Gemini source fingerprints gained version prefixes, forcing a one-time re-parse through the enriched parsers.

## Removed

No removals recorded yet.
