# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

- New **Insights** page (TUI and desktop) surfaces local heuristic recommendations across model right-sizing, cache efficiency, anomalies, and quota pacing. Each card carries a severity, scope, and estimated weekly savings (where applicable) with the assumption stated inline. The engine is fully local — no network, no telemetry. Cursor and Copilot cache rules silence themselves explicitly because their local logs don't expose cache buckets. Press `i` from any page to jump to it. See `docs/guides/insights.md` for rule details.
- Usage now imports Claude Code limit snapshots from the local status-line sidecar at `limits/claude-code.json` and Copilot quota snapshots from `limits/copilot.json`.
- Config adds Claude and Copilot limit sync actions. Copilot sync is explicit, confirmed, feature-gated, and writes a local quota sidecar before refreshing archive limits.
- Claude limit sync now prompts users to set up the Claude Code `statusLine` sidecar first instead of reporting a vague missing sidecar error.
- Config adds a **Claude statusLine** row that installs (or removes) a wrapper script for `~/.claude/settings.json`. The wrapper writes Claude Code's status payload to the sidecar and forwards stdin through whatever command was previously configured (e.g. `cship`), so the visible status line is unchanged. Settings are backed up to `settings.json.bak.<unix-ts>` before editing; a *Generate wrapper only* path is available for users who'd rather edit settings themselves.
- Usage limit rows now attach to the correct Claude Code and Copilot console sections instead of relying on Codex's matching short label.
