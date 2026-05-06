# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Highlights

- Added cache-read price-rate visibility to model tables and session call details, keeping observed cache-hit percentages separate from pricing multipliers.
- Updated embedded pricing generation for current Claude, OpenAI/Codex, Cursor Auto, and Gemini cache-read rates, including Cursor Auto's direct 20% cache-read row.

## Notes

- Added maintainer pricing documentation with provider source quotes and parser caveats for Claude Code, Codex, Cursor, GitHub Copilot, and Gemini.
