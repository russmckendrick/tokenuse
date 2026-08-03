# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Added

No additions recorded yet.

## Changed

- Repaired the pricing refresh, which had been failing since Cursor moved its first-party rates (Auto Cost, Composer, Grok) out of the `Model pricing` table into a client-side widget that the Markdown export does not render. Both Cursor sources now use a new `pinned` extract mode that supplies those rates from `costs/pricing-sources.json` and checks the page for a rename or retirement on every run. No Cursor price changed — the rates were already correct.
- Picked up OpenAI price cuts that the failing refresh had been blocking: GPT-5.6 Luna drops to $0.20/MTok input and $1.20/MTok output, and GPT-5.6 Terra to $2.00/MTok input and $12.00/MTok output, for both the global and Copilot-scoped rows. Archived calls keep their import-time cost as usual.

## Removed

No removals recorded yet.
