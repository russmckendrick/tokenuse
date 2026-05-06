# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Highlights

- Added cache-read price-rate visibility to model tables and session call details, keeping observed cache-hit percentages separate from pricing multipliers.
- Updated embedded pricing generation for current Claude, OpenAI/Codex, Cursor Auto, and Gemini cache-read rates, including Cursor Auto's direct 20% cache-read row.
- Split pricing into upstream and override books, moved official-source pricing rules into JSON config, and added GitHub Copilot usage-based pricing gated to June 1, 2026.
- Added a scheduled/manual GitHub Actions workflow to refresh and publish generated pricing books.
- Changed the generated currency snapshot workflow from nightly to weekly.
- Added published rates and pricing book links to the TUI and desktop Config pages.
- Added the active pricing book date to the TUI and desktop Config pages.
- Moved published cost JSON into `costs/` and renamed the currency snapshot download to `exchange-rates.json`.

## Notes

- Added maintainer pricing documentation with provider source quotes and parser caveats for Claude Code, Codex, Cursor, GitHub Copilot, and Gemini.
