use std::collections::HashMap;
use std::fs;
use std::path::Path;

use color_eyre::{eyre::eyre, Result};
use serde_json::{Map, Value};

pub fn run(output: &Path) -> Result<()> {
    let body: Value = ureq::get(crate::config::LITELLM_PRICING_URL)
        .call()
        .map_err(|e| eyre!("fetch litellm prices: {e}"))?
        .into_json()
        .map_err(|e| eyre!("parse litellm json: {e}"))?;

    let map = body
        .as_object()
        .ok_or_else(|| eyre!("litellm root was not an object"))?;

    let mut models: Map<String, Value> = Map::new();
    let prefixes = [
        "claude-opus-",
        "claude-sonnet-",
        "claude-haiku-",
        "claude-3-",
        "gpt-5",
        "gpt-4o",
        "o1",
        "o3",
        "gemini-",
    ];
    let exact_models = ["codex-mini-latest"];

    for (key, val) in map {
        let canonical = canonicalize(key);
        if !prefixes.iter().any(|p| key.contains(p))
            && !exact_models.iter().any(|model| canonical == *model)
        {
            continue;
        }
        let entry = val
            .as_object()
            .ok_or_else(|| eyre!("model entry {key} not an object"))?;
        let mut out = HashMap::new();
        copy_f64(entry, "input_cost_per_token", &mut out, "input");
        copy_f64(entry, "output_cost_per_token", &mut out, "output");
        copy_f64(
            entry,
            "cache_creation_input_token_cost",
            &mut out,
            "cache_write",
        );
        copy_f64(entry, "cache_read_input_token_cost", &mut out, "cache_read");
        out.entry("web_search".into()).or_insert(0.01);
        if canonical.starts_with("claude-opus") {
            out.insert("fast_multiplier".into(), 6.0);
        }
        models.insert(canonical, Value::Object(to_obj(out)));
    }
    models.insert("cursor-auto".into(), Value::Object(cursor_auto_price()));
    apply_official_overrides(&mut models);

    let aliases = [
        ("default", "cursor-auto"),
        ("auto", "cursor-auto"),
        ("claude-sonnet", "claude-sonnet-4-6"),
        ("claude-opus", "claude-opus-4-7"),
        ("claude-haiku", "claude-haiku-4-5"),
        ("anthropic-auto", "claude-sonnet-4-6"),
        ("openai-auto", "gpt-5"),
    ];
    let mut alias_map = Map::new();
    for (k, v) in aliases {
        alias_map.insert(k.into(), Value::String(v.into()));
    }

    let mut root = Map::new();
    root.insert("_metadata".into(), Value::Object(metadata()));
    root.insert("models".into(), Value::Object(models));
    root.insert("aliases".into(), Value::Object(alias_map));
    root.insert("fallback".into(), Value::String("claude-sonnet-4-6".into()));

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut pretty = serde_json::to_string_pretty(&Value::Object(root))?;
    pretty.push('\n');
    fs::write(output, pretty)?;
    Ok(())
}

fn metadata() -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("input".into(), Value::String("input_cost_per_token".into()));
    fields.insert(
        "output".into(),
        Value::String("output_cost_per_token".into()),
    );
    fields.insert(
        "cache_write".into(),
        Value::String("cache_creation_input_token_cost".into()),
    );
    fields.insert(
        "cache_read".into(),
        Value::String("cache_read_input_token_cost".into()),
    );
    fields.insert("web_search".into(), Value::String("USD per request".into()));

    let mut meta = Map::new();
    meta.insert(
        "source".into(),
        Value::String(
            "Derived from BerriAI/litellm model_prices_and_context_window.json with local official-price overrides".into(),
        ),
    );
    meta.insert(
        "schema".into(),
        Value::String("USD per token unless noted".into()),
    );
    meta.insert("fields".into(), Value::Object(fields));
    meta
}

fn cursor_auto_price() -> Map<String, Value> {
    price_obj(0.00000125, 0.000006, 0.00000125, 0.00000025)
}

fn apply_official_overrides(models: &mut Map<String, Value>) {
    let rows = [
        ("gpt-5", 0.00000125, 0.00001, 0.0, 0.000000125),
        ("gpt-5-chat", 0.00000125, 0.00001, 0.0, 0.000000125),
        ("gpt-5-chat-latest", 0.00000125, 0.00001, 0.0, 0.000000125),
        ("gpt-5.1", 0.00000125, 0.00001, 0.0, 0.000000125),
        ("gpt-5.1-chat", 0.00000125, 0.00001, 0.0, 0.000000125),
        ("gpt-5.1-chat-latest", 0.00000125, 0.00001, 0.0, 0.000000125),
        ("gpt-5-mini", 0.00000025, 0.000002, 0.0, 0.000000025),
        (
            "gpt-5-mini-2025-08-07",
            0.00000025,
            0.000002,
            0.0,
            0.000000025,
        ),
        ("gpt-5-nano", 0.00000005, 0.0000004, 0.0, 0.000000005),
        (
            "gpt-5-nano-2025-08-07",
            0.00000005,
            0.0000004,
            0.0,
            0.000000005,
        ),
        ("gemini-2.5-pro", 0.00000125, 0.00001, 0.0, 0.000000125),
        ("gemini-2.5-flash", 0.0000003, 0.0000025, 0.0, 0.00000003),
        (
            "gemini-2.5-flash-lite",
            0.0000001,
            0.0000004,
            0.0,
            0.00000001,
        ),
    ];

    for (name, input, output, cache_write, cache_read) in rows {
        models.insert(
            name.into(),
            Value::Object(price_obj(input, output, cache_write, cache_read)),
        );
    }
}

fn price_obj(input: f64, output: f64, cache_write: f64, cache_read: f64) -> Map<String, Value> {
    let mut out = HashMap::new();
    out.insert("input".into(), input);
    out.insert("output".into(), output);
    out.insert("cache_write".into(), cache_write);
    out.insert("cache_read".into(), cache_read);
    out.insert("web_search".into(), 0.01);
    to_obj(out)
}

fn copy_f64(
    src: &Map<String, Value>,
    src_key: &str,
    dst: &mut HashMap<String, f64>,
    dst_key: &str,
) {
    if let Some(v) = src.get(src_key).and_then(|v| v.as_f64()) {
        dst.insert(dst_key.into(), v);
    }
}

fn to_obj(map: HashMap<String, f64>) -> Map<String, Value> {
    let mut obj = Map::new();
    for (k, v) in map {
        obj.insert(
            k,
            serde_json::Number::from_f64(v)
                .map(Value::Number)
                .unwrap_or(Value::Null),
        );
    }
    obj
}

fn canonicalize(model: &str) -> String {
    let mut s = model.trim().to_lowercase();
    if let Some(idx) = s.rfind('/') {
        s = s[idx + 1..].to_string();
    }
    if let Some(idx) = s.find('@') {
        s.truncate(idx);
    }
    s
}
