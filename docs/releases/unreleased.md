# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Added

- Pricing and model names for the September 22, 2026 launches: Claude Opus 5.5 ($4/$20 per MTok, 0.05x cache reads, 2x fast mode) across Claude Code, Cursor, and Copilot, and GPT-6 Sol ($2/$10) and GPT-6 Luna ($0.10/$0.50) for Codex and Copilot. Opus 5.5 now shows as its own model instead of being grouped under Opus 5.

## Changed

- Archive schema v10 repairs costs that were already stored for the new models. Opus 5.5 calls charged at the inherited Opus 5 rates, and GPT-6 Sol, Luna, and Astra calls charged at the Sonnet fallback, are recomputed only when the stored cost proves the stale formula was used.
- Pricing books refreshed on September 23, 2026. `gpt-5-codex`, `gpt-5.1-codex` (including Max and Mini), `gpt-5.2-codex`, and `codex-mini-latest` now come from OpenAI's model pages. LiteLLM dropped their direct keys, which would otherwise have left a 10% regional Azure uplift, or fallback pricing for `codex-mini-latest`.
- Pricing refreshes now keep expired historical pinned rates without requiring their old marker text to remain on a provider's live page. Current pins still fail loudly when an entire configured source disappears.

## Removed

No removals recorded yet.
