use std::collections::HashMap;
use std::fs;
use std::path::Path;

use color_eyre::{eyre::eyre, Result};
use serde_json::{Map, Value};

const LITELLM_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

pub fn run(output: &Path) -> Result<()> {
    let body: Value = ureq::get(LITELLM_URL)
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

    for (key, val) in map {
        if !prefixes.iter().any(|p| key.contains(p)) {
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
        if key.starts_with("claude-opus") {
            out.insert("fast_multiplier".into(), 6.0);
        }
        models.insert(canonicalize(key), Value::Object(to_obj(out)));
    }

    let aliases = [
        ("cursor-auto", "claude-sonnet-4-5"),
        ("default", "claude-sonnet-4-5"),
        ("auto", "claude-sonnet-4-5"),
        ("anthropic-auto", "claude-sonnet-4-5"),
        ("openai-auto", "gpt-5"),
    ];
    let mut alias_map = Map::new();
    for (k, v) in aliases {
        alias_map.insert(k.into(), Value::String(v.into()));
    }

    let mut root = Map::new();
    root.insert("models".into(), Value::Object(models));
    root.insert("aliases".into(), Value::Object(alias_map));
    root.insert("fallback".into(), Value::String("claude-sonnet-4-5".into()));

    let pretty = serde_json::to_string_pretty(&Value::Object(root))?;
    fs::write(output, pretty)?;
    Ok(())
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
