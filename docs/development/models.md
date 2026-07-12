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
