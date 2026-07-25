# Pricing And Cache Rates

Last checked: July 11, 2026.

`tokenuse` calculates cost from local usage files. It does not call provider billing APIs during ingestion. Pricing is loaded from two books:

- `costs/pricing-upstream.json`: broad model coverage generated from LiteLLM and other machine-readable feeds.
- `costs/pricing-overrides.json`: official-source corrections, aliases, fallback rows, tool-scoped rows, provenance, and effective dates.

`costs/pricing-sources.json` owns the live source configuration: URLs, source kind, table headings, columns, row matches, scope, defaults, and published local-download URLs. Rust implements generic JSON-map, Markdown-table, and HTML-table/text extraction; provider-specific selectors stay in JSON.

## Source Policy

LiteLLM is broad coverage, not final authority. Official provider/tool docs override LiteLLM when they publish cache rates, special modes, tool pricing, aliases, or effective dates:

| Area | Configured source |
| --- | --- |
| Claude model pricing | [Claude pricing markdown](https://platform.claude.com/docs/en/about-claude/pricing.md) |
| Claude Code fast mode | [Claude fast mode markdown](https://code.claude.com/docs/en/fast-mode.md) |
| OpenAI/Codex API pricing | [OpenAI API pricing](https://openai.com/api/pricing/) |
| Gemini API pricing | [Gemini API pricing](https://ai.google.dev/gemini-api/docs/pricing) |
| Cursor Auto pricing | [Cursor models and pricing markdown](https://cursor.com/docs/models-and-pricing.md) |
| GitHub Copilot pricing | [GitHub Copilot models and pricing markdown](https://docs.github.com/en/copilot/reference/copilot-billing/models-and-pricing.md) |

Generated books carry top-level `checked_at` metadata, and every override row carries `source_name`, `source_url`, `checked_at`, and optional `note`. Rows can also carry `effective_from`; future-effective rows are ignored until the call timestamp reaches that date. Calls without a timestamp use import time.

## GitHub Copilot

Copilot moved every plan to usage-based billing ("AI Credits", 1 credit = $0.01) on June 1, 2026, and bills token consumption at the listed per-model API rates. Copilot rows therefore live under the `copilot` tool scope with `effective_from: "2026-06-01"`.

That scope matters: `GPT-5.6 Terra`, `Claude Opus 5`, `Gemini 3.5 Flash`, and similar display names are mapped for Copilot only. They do not override Codex, Claude Code, Gemini, or Cursor calls with similar model names.

The upstream page now uses per-provider tables: the Anthropic table carries a `Cache write` column, and the OpenAI/Google tables add `Tier`/`Threshold` columns with separate Default and Long-context rows per model. Row matching is first-match, which selects the Default tier — long-context surcharges are not modelled. Rows retired since the June switch (`GPT-4.1`, `GPT-5.2`, `GPT-5.2-Codex`, `Grok Code Fast 1`, `Goldeneye`) keep their last-known override rows via the drift handling below.

GitHub includes cached-token rates. `tokenuse` stores those rates in the Copilot rows, but parsed Copilot transcripts currently do not expose reliable cache buckets, so cached-token billing is only applied when the parser has trustworthy cache counts.

## Cache Rates

The UI's `Cache` column remains observed cache-hit behavior from local usage data. `Cache Rate` is the pricing multiplier for cached input relative to normal input.

Current important rows:

- Claude prompt-cache reads are 10% of input; 5-minute cache writes are 125% of input. 1-hour cache writes are 200% of input — the books carry the 5-minute rate and the pricing formula applies a fixed 1.6x premium to the 1h share reported under `usage.cache_creation` (see [Architecture — Pricing](architecture.md#pricing)).
- Cursor Auto reads are 20% of input, and cache writes use the same rate as input. Cursor folded its standalone "Auto pricing" table into the combined "Model pricing" table, so `cursor-auto` is now read from that table's `Auto Cost` row rather than from a per-token-type table.
- Current OpenAI GPT-5.4/GPT-5.5/GPT-5.6 and Codex rows use 10% cached-input pricing; `codex-mini-latest` remains 25%.
- Gemini rows are explicit overrides with source provenance because Gemini publishes prompt-length tiers that the parser cannot yet choose per call.
- Claude Sonnet 5 carries introductory pricing in the upstream book; a dated override switches it to standard pricing from September 1, 2026 (`effective_from: "2026-09-01"`).
- Claude Code fast mode is modelled as `fast_multiplier` on the base row: 2x for Opus 5 and Opus 4.8, 6x for Opus 4.7 and the historical Opus 4.6 rows. Opus 4.7 fast mode was deprecated on June 25, 2026 and removed on July 24, 2026; its row keeps the 6x multiplier so archived fast-mode calls from before removal still price correctly.

## Fallback Visibility

A `(tool, model)` pair that matches no tool-scoped or global row (including aliases and prefix rows) is billed at the book's fallback model. That silent fallback is now surfaced: `pricing::uses_fallback` reports it, the dashboard collects the distinct affected pairs into `DashboardData.fallback_priced_models` (all-time scope on the Config pages so the warning cannot hide behind a period filter), and report metadata carries the same list. Synthetic placeholder models (`<synthetic>`) are excluded — they carry no tokens.

## Maintainer Refresh

Refresh the checked-in books with:

```bash
cargo run -- --refresh-prices
```

The command writes both:

- `costs/pricing-upstream.json`
- `costs/pricing-overrides.json`

Do not hand-edit `pricing-upstream.json`. Curated aliases, fallbacks, and rows that cannot yet be reliably extracted live in `pricing-overrides.json` and `pricing-sources.json`.

GitHub Actions also runs `.github/workflows/refresh-pricing.yml` weekly and on manual dispatch. The workflow follows the currency-rate pattern: generate the books, run pricing tests, and commit `pricing-upstream.json` plus `pricing-overrides.json` only when those generated files differ. It installs system build dependencies through the shared `.github/actions/linux-build-deps` composite action (`core` profile), so the `libfontconfig1-dev`/`libdbus-1-dev` package list stays in sync with the rest of CI.

### Upstream row changes

`model-rows` sources tolerate two kinds of upstream drift without failing the whole refresh:

- **Deprecation annotations.** When a source keeps a priced row but relabels it (e.g. `Claude Sonnet 4 ([deprecated](...))`), the matcher strips a trailing parenthetical status annotation and still matches the bare model name.
- **Retired rows.** When a configured model disappears from a source entirely (e.g. a Copilot model that was pulled), the refresh prints a `warning: ... skipping` line and keeps the model's last-known override row instead of erroring.

As a safety net, a source that matches **none** of its configured rows still fails loudly — that pattern signals a table heading/column change rather than a single model being retired.

## Local Downloads

The TUI and desktop Config pages can download published pricing books after confirmation. Local files are written as:

- `<config dir>/tokenuse/pricing-upstream.json`
- `<config dir>/tokenuse/pricing-overrides.json`

The app reloads pricing in-process after a successful download. Refreshed pricing applies to newly imported calls; existing archive rows keep their import-time `cost_usd`.

## Parser Caveats

- Claude Code exposes input, output, cache-write, and cache-read buckets directly.
- Codex/OpenAI reports cached input inside total input. The parser subtracts cached input before pricing and prices the cached portion as cache read.
- Gemini reports cached tokens inside input, so the Gemini parser follows the same subtract-then-price pattern.
- Cursor local files do not provide a reliable cache breakdown. `cursor-auto` pricing is correct, but observed cache-read tokens remain 0 until Cursor exposes them in local source data.
- Copilot local transcripts are still estimated and do not expose reliable cache buckets. GitHub's June 1, 2026 billing includes cached tokens, but tokenuse cannot reconstruct that billing exactly from local files today.
