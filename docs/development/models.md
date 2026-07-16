# Model Normalisation

`tokenuse` resolves every model identifier through one shared registry before it reaches an aggregate, report, TUI row, or desktop view. The registry gives each model a stable canonical id, display name, provider, and family. Tool adapters should preserve the model identifier they observe; they must not maintain their own display-name tables.

The implementation lives in:

- `src/models/registry.json`: ordered data rules.
- `src/models/mod.rs`: canonicalisation, rule loading, provider ids, fallback naming, and tests.
- `src/pricing/mod.rs`: pricing lookup, which reuses the same canonical key function.

## Resolution Order

`models::resolve(tool_id, raw_model)` applies these steps:

1. Trim and lowercase the raw value.
2. Remove an `@suffix`, retain only the final vendor-path segment, and strip a trailing `-YYYYMMDD` date.
3. Walk `registry.json` from top to bottom. Rules with `tool` only match that adapter; the first matching exact or prefix rule wins.
4. If no rule matches, infer common GPT, Claude, and Gemini providers and produce a readable name. Other unknown ids are title-cased and assigned to `Other` rather than being rendered raw.

The returned identity has four fields:

| Field | Purpose |
| --- | --- |
| `canonical_id` | Aggregation key shared by equivalent raw identifiers. |
| `display` | Human-facing model name. |
| `provider` | Stable provider id and label used for grouping, colors, and icons. |
| `family` | Broader product family such as `Opus`, `GPT-5 Codex`, or `Gemini`. |

Tool-scoped rules must appear before broader rules. For example, Copilot's `openai-auto`, `anthropic-auto`, and `auto` routers resolve to OpenAI, Anthropic, and GitHub identities respectively, while Cursor's `auto` and `default` remain Cursor identities.

## Cursor raw ids and precedence

Cursor records a model at several layers. Its adapter preserves the strongest observed id in this order: AgentKv assistant `providerOptions.cursor.modelName`, bubble `modelInfo.modelName`, composer `modelConfig.modelName`, tracking summary/hash model, `store.db` `lastUsedModel`, then `cursor-auto`.

Observed Cursor families include:

| Raw family | Canonical behavior | Pricing |
| --- | --- | --- |
| `claude-4.5-sonnet-thinking`, `claude-4.5-sonnet-high-thinking`, normal `claude-sonnet-4-5-*` | reversed version/family ids normalize to `claude-sonnet-4-5-*`; thinking and effort suffixes resolve to the same Sonnet identity | matching Claude row |
| `composer-1*` | `cursor-composer-1` | global fallback unless an official row is present |
| `composer-2.5*` / `composer-2-5*` | `cursor-composer-2.5`; Fast keeps a distinct display/rate but the same model-overreliance identity | official Cursor-scoped Composer 2.5 or Fast row |
| `grok-4.5*` / `grok-4-5*` | `cursor-grok-4.5`; Fast/effort suffixes share the identity | official Cursor-scoped Grok 4.5 or Fast row |
| GPT/Codex ids such as `gpt-5.1-codex-max` and newer registry-unknown variants | shared GPT fallback naming retains the full variant and OpenAI provider | matching global GPT row or fallback |
| `vega*`, including Fast/reasoning/effort variants | `cursor-vega`, displayed as `Vega (Preview)` | observed-only; documented unknown-model fallback until Cursor publishes a rate |
| `auto`, `default`, `cursor-auto`, `cursor-default` | `cursor-auto` | official Cursor Auto row |

Reasoning markers (`thinking`, `low`, `medium`, `high`, `xhigh`, `max`) and speed markers may occur in either order. The Coach effort parser searches the suffix components rather than assuming effort is the final word. Registry identities intentionally fold those suffixes for diversity analysis, while pricing still receives the raw normalized key so Fast variants can use distinct rates.

Official Cursor first-party rates are refreshed from [Cursor Models & Pricing](https://cursor.com/docs/models-and-pricing) into tool-scoped override rows:

| Model | Input / MTok | Cache read / MTok | Output / MTok |
| --- | ---: | ---: | ---: |
| Composer 2.5 | $0.50 | $0.20 | $2.50 |
| Composer 2.5 Fast | $3.00 | $0.50 | $15.00 |
| Grok 4.5 | $2.00 | $0.50 | $6.00 |
| Grok 4.5 Fast | $4.00 | $1.00 | $18.00 |

Cursor Auto remains $1.25 input/cache-write, $0.25 cache-read, and $6 output per MTok. The Teams/Enterprise Cursor Token Rate is not applied because local records do not identify an applicable billing plan.

## Registry Schema

Each entry in `registry.json` supports:

| Field | Required | Meaning |
| --- | --- | --- |
| `tool` | No | Restrict the rule to one stable adapter id. |
| `match` | Yes | `exact` or `prefix`. |
| `keys` | Yes | Canonicalised keys tested in order. |
| `canonical` | No | Fold key; defaults to the first key. |
| `display` | Yes | Human-facing name. |
| `provider` | Yes | One of `anthropic`, `openai`, `google`, `github`, `cursor`, `xai`, or `other`. |
| `family` | Yes | Human-facing family label. |

Use `exact` when a suffix changes the product identity, and `prefix` when dated, preview, or tool-specific suffixes should remain in the same row. Put the most specific prefix first: `gpt-5.3-codex-spark` must precede `gpt-5.3-codex`, for example.

## Adding Or Updating A Model

1. Capture the raw identifier and adapter id from a fixture or parser test.
2. Run it mentally through the canonicalisation rules above.
3. Add the narrowest registry rule in specificity order. Reuse a canonical id only when the rows should genuinely aggregate together.
4. Add or update tests in `src/models/mod.rs` for display, canonical folding, provider, and family behavior. Include vendor paths or dated ids when relevant.
5. Update pricing separately if the new model needs a rate. Display normalisation and pricing share canonicalisation, but a registry rule does not create a price row.
6. Check the unified desktop Models page, a dedicated tool page, the TUI, and at least one report/export surface.

Adding an entirely new provider also requires a `Provider` variant in `src/models/mod.rs`, provider color tokens in `DESIGN.md` and `desktop/src/styles/tokens.css`, and an icon mapping in `desktop/src/icons/`. Use `other` until those shared surfaces are ready.

Run the focused and full checks after changing the registry:

```bash
cargo test models::tests
cargo fmt --check
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

From `desktop/`, also run `CI=true pnpm run check` and `CI=true pnpm run build` when provider grouping, icons, or serialized model fields change.

## Pricing Boundary

`models::canonical_key` is the common normaliser for model identity and pricing. Pricing then applies its own tool-scoped effective rows, aliases, date windows, and fallback rules. Keep that separation deliberate:

- the model registry answers “what should this model be called and grouped with?”
- the pricing books answer “what did this model cost for this tool at this time?”

See [Pricing and cache rates](pricing.md) for pricing sources, refresh rules, and cache-rate behavior.
