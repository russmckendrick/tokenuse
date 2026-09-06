# Pricing And Cache Rates

Last checked: September 6, 2026.

`tokenuse` calculates cost from local usage files. It does not call provider billing APIs during ingestion. Pricing is loaded from two books:

- `costs/pricing-upstream.json`: broad model coverage generated from LiteLLM and other machine-readable feeds.
- `costs/pricing-overrides.json`: official-source corrections, aliases, fallback rows, tool-scoped rows, provenance, and effective dates.

`costs/pricing-sources.json` owns the live source configuration: URLs, aliases, source kind, table headings, columns, row matches, scope, defaults, and published local-download URLs. Rust implements generic JSON-map, Markdown-table, and HTML-table/text extraction, plus maintainer-pinned historical or otherwise non-extractable rows; provider-specific selectors stay in JSON.

## Source Policy

LiteLLM is broad coverage, not final authority. Official provider/tool docs override LiteLLM when they publish cache rates, special modes, tool pricing, aliases, or effective dates:

| Area | Configured source |
| --- | --- |
| Claude model pricing | [Claude pricing markdown](https://platform.claude.com/docs/en/about-claude/pricing.md) |
| Claude Code fast mode | [Claude fast mode markdown](https://code.claude.com/docs/en/fast-mode.md) |
| OpenAI/Codex API pricing | Official model Markdown for [GPT-5.4](https://developers.openai.com/api/docs/models/gpt-5.4), [GPT-6 Astra](https://developers.openai.com/api/docs/models/gpt-6-astra), [GPT-5.6 Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol), [Terra](https://developers.openai.com/api/docs/models/gpt-5.6-terra), and [Luna](https://developers.openai.com/api/docs/models/gpt-5.6-luna) |
| Gemini API pricing | [Gemini API pricing](https://ai.google.dev/gemini-api/docs/pricing), plus Google's [August 2024 Gemini 1.5 Flash price update](https://developers.googleblog.com/en/gemini-15-flash-updates-google-ai-studio-gemini-api/) for archived calls |
| Cursor Auto, first-party, and selected third-party model pricing | [Cursor models and pricing markdown](https://cursor.com/docs/models-and-pricing.md), plus the official GPT-5.4, GPT-5.5, and GPT-5.6 model pages for explicit Fast and long-context tiers |
| GitHub Copilot pricing | [GitHub Copilot models and pricing markdown](https://docs.github.com/en/copilot/reference/copilot-billing/models-and-pricing.md) |

Generated books carry top-level `checked_at` metadata, and every override row carries `source_name`, `source_url`, `checked_at`, and optional `note`. Rows can carry `effective_from` and an exclusive `effective_to`; that particular row is active only inside its window. These dates select price versions rather than enforce model availability, so an applicable global or family-prefix row can still resolve outside a tool-scoped window. Calls without a timestamp use import time.

## GitHub Copilot

Copilot moved every plan to usage-based billing ("AI Credits", 1 credit = $0.01) on June 1, 2026, and bills token consumption at the listed per-model API rates. Copilot rows therefore live under the `copilot` tool scope with `effective_from: "2026-06-01"`.

That scope matters: `GPT-5.6 Terra`, `Claude Opus 5`, `Gemini 3.5 Flash`, and similar display names are mapped for Copilot only. They do not override Codex, Claude Code, Gemini, or Cursor calls with similar model names.

The upstream page uses per-provider tables: the Anthropic table carries a `Cache write` column, and the OpenAI/Google/xAI tables add `Tier`/`Threshold` columns with separate Default and Long-context rows per model. Row matching is first-match, which selects the Default tier — long-context surcharges are not modelled. The September refresh adds GPT-6 Astra, Claude Fable 5.1, Gemini 3.6/3.7/3.8 Flash, Grok 4.5/4.6, MAI-Code-1.1-Flash, and Kimi K3. Retired models keep their last-known override rows via the drift handling below.

GitHub includes cached-token rates. `tokenuse` stores those rates in the Copilot rows, but parsed Copilot transcripts currently do not expose reliable cache buckets, so cached-token billing is only applied when the parser has trustworthy cache counts.

## Cache Rates

The UI's `Cache` column remains observed cache-hit behavior from local usage data. `Cache Rate` is the pricing multiplier for cached input relative to normal input.

Current important rows:

- Most Claude prompt-cache reads are 10% of input; Fable 5.1 and Mythos 5.1 are the exception at 2.5%. Five-minute cache writes are 125% of input. One-hour cache writes are 200% of input — the books carry the 5-minute rate and the pricing formula applies a fixed 1.6x premium to the 1h share reported under `usage.cache_creation` (see [Architecture — Pricing](architecture.md#pricing)).
- Cursor's legacy Enterprise Auto row reads at 20% of input and writes at the input rate, but it expires on September 7, 2026. Current Auto modes bill the list price of the model chosen for each request; local Cursor artifacts often expose only `default`, so those unresolved post-transition calls deliberately surface through fallback visibility rather than retaining the obsolete flat estimate.
- Cursor-scoped rows cover GLM 5.2 at $1.40/$0.26 cached/$4.40, Kimi K2.7 Code at $0.95/$0.19 cached/$4, and GPT-5 Fast at $2.50/$0.25 cached/$20 per MTok. GPT-5.4 Fast is 2x its standard row, GPT-5.5 Fast uses its published $12.50/$1.25 cached/$75 rates, and GPT-5.6 Luna/Sol/Terra Fast are 2x including cache writes. These are explicit `*-fast` rows rather than base-model multipliers, so the fixed $0.01 web-search charge is not multiplied. Cursor bracket ids with a numeric `context` above 272K select the corresponding GPT-5.4 or GPT-5.6 Luna/Sol/Terra long-context row; 272K exactly remains on the default tier.
- The newly reviewed GPT-5.6 and GPT-6 Astra rows, plus current rows such as GPT-5.3-Codex and GPT-5.4, use 10% cached-input pricing; exact older variants can differ, and `codex-mini-latest` remains 25%. GPT-5.6 Sol's promotion began August 21, 2026 at $4 input, $5 cache write, $0.40 cache read, and $20 output per MTok; earlier rows retain the preceding $5/$30 rate, and the unsuffixed `gpt-5.6` alias resolves to Sol. GPT-6 Astra's bundled row is the default tier; requests above 272K input tokens use OpenAI's higher long-context rates, which `tokenuse` cannot yet select per call.
- Gemini rows are explicit overrides with source provenance because Gemini publishes prompt-length tiers that the parser cannot yet choose per call. Newly imported or rebuilt Gemini 1.5 Flash history on or after August 12, 2024 uses the short-context $0.075 input, $0.01875 cached-input, and $0.30 output rates per MTok; requests above 128K input tokens remain a documented 2x estimate gap.
- Anthropic made Sonnet 5 launch pricing permanent at $2 input, $2.50 five-minute cache writes, $0.20 cache reads, and $10 output per MTok; the previously announced September 1 increase never took effect.
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

The broad LiteLLM import accepts only chat/Responses models, requires positive input and output rates for those text-generation rows, and merges aliases that normalize to the same model without allowing a zero or incomplete alias to erase known rates. Direct provider model ids take precedence over cloud-region and proxy aliases, preventing a regional uplift from replacing a provider's standard rate. Pricing normalization preserves dated model snapshots even though display identity folds them together, so an older pin with a different rate cannot nondeterministically replace the base model. Official table scrapers also declare their required token columns, so an upstream column rename fails the refresh instead of silently writing a partial price.

### Pinned rows

Pinned rows are reserved for historical or time-bounded rates that should not be inferred from a current live table. Cursor's current Composer, Grok, GLM 5.2, Kimi K2.7 Code, GPT-5 Fast, GPT-5.5 Fast, and GPT-5.6 Fast and long-context rates are machine-readable. GPT-5.4 Fast and long-context tiers are emitted as distinct rows by scaling the freshly scraped standard row by the model page's published multipliers; only token fields are scaled. The time-bounded legacy Enterprise Auto rate and retired Composer 1/1.5/2 rows remain pinned.

Those rows use `"mode": "pinned"`. Nothing is parsed from the page — every row supplies its rates through `set`, and the note records what was verified and when. The page is still fetched, and each row's `match` string is used as a liveness check:

- **Model still named on the page.** Normal case; the pinned rates are written.
- **Model no longer named.** The refresh prints a `warning: ... keeping the pinned price` line and keeps the row, so archived calls still price.
- **No live model named at all.** The source fails loudly, mirroring the `model-rows` zero-match guard — the page has been reshuffled and the pins need re-checking by hand.
- **`"retired": true`.** The model is already gone from the source (e.g. Composer 1); the liveness check is skipped so the refresh does not warn on every run.

A pinned row cannot detect an upstream *price* change — only a rename or retirement. Re-check historical pins when the source publishes a retrospective correction.

## Local Downloads

The TUI and desktop Config pages can download published pricing books after confirmation. Local files are written as:

- `<config dir>/tokenuse/pricing-upstream.json`
- `<config dir>/tokenuse/pricing-overrides.json`

The app reloads pricing in-process after a successful download. Refreshed pricing normally applies only to newly imported calls because archive rows keep their import-time `cost_usd`. Archive schema v9 makes one deliberate exception: it repairs only rows proven to carry the withdrawn post-September-1 Sonnet 5 rates, the predecessor cache-read rate for Fable/Mythos 5.1, the former zero-token-rate GPT-5.3-Codex Spark placeholder, or GPT-6 Astra's old Sonnet fallback formula. The full archive is checked once during migration; afterward, the same idempotent guard checks each newly inserted ID range plus exact older Cursor rows whose authoritative token reconstruction updated their cost, so stale local books cannot reintroduce those errors. Ambiguous, already-correct, and unrelated rows are left alone.

Downloaded books override the embedded pair only while their latest `checked_at`/`generated_at` date is at least as new. A newer application build automatically uses its embedded books instead of an older download, and the Config status reports `EmbeddedBooks`; this prevents a stale local `pricing-overrides.json` from forcing a newly released model such as GPT-6 Astra through fallback pricing.

## Parser Caveats

- Claude Code exposes input, output, cache-write, and cache-read buckets directly.
- Codex/OpenAI reports cached input inside total input. The parser subtracts cached input before pricing and prices the cached portion as cache read.
- Gemini reports cached tokens inside input, so the Gemini parser follows the same subtract-then-price pattern.
- Cursor local files do not provide a reliable cache breakdown. Exact models use Cursor-scoped list prices, and structured GPT-5.4/5.6 ids with a numeric `context` can select Cursor's long-context tier. Non-Cursor artifacts and rows without context evidence still cannot prove that a long-context surcharge applied; unresolved Auto calls on or after September 7 likewise cannot be priced precisely until Cursor stores the routed model locally.
- Copilot local transcripts are still estimated and do not expose reliable cache buckets. GitHub's June 1, 2026 billing includes cached tokens, but tokenuse cannot reconstruct that billing exactly from local files today.
