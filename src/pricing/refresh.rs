use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};

const EMBEDDED_OVERRIDES: &str = include_str!("books/pricing-overrides.json");
const UPSTREAM_FILE_NAME: &str = "pricing-upstream.json";
const OVERRIDES_FILE_NAME: &str = "pricing-overrides.json";

#[derive(Debug, Clone)]
pub struct RefreshOutput {
    pub upstream: PathBuf,
    pub overrides: PathBuf,
}

#[derive(Debug, Deserialize)]
struct PricingSourcesConfig {
    published_books: PublishedBooks,
    sources: Vec<SourceConfig>,
}

#[derive(Debug, Deserialize)]
struct PublishedBooks {
    upstream_url: String,
    overrides_url: String,
}

#[derive(Debug, Deserialize)]
struct SourceConfig {
    id: String,
    name: String,
    url: String,
    kind: String,
    output: String,
    #[serde(default)]
    effective_from: Option<String>,
    #[serde(default)]
    include: IncludeRules,
    #[serde(default)]
    fields: FieldMap,
    #[serde(default)]
    defaults: PriceDefaults,
    #[serde(default)]
    table_heading: Option<String>,
    #[serde(default)]
    extract: Option<ExtractConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct IncludeRules {
    #[serde(default)]
    key_contains: Vec<String>,
    #[serde(default)]
    canonical_exact: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FieldMap {
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    cache_write: Option<String>,
    #[serde(default)]
    cache_read: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct PriceDefaults {
    #[serde(default)]
    web_search: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct ExtractConfig {
    mode: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    columns: ExtractColumns,
    #[serde(default)]
    labels: FieldLabels,
    #[serde(default)]
    rows: Vec<ExtractRow>,
    #[serde(default)]
    defaults: PriceDefaults,
    #[serde(default)]
    set: FixedFields,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ExtractColumns {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    price: Option<String>,
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    cache_write: Option<String>,
    #[serde(default)]
    cache_read: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FieldLabels {
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    cache_write: Option<String>,
    #[serde(default)]
    cache_read: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ExtractRow {
    model: String,
    #[serde(default, rename = "match")]
    match_text: Option<String>,
    #[serde(default)]
    effective_from: Option<String>,
    #[serde(default)]
    set: FixedFields,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FixedFields {
    #[serde(default)]
    input: Option<f64>,
    #[serde(default)]
    output: Option<f64>,
    #[serde(default)]
    cache_write: Option<f64>,
    #[serde(default)]
    cache_read: Option<f64>,
    #[serde(default)]
    web_search: Option<f64>,
    #[serde(default)]
    fast_multiplier: Option<f64>,
}

pub fn run(output_dir: &Path) -> Result<RefreshOutput> {
    let config = sources_config()?;
    let checked_at = Utc::now().date_naive().to_string();
    let mut models = Map::new();

    for source in config
        .sources
        .iter()
        .filter(|source| source.output == "upstream")
    {
        match source.kind.as_str() {
            "json-map" => {
                let body = fetch_json(source)?;
                merge_json_map_source(source, &body, &mut models)?;
            }
            other => {
                return Err(eyre!(
                    "source {} uses unsupported upstream kind {other}",
                    source.id
                ));
            }
        }
    }

    let upstream = build_upstream_book(models, &checked_at);
    fs::create_dir_all(output_dir).wrap_err_with(|| format!("create {}", output_dir.display()))?;
    let upstream_path = output_dir.join(UPSTREAM_FILE_NAME);
    write_json_value(&upstream_path, &upstream)?;

    let overrides_path = output_dir.join(OVERRIDES_FILE_NAME);
    let mut overrides = override_book_base(&overrides_path)?;
    set_book_checked_at(&mut overrides, &checked_at);
    for source in config
        .sources
        .iter()
        .filter(|source| source.output == "overrides")
    {
        merge_override_source(source, &mut overrides)?;
    }
    write_json_value(&overrides_path, &overrides)?;

    let overrides_raw = serde_json::to_string(&overrides)?;
    let upstream_raw = serde_json::to_string(&upstream)?;
    crate::pricing::PriceTable::from_books(&upstream_raw, &overrides_raw)
        .map_err(|e| eyre!("validate pricing books: {e}"))?;

    Ok(RefreshOutput {
        upstream: upstream_path,
        overrides: overrides_path,
    })
}

pub fn download_published_books(paths: &crate::config::ConfigPaths) -> Result<()> {
    let config = sources_config()?;
    let upstream_raw = fetch_string(
        "published pricing upstream",
        &config.published_books.upstream_url,
    )?;
    let overrides_raw = fetch_string(
        "published pricing overrides",
        &config.published_books.overrides_url,
    )?;

    crate::pricing::PriceTable::from_books(&upstream_raw, &overrides_raw)
        .map_err(|e| eyre!("validate published pricing books: {e}"))?;

    let upstream_value: Value = serde_json::from_str(&upstream_raw)
        .map_err(|e| eyre!("parse published pricing upstream: {e}"))?;
    let overrides_value: Value = serde_json::from_str(&overrides_raw)
        .map_err(|e| eyre!("parse published pricing overrides: {e}"))?;
    write_json_value(&paths.pricing_upstream_file, &upstream_value)?;
    write_json_value(&paths.pricing_overrides_file, &overrides_value)?;
    Ok(())
}

fn sources_config() -> Result<PricingSourcesConfig> {
    let config: PricingSourcesConfig = serde_json::from_str(super::SOURCES_CONFIG)
        .map_err(|e| eyre!("parse pricing sources config: {e}"))?;
    for source in &config.sources {
        if source.kind == "markdown-table"
            && source.table_heading.as_deref().unwrap_or("").is_empty()
            && source
                .extract
                .as_ref()
                .is_none_or(|extract| extract.mode != "model-rows")
        {
            return Err(eyre!(
                "markdown-table source {} is missing table_heading",
                source.id
            ));
        }
    }
    Ok(config)
}

fn override_book_base(overrides_path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(overrides_path).unwrap_or_else(|_| EMBEDDED_OVERRIDES.to_string());
    serde_json::from_str(&raw).map_err(|e| eyre!("parse pricing overrides base: {e}"))
}

fn fetch_json(source: &SourceConfig) -> Result<Value> {
    let raw = fetch_string(&source.name, &source.url)?;
    serde_json::from_str(&raw).map_err(|e| eyre!("parse {} json: {e}", source.id))
}

fn fetch_string(label: &str, url: &str) -> Result<String> {
    ureq::get(url)
        .call()
        .map_err(|e| eyre!("fetch {label}: {e}"))?
        .into_string()
        .map_err(|e| eyre!("read {label}: {e}"))
}

fn merge_json_map_source(
    source: &SourceConfig,
    body: &Value,
    models: &mut Map<String, Value>,
) -> Result<()> {
    let map = body
        .as_object()
        .ok_or_else(|| eyre!("{} root was not an object", source.id))?;

    for (key, val) in map {
        let canonical = canonicalize(key);
        if !source.include.matches(key, &canonical) {
            continue;
        }
        let entry = val
            .as_object()
            .ok_or_else(|| eyre!("model entry {key} from {} was not an object", source.id))?;
        let mut out = Map::new();
        copy_f64(entry, source.fields.input.as_deref(), &mut out, "input");
        copy_f64(entry, source.fields.output.as_deref(), &mut out, "output");
        copy_f64(
            entry,
            source.fields.cache_write.as_deref(),
            &mut out,
            "cache_write",
        );
        copy_f64(
            entry,
            source.fields.cache_read.as_deref(),
            &mut out,
            "cache_read",
        );
        if let Some(web_search) = source.defaults.web_search {
            insert_f64(&mut out, "web_search", web_search);
        }
        models.insert(canonical, Value::Object(out));
    }

    Ok(())
}

fn merge_override_source(source: &SourceConfig, overrides: &mut Value) -> Result<()> {
    let Some(extract) = source.extract.as_ref() else {
        return Err(eyre!(
            "override source {} is missing extract config",
            source.id
        ));
    };
    let raw = fetch_string(&source.name, &source.url)?;
    match extract.mode.as_str() {
        "label-rows" => merge_label_rows_source(source, extract, &raw, overrides),
        "model-rows" => merge_model_rows_source(source, extract, &raw, overrides),
        other => Err(eyre!(
            "source {} uses unsupported override extract mode {other}",
            source.id
        )),
    }
}

fn merge_label_rows_source(
    source: &SourceConfig,
    extract: &ExtractConfig,
    raw: &str,
    overrides: &mut Value,
) -> Result<()> {
    let model = extract
        .model
        .as_deref()
        .ok_or_else(|| eyre!("label-rows source {} is missing extract.model", source.id))?;
    let rows = first_source_table(source, raw)?;
    let header = rows
        .first()
        .ok_or_else(|| eyre!("source {} did not contain a table header", source.id))?;
    let label_idx = header_index(
        header,
        extract.columns.label.as_deref().unwrap_or("Token type"),
    )
    .ok_or_else(|| eyre!("source {} table is missing label column", source.id))?;
    let price_idx = header_index(
        header,
        extract
            .columns
            .price
            .as_deref()
            .unwrap_or("Price per 1M tokens"),
    )
    .ok_or_else(|| eyre!("source {} table is missing price column", source.id))?;

    let mut price = Map::new();
    for (field, wanted) in [
        ("input", extract.labels.input.as_deref()),
        ("output", extract.labels.output.as_deref()),
        ("cache_write", extract.labels.cache_write.as_deref()),
        ("cache_read", extract.labels.cache_read.as_deref()),
    ] {
        let Some(wanted) = wanted else {
            continue;
        };
        let value = rows
            .iter()
            .skip(1)
            .find(|row| {
                row.get(label_idx)
                    .map(|cell| comparable(cell) == comparable(wanted))
                    .unwrap_or(false)
            })
            .and_then(|row| row.get(price_idx))
            .and_then(|cell| price_per_token(cell));
        if let Some(value) = value {
            insert_f64(&mut price, field, value);
        } else {
            return Err(eyre!(
                "source {} did not contain label row {wanted}",
                source.id
            ));
        }
    }

    apply_defaults_and_fixed(&mut price, &extract.defaults, &extract.set);
    add_provenance(source, extract.note.as_deref(), &mut price);
    insert_override_price(source, extract, model, price, overrides)
}

fn merge_model_rows_source(
    source: &SourceConfig,
    extract: &ExtractConfig,
    raw: &str,
    overrides: &mut Value,
) -> Result<()> {
    let tables = source_tables(source, raw)?;
    for row_rule in &extract.rows {
        let match_text = row_rule
            .match_text
            .as_deref()
            .unwrap_or(row_rule.model.as_str());
        let mut price = None;
        for table in &tables {
            if let Some(candidate) = price_from_model_table(source, extract, table, match_text)? {
                price = Some(candidate);
                break;
            }
        }
        let mut price =
            price.ok_or_else(|| eyre!("source {} did not contain row {match_text}", source.id))?;
        apply_defaults_and_fixed(&mut price, &extract.defaults, &extract.set);
        apply_fixed(&mut price, &row_rule.set);
        let note = row_rule.note.as_deref().or(extract.note.as_deref());
        add_provenance(source, note, &mut price);
        if let Some(effective_from) = row_rule
            .effective_from
            .as_deref()
            .or(source.effective_from.as_deref())
        {
            price.insert(
                "effective_from".into(),
                Value::String(effective_from.into()),
            );
        }
        insert_override_price(source, extract, &row_rule.model, price, overrides)?;
    }
    Ok(())
}

fn price_from_model_table(
    source: &SourceConfig,
    extract: &ExtractConfig,
    table: &[Vec<String>],
    match_text: &str,
) -> Result<Option<Map<String, Value>>> {
    let Some(header) = table.first() else {
        return Ok(None);
    };
    let model_idx = header_index(header, extract.columns.model.as_deref().unwrap_or("Model"))
        .ok_or_else(|| eyre!("source {} table is missing model column", source.id))?;
    let wanted = comparable(match_text);
    let Some(row) = table.iter().skip(1).find(|row| {
        row.get(model_idx)
            .map(|cell| comparable(cell) == wanted)
            .unwrap_or(false)
    }) else {
        return Ok(None);
    };

    let mut price = Map::new();
    copy_price_from_table(
        row,
        header,
        extract.columns.input.as_deref(),
        &mut price,
        "input",
    );
    copy_price_from_table(
        row,
        header,
        extract.columns.output.as_deref(),
        &mut price,
        "output",
    );
    copy_price_from_table(
        row,
        header,
        extract.columns.cache_write.as_deref(),
        &mut price,
        "cache_write",
    );
    copy_price_from_table(
        row,
        header,
        extract.columns.cache_read.as_deref(),
        &mut price,
        "cache_read",
    );
    Ok(Some(price))
}

fn copy_price_from_table(
    row: &[String],
    header: &[String],
    column: Option<&str>,
    price: &mut Map<String, Value>,
    field: &str,
) {
    let Some(column) = column else {
        return;
    };
    if let Some(value) = header_index(header, column)
        .and_then(|idx| row.get(idx))
        .and_then(|cell| price_per_token(cell))
    {
        insert_f64(price, field, value);
    }
}

fn first_source_table(source: &SourceConfig, raw: &str) -> Result<Vec<Vec<String>>> {
    source_tables(source, raw)?
        .into_iter()
        .next()
        .ok_or_else(|| eyre!("source {} did not contain a table", source.id))
}

fn source_tables(source: &SourceConfig, raw: &str) -> Result<Vec<Vec<Vec<String>>>> {
    match source.kind.as_str() {
        "markdown-table" => Ok(markdown_tables(raw, source.table_heading.as_deref())),
        "html-table" => Ok(html_tables(raw, source.table_heading.as_deref())),
        "text" => Ok(markdown_tables(
            &html_to_text(raw),
            source.table_heading.as_deref(),
        )),
        other => Err(eyre!(
            "source {} uses unsupported override source kind {other}",
            source.id
        )),
    }
}

fn apply_defaults_and_fixed(
    price: &mut Map<String, Value>,
    defaults: &PriceDefaults,
    fixed: &FixedFields,
) {
    if let Some(web_search) = defaults.web_search {
        price
            .entry("web_search")
            .or_insert_with(|| json!(web_search));
    }
    apply_fixed(price, fixed);
}

fn apply_fixed(price: &mut Map<String, Value>, fixed: &FixedFields) {
    for (field, value) in [
        ("input", fixed.input),
        ("output", fixed.output),
        ("cache_write", fixed.cache_write),
        ("cache_read", fixed.cache_read),
        ("web_search", fixed.web_search),
        ("fast_multiplier", fixed.fast_multiplier),
    ] {
        if let Some(value) = value {
            insert_f64(price, field, value);
        }
    }
}

fn add_provenance(source: &SourceConfig, note: Option<&str>, price: &mut Map<String, Value>) {
    let mut provenance = Map::new();
    provenance.insert("source_name".into(), Value::String(source.name.clone()));
    provenance.insert("source_url".into(), Value::String(source.url.clone()));
    provenance.insert(
        "checked_at".into(),
        Value::String(Utc::now().date_naive().to_string()),
    );
    if let Some(note) = note {
        provenance.insert("note".into(), Value::String(note.into()));
    }
    price.insert("provenance".into(), Value::Object(provenance));
}

fn insert_override_price(
    source: &SourceConfig,
    extract: &ExtractConfig,
    model: &str,
    price: Map<String, Value>,
    overrides: &mut Value,
) -> Result<()> {
    let scope = extract.scope.as_deref().unwrap_or("global");
    match scope {
        "global" => {
            let models = object_member(overrides, "models")?;
            models.insert(canonicalize(model), Value::Object(price));
        }
        "tool" => {
            let tool = extract
                .tool
                .as_deref()
                .ok_or_else(|| eyre!("tool-scoped source {} is missing extract.tool", source.id))?;
            let tool_models = object_member(overrides, "tool_models")?;
            let tool_entry = tool_models
                .entry(tool.trim().to_ascii_lowercase())
                .or_insert_with(|| Value::Object(Map::new()));
            let Value::Object(models) = tool_entry else {
                return Err(eyre!("tool_models.{tool} must be an object"));
            };
            models.insert(canonicalize(model), Value::Object(price));
        }
        other => {
            return Err(eyre!(
                "source {} uses unsupported override scope {other}",
                source.id
            ));
        }
    }
    Ok(())
}

fn object_member<'a>(value: &'a mut Value, key: &str) -> Result<&'a mut Map<String, Value>> {
    let Value::Object(root) = value else {
        return Err(eyre!("pricing override book root must be an object"));
    };
    let entry = root
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(map) = entry else {
        return Err(eyre!("pricing override book {key} must be an object"));
    };
    Ok(map)
}

impl IncludeRules {
    fn matches(&self, raw_key: &str, canonical: &str) -> bool {
        self.key_contains
            .iter()
            .any(|needle| raw_key.contains(needle))
            || self
                .canonical_exact
                .iter()
                .any(|model| canonical == model.as_str())
    }
}

fn build_upstream_book(models: Map<String, Value>, checked_at: &str) -> Value {
    let mut fields = Map::new();
    fields.insert("input".into(), Value::String("USD per input token".into()));
    fields.insert(
        "output".into(),
        Value::String("USD per output token".into()),
    );
    fields.insert(
        "cache_write".into(),
        Value::String("USD per cache write token".into()),
    );
    fields.insert(
        "cache_read".into(),
        Value::String("USD per cache read token".into()),
    );
    fields.insert(
        "web_search".into(),
        Value::String("USD per web search request".into()),
    );

    let mut metadata = Map::new();
    metadata.insert(
        "schema".into(),
        Value::String("tokenuse pricing-upstream.v1".into()),
    );
    metadata.insert(
        "source".into(),
        Value::String("Generated from pricing-sources.json upstream sources".into()),
    );
    metadata.insert("checked_at".into(), Value::String(checked_at.into()));
    metadata.insert("fields".into(), Value::Object(fields));

    let mut root = Map::new();
    root.insert("_metadata".into(), Value::Object(metadata));
    root.insert("models".into(), Value::Object(models));
    Value::Object(root)
}

fn set_book_checked_at(root: &mut Value, checked_at: &str) {
    let Value::Object(root) = root else {
        return;
    };
    let metadata = root
        .entry("_metadata")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(metadata) = metadata {
        metadata.insert("checked_at".into(), Value::String(checked_at.into()));
    }
}

fn copy_f64(
    src: &Map<String, Value>,
    src_key: Option<&str>,
    dst: &mut Map<String, Value>,
    dst_key: &str,
) {
    let Some(src_key) = src_key else {
        return;
    };
    if let Some(v) = src.get(src_key).and_then(|v| v.as_f64()) {
        insert_f64(dst, dst_key, v);
    }
}

fn insert_f64(dst: &mut Map<String, Value>, key: &str, value: f64) {
    let value = serde_json::Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null);
    dst.insert(key.into(), value);
}

fn write_json_value(output: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    let mut pretty = serde_json::to_string_pretty(value)?;
    pretty.push('\n');
    fs::write(output, pretty).wrap_err_with(|| format!("write {}", output.display()))?;
    Ok(())
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

fn markdown_tables(raw: &str, heading: Option<&str>) -> Vec<Vec<Vec<String>>> {
    let mut lines = raw.lines();
    if let Some(heading) = heading {
        for line in lines.by_ref() {
            if comparable(line.trim().trim_matches('#')) == comparable(heading) {
                break;
            }
        }
    }

    let mut tables = Vec::new();
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in lines {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            if in_table {
                if !rows.is_empty() {
                    tables.push(std::mem::take(&mut rows));
                }
                if heading.is_some() {
                    break;
                }
                in_table = false;
            }
            continue;
        }
        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(clean_cell)
            .collect();
        if cells
            .iter()
            .all(|cell| cell.chars().all(|ch| matches!(ch, '-' | ':' | ' ' | '\t')))
        {
            in_table = true;
            continue;
        }
        if cells.iter().all(|cell| cell.is_empty()) {
            in_table = true;
            continue;
        }
        in_table = true;
        rows.push(cells);
    }
    if !rows.is_empty() {
        tables.push(rows);
    }
    tables
}

#[cfg(test)]
fn markdown_table(raw: &str, heading: Option<&str>) -> Vec<Vec<String>> {
    markdown_tables(raw, heading)
        .into_iter()
        .next()
        .unwrap_or_default()
}

fn header_index(header: &[String], wanted: &str) -> Option<usize> {
    let wanted = comparable(wanted);
    header.iter().position(|cell| comparable(cell) == wanted)
}

fn clean_cell(cell: &str) -> String {
    let mut out = String::new();
    let mut chars = cell.trim().chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '[' => {
                let mut label = String::new();
                for inner in chars.by_ref() {
                    if inner == ']' {
                        break;
                    }
                    label.push(inner);
                }
                if chars.peek() == Some(&'(') {
                    for inner in chars.by_ref() {
                        if inner == ')' {
                            break;
                        }
                    }
                }
                if label.starts_with('^') {
                    continue;
                }
                out.push_str(&label);
            }
            '\\' => {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            }
            '<' => {
                for inner in chars.by_ref() {
                    if inner == '>' {
                        break;
                    }
                }
            }
            _ => out.push(ch),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn comparable(value: &str) -> String {
    let mut out = String::new();
    let clean = clean_cell(value);
    let mut chars = clean.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '[' && chars.peek() == Some(&'^') {
            for inner in chars.by_ref() {
                if inner == ']' {
                    break;
                }
            }
            continue;
        }
        if ch.is_ascii_alphanumeric() || ch == '.' {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || matches!(ch, '-' | '_' | '+' | '/') {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn price_per_token(cell: &str) -> Option<f64> {
    let value = dollar_values(cell).into_iter().next()?;
    Some(value / 1_000_000.0)
}

fn dollar_values(cell: &str) -> Vec<f64> {
    let mut values = Vec::new();
    let mut chars = cell.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '$' {
            continue;
        }
        let mut number = String::new();
        while let Some(&next) = chars.peek() {
            if next.is_ascii_digit() || next == '.' || next == ',' {
                number.push(next);
                chars.next();
            } else {
                break;
            }
        }
        if let Ok(value) = number.replace(',', "").parse::<f64>() {
            values.push(value);
        }
    }
    values
}

fn html_to_text(raw: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => {
                in_tag = true;
                out.push('\n');
            }
            '>' => {
                in_tag = false;
                out.push('\n');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
}

fn html_tables(raw: &str, heading: Option<&str>) -> Vec<Vec<Vec<String>>> {
    let mut start = 0;
    let mut single = false;
    if let Some(heading) = heading {
        if let Some(idx) = find_ascii_case_insensitive(raw, heading) {
            start = idx;
            single = true;
        }
    }

    let mut tables = Vec::new();
    let mut rest = &raw[start..];
    while let Some(table_start) = find_ascii_case_insensitive(rest, "<table") {
        rest = &rest[table_start..];
        let Some(table_end) = find_ascii_case_insensitive(rest, "</table>") else {
            break;
        };
        let table_html = &rest[..table_end];
        let table = html_table_rows(table_html);
        if !table.is_empty() {
            tables.push(table);
            if single {
                break;
            }
        }
        rest = &rest[table_end + "</table>".len()..];
    }
    tables
}

fn html_table_rows(table: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut rest = table;
    while let Some(row_start) = find_ascii_case_insensitive(rest, "<tr") {
        rest = &rest[row_start..];
        let Some(row_end) = find_ascii_case_insensitive(rest, "</tr>") else {
            break;
        };
        let row_html = &rest[..row_end];
        let row = html_cells(row_html);
        if !row.is_empty() && !row.iter().all(|cell| cell.is_empty()) {
            rows.push(row);
        }
        rest = &rest[row_end + "</tr>".len()..];
    }
    rows
}

fn html_cells(row: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut rest = row;
    loop {
        let th = find_ascii_case_insensitive(rest, "<th");
        let td = find_ascii_case_insensitive(rest, "<td");
        let next_cell = match (th, td) {
            (Some(th), Some(td)) if th < td => Some((th, "</th>")),
            (Some(th), None) => Some((th, "</th>")),
            (_, Some(td)) => Some((td, "</td>")),
            (None, None) => None,
        };
        let Some((cell_start, close_tag)) = next_cell else {
            break;
        };
        rest = &rest[cell_start..];
        let Some(open_end) = rest.find('>') else {
            break;
        };
        let Some(cell_end) = find_ascii_case_insensitive(rest, close_tag) else {
            break;
        };
        cells.push(clean_cell(&html_to_text(&rest[open_end + 1..cell_end])));
        rest = &rest[cell_end + close_tag.len()..];
    }
    cells
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|window| {
        window
            .iter()
            .zip(needle.iter())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_config_parses() {
        let config = sources_config().unwrap();

        assert!(config.sources.iter().any(|source| source.id == "litellm"));
        assert!(config
            .sources
            .iter()
            .any(|source| source.url.ends_with("fast-mode.md")));
        assert!(config
            .sources
            .iter()
            .any(|source| source.url.ends_with("models-and-pricing.md")));
        assert!(config.sources.iter().any(|source| {
            source.id == "github-copilot-pricing"
                && source.kind == "markdown-table"
                && source.output == "overrides"
        }));
    }

    #[test]
    fn json_map_source_uses_configured_filters_and_fields() {
        let source: SourceConfig = serde_json::from_str(
            r#"{
              "id": "fixture",
              "name": "Fixture",
              "url": "https://example.invalid/prices.json",
              "kind": "json-map",
              "output": "upstream",
              "include": { "key_contains": ["gpt-"], "canonical_exact": ["codex-mini-latest"] },
              "fields": {
                "input": "in",
                "output": "out",
                "cache_write": "cw",
                "cache_read": "cr"
              },
              "defaults": { "web_search": 0.01 }
            }"#,
        )
        .unwrap();
        let body: Value = serde_json::from_str(
            r#"{
              "openai/gpt-fixture@v1": { "in": 0.1, "out": 0.2, "cw": 0.3, "cr": 0.4 },
              "codex-mini-latest": { "in": 1.0, "out": 2.0 },
              "ignored": { "in": 9.0, "out": 9.0 }
            }"#,
        )
        .unwrap();
        let mut models = Map::new();

        merge_json_map_source(&source, &body, &mut models).unwrap();

        assert!(models.contains_key("gpt-fixture"));
        assert!(models.contains_key("codex-mini-latest"));
        assert!(!models.contains_key("ignored"));
    }

    #[test]
    fn markdown_table_extracts_table_after_heading() {
        let raw = r#"
# Title

## Other

| Nope |
| --- |
| x |

## Pricing

| Model | Input | Output |
| --- | ---: | ---: |
| GPT | $1 | $2 |

after
"#;

        let rows = markdown_table(raw, Some("Pricing"));

        assert_eq!(rows[0], vec!["Model", "Input", "Output"]);
        assert_eq!(rows[1], vec!["GPT", "$1", "$2"]);
    }

    #[test]
    fn label_rows_extract_cursor_auto_override() {
        let source: SourceConfig = serde_json::from_str(
            r#"{
              "id": "cursor",
              "name": "Cursor pricing",
              "url": "https://example.invalid/cursor.md",
              "kind": "markdown-table",
              "output": "overrides",
              "table_heading": "Auto pricing",
              "extract": {
                "mode": "label-rows",
                "scope": "global",
                "model": "cursor-auto",
                "columns": { "label": "Token type", "price": "Price per 1M tokens" },
                "labels": {
                  "input": "Input + Cache Write",
                  "cache_write": "Input + Cache Write",
                  "output": "Output",
                  "cache_read": "Cache Read"
                },
                "defaults": { "web_search": 0.01 }
              }
            }"#,
        )
        .unwrap();
        let raw = r#"
### Auto pricing

| Token type | Price per 1M tokens |
| --- | ---: |
| Input + Cache Write | $1.25 |
| Output | $6.00 |
| Cache Read | $0.25 |
"#;
        let mut overrides = json!({ "fallback": "cursor-auto", "models": {} });

        merge_label_rows_source(
            &source,
            source.extract.as_ref().unwrap(),
            raw,
            &mut overrides,
        )
        .unwrap();

        let row = overrides.pointer("/models/cursor-auto").unwrap();
        assert_eq!(row["input"], json!(0.00000125));
        assert_eq!(row["cache_write"], json!(0.00000125));
        assert_eq!(row["cache_read"], json!(0.00000025));
        assert_eq!(row["output"], json!(0.000006));
    }

    #[test]
    fn model_rows_extract_tool_scoped_effective_override() {
        let source: SourceConfig = serde_json::from_str(
            r#"{
              "id": "github",
              "name": "GitHub Copilot models and pricing",
              "url": "https://example.invalid/github.md",
              "kind": "markdown-table",
              "output": "overrides",
              "effective_from": "2026-06-01",
              "extract": {
                "mode": "model-rows",
                "scope": "tool",
                "tool": "copilot",
                "columns": {
                  "model": "Model",
                  "input": "Input",
                  "cache_read": "Cached input",
                  "output": "Output"
                },
                "rows": [{ "match": "GPT-4.1", "model": "gpt-4.1" }]
              }
            }"#,
        )
        .unwrap();
        let raw = r#"
### OpenAI

| Model | Release status | Input | Cached input | Output |
| --- | --- | ---: | ---: | ---: |
| | | | | |
| GPT-4.1[^1] | GA | $2.00 | $0.50 | $8.00 |
"#;
        let mut overrides = json!({ "fallback": "gpt-4.1", "tool_models": {} });

        merge_model_rows_source(
            &source,
            source.extract.as_ref().unwrap(),
            raw,
            &mut overrides,
        )
        .unwrap();

        let row = overrides.pointer("/tool_models/copilot/gpt-4.1").unwrap();
        assert_eq!(row["input"], json!(0.000002));
        assert_eq!(row["cache_read"], json!(0.0000005));
        assert_eq!(row["output"], json!(0.000008));
        assert_eq!(row["effective_from"], json!("2026-06-01"));
    }

    #[test]
    fn html_table_extracts_text_fallback_rows() {
        let source: SourceConfig = serde_json::from_str(
            r#"{
              "id": "html",
              "name": "HTML pricing",
              "url": "https://example.invalid/pricing",
              "kind": "html-table",
              "output": "overrides",
              "table_heading": "Pricing",
              "extract": {
                "mode": "model-rows",
                "scope": "global",
                "columns": {
                  "model": "Model",
                  "input": "Input",
                  "cache_read": "Cached input",
                  "output": "Output"
                },
                "rows": [{ "match": "Gemini 3 Flash", "model": "gemini-3-flash" }]
              }
            }"#,
        )
        .unwrap();
        let raw = r#"
<h2>Pricing</h2>
<table>
  <tr><th>Model</th><th>Input</th><th>Cached input</th><th>Output</th></tr>
  <tr><td><a href="/models/gemini">Gemini 3 Flash</a></td><td>$0.50</td><td>$0.05</td><td>$3.00</td></tr>
</table>
"#;
        let mut overrides = json!({ "fallback": "gemini-3-flash", "models": {} });

        merge_model_rows_source(
            &source,
            source.extract.as_ref().unwrap(),
            raw,
            &mut overrides,
        )
        .unwrap();

        let row = overrides.pointer("/models/gemini-3-flash").unwrap();
        assert!((row["input"].as_f64().unwrap() - 0.0000005).abs() < f64::EPSILON);
        assert!((row["cache_read"].as_f64().unwrap() - 0.00000005).abs() < f64::EPSILON);
        assert!((row["output"].as_f64().unwrap() - 0.000003).abs() < f64::EPSILON);
    }

    #[test]
    fn unsupported_upstream_kind_fails() {
        let source: SourceConfig = serde_json::from_str(
            r#"{
              "id": "fixture",
              "name": "Fixture",
              "url": "https://example.invalid/prices.md",
              "kind": "markdown-table",
              "output": "upstream"
            }"#,
        )
        .unwrap();

        assert_eq!(source.kind, "markdown-table");
        assert_ne!(source.kind, "json-map");
    }
}
