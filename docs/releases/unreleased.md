# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Added

- Claude subscription sync now shows model-scoped weekly caps as their own gauges, starting with the separate Fable weekly limit on Max and Ultra plans.
- Copilot Fusion turns that only record the router id now show as Copilot Fusion, Fusion Max or Fusion Fast instead of a raw model id.
- Coach findings can be traced back to the sessions behind them (issue #5). Each finding keeps up to 50 evidence samples with timestamps. On desktop they scroll in the Findings detail and each one opens its session. In the TUI, Enter on a finding opens its evidence and Enter again opens the session.
- Prompt evidence in Coach findings can be ignored, which drops that prompt from every prompt finding (desktop Ignore button, TUI `x`). Restore all with the desktop Restore all button or `Shift-X` in the TUI. Use it for templated prompts from scripts and tools.

## Changed

- Claude Ultra (formerly Max 20x) accounts without a 5-hour limit now show only the weekly gauges, and the weekly gauge falls back to the new `weekly_all` limit when the legacy `seven_day` field is missing. The demo data now shows an Ultra plan with weekly and Fable gauges.

## Fixed

- Fewer false positives on Repeated Prompts and other prompt findings (issue #6). Coach now skips scripted runs (`claude -p`, Agent SDK apps, `codex exec`) and messages the agent harness writes (system reminders, subagent boilerplate, task notifications, image placeholders, compaction summaries). Subagent calls no longer split the parent prompt into hundreds of identical turns. Claude Code and Codex sessions re-parse once on the next sync to pick up the headless tag.

## Removed

No removals recorded yet.
