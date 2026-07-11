# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

- Added an Agent Setup page to the TUI and desktop app. It refreshes a local-only, redacted audit snapshot of agent home folders, archive usage, project coverage, tool knowledge files, MCP declarations, and recent context hygiene into `agent-audit.json`.
- Refreshed the pricing books for the mid-2026 model wave: Claude Fable 5, Claude Mythos 5, Claude Opus 4.8 (including its 2x fast mode; Opus 4.7 fast is now priced at its 6x rate), and Claude Sonnet 5 with its introductory pricing stepping up automatically on September 1, 2026. Fable/Mythos and Grok models now flow in from the upstream feed, so Claude 5-family usage no longer falls back to Sonnet 4.6 pricing.
- Extended Copilot's tool-scoped pricing to GitHub's current AI-credit model lineup (GPT-5.6 Luna/Sol/Terra, Claude Opus 4.8, Claude Sonnet 5, Claude Fable 5, Gemini 3.5 Flash, MAI-Code-1-Flash, Kimi K2.7 Code). Rows GitHub retired keep their last-known prices for historical usage.
- Copilot limit gauges now understand GitHub's June 1, 2026 switch to AI-credit billing: post-switch quota snapshots are labelled "AI Credits" with the remaining balance in credit units, the newer `quota_reset_date_utc` field is parsed, and the pacing insight speaks in AI-credit terms.
- Copilot ingestion now reads the CLI's SQLite stores (`~/.copilot/session-store.db` turns and `~/.copilot/data.db` per-session token totals), so sessions from CLI builds that stopped writing `events.jsonl` around May 2026 show up again. The data.db totals use real token counts and refresh in place as live sessions grow.
- VS Code Copilot transcripts now fall back to the session's declared model when tool-call id inference finds no known prefix, instead of pricing those sessions as the generic `copilot-auto` bucket.
