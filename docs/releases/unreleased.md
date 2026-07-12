# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Changed

- Model names are now normalised through a single shared registry (`src/models/registry.json`) instead of per-tool tables, and every model carries a provider (Anthropic, OpenAI, Google, GitHub, Cursor, xAI) and family. Dated ids and vendor paths fold into one row, Claude Code now shows Fable 5 / Opus 4.8 style names, Cursor's raw `auto`/`default` render as "Cursor (auto)" / "Cursor (default)", and Copilot's router models are attributed to their underlying provider ("OpenAI (auto)", "Anthropic (auto)"). Unknown ids are prettified instead of rendering raw.

## Removed

- Removed the Insights page and its advice engine (the feature that shelled out to local `codex`/`claude`/`gemini` CLIs) plus the Agent Setup audit page from both the TUI and the desktop app. Token Use now focuses on token usage and cost tracking only.
- The TUI tab strip is now Overview, Deep Dive, and Usage; the freed `i` and `a` keys are intentionally unbound. Config and Session remain reachable as before.
- The local archive schema moved to v3: the advice tables are dropped on first open while all calls and limits are preserved. Older Token Use binaries refuse to open a v3 archive with a "newer than this binary supports" error, so downgrade requires deleting `archive.db` (it rebuilds from local tool history).
- The bundled advice prompt files are no longer shipped or written to `<config dir>/tokenuse/advice-prompts/`; the `agent-audit.json` snapshot is no longer written. Existing files on disk are harmless leftovers and can be deleted.
