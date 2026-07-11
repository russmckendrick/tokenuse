use std::collections::HashMap;
#[cfg(feature = "quota-sync")]
use std::collections::HashSet;
use std::fs;
use std::path::Path;
#[cfg(feature = "quota-sync")]
use std::process::Command;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde::Deserialize;
use serde_json::Value;

use super::config;
use crate::tools::{LimitCredits, LimitSnapshot, LimitWindow, SessionSource};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Sidecar {
    Wrapped {
        observed_at: Option<DateTime<Utc>>,
        #[serde(default)]
        host: Option<String>,
        #[serde(default)]
        login: Option<String>,
        payload: CopilotUsage,
    },
    Raw(CopilotUsage),
}

#[derive(Debug, Deserialize)]
struct CopilotUsage {
    copilot_plan: Option<String>,
    access_type_sku: Option<String>,
    quota_reset_date: Option<String>,
    quota_reset_date_utc: Option<String>,
    quota_snapshots: Option<HashMap<String, QuotaSnapshot>>,
    token_based_billing: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct QuotaSnapshot {
    entitlement: Option<f64>,
    percent_remaining: Option<f64>,
    remaining: Option<f64>,
    quota_remaining: Option<f64>,
    unlimited: Option<bool>,
    overage_permitted: Option<bool>,
    timestamp_utc: Option<DateTime<Utc>>,
    token_based_billing: Option<bool>,
}

pub fn parse_sidecar(source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
    if source.tool != config::TOOL_ID {
        return Err(eyre!("Copilot limit source had wrong tool id"));
    }

    let raw = fs::read_to_string(&source.path)
        .wrap_err_with(|| format!("read {}", source.path.display()))?;
    let fallback_observed_at = fs::metadata(&source.path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(DateTime::<Utc>::from);
    parse_sidecar_str(&raw, fallback_observed_at)
}

fn parse_sidecar_str(
    raw: &str,
    fallback_observed_at: Option<DateTime<Utc>>,
) -> Result<Vec<LimitSnapshot>> {
    let sidecar: Sidecar =
        serde_json::from_str(raw).map_err(|e| eyre!("parse Copilot limits sidecar: {e}"))?;
    let (observed_at, usage, account_host, account_login) = match sidecar {
        Sidecar::Wrapped {
            observed_at,
            host,
            login,
            payload,
        } => (observed_at.or(fallback_observed_at), payload, host, login),
        Sidecar::Raw(usage) => (fallback_observed_at, usage, None, None),
    };
    let plan_type = copilot_plan_type(&usage);

    let Some(snapshots) = usage.quota_snapshots else {
        return Ok(Vec::new());
    };
    let reset = usage
        .quota_reset_date
        .as_deref()
        .and_then(parse_reset)
        .or_else(|| usage.quota_reset_date_utc.as_deref().and_then(parse_reset));

    let mut rows = Vec::new();
    let mut placeholder_rows = 0usize;
    let mut snapshots: Vec<_> = snapshots.into_iter().collect();
    snapshots.sort_by(|a, b| a.0.cmp(&b.0));
    for (id, snapshot) in snapshots {
        if snapshot.unlimited.unwrap_or(false) && snapshot.entitlement.unwrap_or(0.0) <= 0.0 {
            placeholder_rows += 1;
            continue;
        }

        let remaining = snapshot.quota_remaining.or(snapshot.remaining);

        let Some(percent_remaining) = snapshot
            .percent_remaining
            .or_else(|| percent_remaining_from_balance(&snapshot, remaining))
        else {
            continue;
        };
        let used_percent = (100.0 - percent_remaining).clamp(0.0, 100.0);
        let reached = remaining
            .is_some_and(|remaining| remaining <= 0.0)
            .then(|| "primary".to_string());

        let row_observed = snapshot.timestamp_utc.or(observed_at);
        let credits_era = token_based_billing_enabled(usage.token_based_billing.as_ref())
            .or(snapshot.token_based_billing)
            .unwrap_or_else(|| row_observed.is_some_and(|at| at >= credits_era_start()));

        rows.push(LimitSnapshot {
            tool: config::TOOL_ID,
            limit_id: id.clone(),
            limit_name: Some(human_limit_name(&id, credits_era)),
            plan_type: plan_type.clone(),
            observed_at: row_observed,
            primary: Some(LimitWindow {
                used_percent,
                window_minutes: window_minutes_for(&id),
                resets_at: reset,
            }),
            secondary: None,
            credits: Some(LimitCredits {
                has_credits: snapshot.entitlement.is_some()
                    || snapshot.remaining.is_some()
                    || snapshot.quota_remaining.is_some(),
                unlimited: snapshot.unlimited.unwrap_or(false),
                balance: remaining,
                total: snapshot.entitlement,
                additional_usage: snapshot.overage_permitted,
            }),
            rate_limit_reached_type: reached,
        });
    }

    // Business/Enterprise org seats: GitHub returns every quota snapshot as a
    // zero-entitlement placeholder (unlimited, no reset date) and never
    // exposes the org credit pool to the member. Emit one informational row
    // so the seat doesn't render as an empty section. Individual payloads
    // always carry a constrained premium row plus a reset date, so the triple
    // guard keeps them out.
    if rows.is_empty() && placeholder_rows > 0 && reset.is_none() {
        rows.push(org_managed_row(plan_type, observed_at));
    }

    apply_account_labels(&mut rows, account_host.as_deref(), account_login.as_deref());

    Ok(rows)
}

/// Per-account sidecars carry the account in the wrapper; suffix the limit id
/// so two accounts' gauges coexist through the `(tool, limit_id)` dedup, and
/// append the login (or host) to the display name. Legacy single-account
/// sidecars have neither field and keep their historical ids.
fn apply_account_labels(rows: &mut [LimitSnapshot], host: Option<&str>, login: Option<&str>) {
    let id_suffix = match (host, login) {
        (Some(host), Some(login)) => format!("{host}/{login}"),
        (Some(host), None) => host.to_string(),
        (None, Some(login)) => login.to_string(),
        (None, None) => return,
    };
    let display = login.or(host).unwrap_or_default().to_string();
    for row in rows {
        row.limit_id = format!("{}@{id_suffix}", row.limit_id);
        if let Some(name) = row.limit_name.take() {
            row.limit_name = Some(format!("{name} · {display}"));
        }
    }
}

/// Whether any Copilot limit sidecar exists next to the legacy path — the
/// legacy `copilot.json` or any per-account `copilot-<host>-<login>.json`.
pub fn any_sidecar_present(legacy_file: &Path) -> bool {
    if legacy_file.is_file() {
        return true;
    }
    let Some(limits_dir) = legacy_file.parent() else {
        return false;
    };
    let Ok(entries) = fs::read_dir(limits_dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .is_some_and(config::is_limit_sidecar_name)
            && entry.path().is_file()
    })
}

const ORG_MANAGED_LIMIT_ID: &str = "org_managed_credits";

fn org_managed_row(plan_type: Option<String>, observed_at: Option<DateTime<Utc>>) -> LimitSnapshot {
    LimitSnapshot {
        tool: config::TOOL_ID,
        limit_id: ORG_MANAGED_LIMIT_ID.into(),
        limit_name: Some(crate::copy::copy().usage.org_managed_credits.clone()),
        plan_type,
        observed_at,
        primary: Some(LimitWindow {
            used_percent: 0.0,
            window_minutes: 43_200,
            resets_at: None,
        }),
        secondary: None,
        credits: Some(LimitCredits {
            has_credits: false,
            unlimited: true,
            balance: None,
            total: None,
            additional_usage: None,
        }),
        rate_limit_reached_type: None,
    }
}

/// Sync quota sidecars for every discovered Copilot account. A single
/// account keeps the legacy unlabeled `copilot.json` (stable gauge identity
/// for existing users); multiple accounts each get a labeled
/// `copilot-<host>-<login>.json` and the legacy file is removed so one
/// account's gauges aren't shown twice. Returns the total snapshot count.
#[cfg(feature = "quota-sync")]
pub fn refresh_sidecar(output: &Path) -> Result<usize> {
    let accounts: Vec<CopilotAccount> = discover_accounts()
        .into_iter()
        .filter(|account| config::copilot_user_url(&account.host).is_some())
        .collect();
    if accounts.is_empty() {
        return Err(eyre!(
            "Copilot OAuth token not found — no github-copilot config files, no Copilot CLI \
             accounts, and the GitHub CLI (gh) was not found on PATH or in /opt/homebrew/bin, \
             /usr/local/bin, or ~/.local/bin"
        ));
    }

    if let [account] = accounts.as_slice() {
        return fetch_account_sidecar(account, output, false);
    }

    let limits_dir = output.parent().unwrap_or_else(|| Path::new("."));
    let mut synced = 0usize;
    let mut first_error = None;
    for account in &accounts {
        let file = limits_dir.join(config::account_sidecar_file_name(
            &account.host,
            account.login.as_deref(),
        ));
        match fetch_account_sidecar(account, &file, true) {
            Ok(count) => synced += count,
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if synced == 0 {
        if let Some(error) = first_error {
            return Err(error);
        }
    }
    // The unlabeled legacy sidecar would render one account's gauges twice.
    let _ = fs::remove_file(output);
    Ok(synced)
}

#[cfg(feature = "quota-sync")]
fn fetch_account_sidecar(account: &CopilotAccount, output: &Path, labeled: bool) -> Result<usize> {
    let url = config::copilot_user_url(&account.host)
        .ok_or_else(|| eyre!("Copilot quota sync does not support host {}", account.host))?;
    let raw = ureq::get(&url)
        .timeout(crate::quota_sync::HTTP_TIMEOUT)
        .set("Accept", "application/json")
        .set("User-Agent", "tokenuse")
        .set("Authorization", &format!("Bearer {}", account.token))
        .call()
        .map_err(|e| eyre!("fetch Copilot limits for {}: {e}", account.label()))?
        .into_string()
        .map_err(|e| eyre!("read Copilot limits for {}: {e}", account.label()))?;
    let payload: Value =
        serde_json::from_str(&raw).map_err(|e| eyre!("parse Copilot limits json: {e}"))?;
    let count = payload
        .get("quota_snapshots")
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len);
    write_sidecar(output, payload, &url, labeled.then_some(account))?;
    Ok(count)
}

#[cfg(not(feature = "quota-sync"))]
pub fn refresh_sidecar(_output: &Path) -> Result<usize> {
    Err(eyre!("Copilot limit sync unavailable in this build"))
}

#[cfg(feature = "quota-sync")]
fn write_sidecar(
    output: &Path,
    payload: Value,
    source: &str,
    account: Option<&CopilotAccount>,
) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    let mut wrapped = serde_json::json!({
        "observed_at": Utc::now().to_rfc3339(),
        "source": source,
        "payload": payload,
    });
    if let Some(account) = account {
        wrapped["host"] = serde_json::json!(account.host);
        if let Some(login) = &account.login {
            wrapped["login"] = serde_json::json!(login);
        }
    }
    let mut pretty = serde_json::to_string_pretty(&wrapped)?;
    pretty.push('\n');
    fs::write(output, pretty).wrap_err_with(|| format!("write {}", output.display()))?;
    Ok(())
}

/// A GitHub identity the quota sync can act for: which host to query and the
/// OAuth token that is valid there (tokens are region-locked).
#[cfg(feature = "quota-sync")]
#[derive(Debug, Clone)]
struct CopilotAccount {
    host: String,
    login: Option<String>,
    token: String,
}

#[cfg(feature = "quota-sync")]
impl CopilotAccount {
    fn label(&self) -> String {
        match &self.login {
            Some(login) => format!("{login}@{}", self.host),
            None => self.host.clone(),
        }
    }
}

/// Collect (host, login, token) tuples from every local source, in priority
/// order: github-copilot credential files, gh CLI hosts, the Copilot CLI's
/// own account store, then environment tokens. Duplicates (same host+token
/// or host+login) keep the earliest source.
#[cfg(feature = "quota-sync")]
fn discover_accounts() -> Vec<CopilotAccount> {
    let mut accounts = Vec::new();
    for file in config::credential_files() {
        if let Ok(raw) = fs::read_to_string(&file) {
            if let Ok(value) = serde_json::from_str::<Value>(&raw) {
                collect_credential_accounts(&value, &mut accounts);
            }
        }
    }
    accounts.extend(gh_cli_accounts());
    if let Some(cli_root) = config::cli_root() {
        let store = cli_root.join(config::CLI_DATA_STORE_FILE);
        if store.is_file() {
            accounts.extend(
                super::parser::cli_accounts(&store)
                    .into_iter()
                    .map(|account| CopilotAccount {
                        host: account.host,
                        login: Some(account.login),
                        token: account.token,
                    }),
            );
        }
    }
    if let Some(account) = env_account() {
        accounts.push(account);
    }
    dedup_accounts(accounts)
}

/// hosts.json (older) keys entries by bare host; apps.json (newer) keys by
/// `host:client-id`. Values carry `oauth_token` and usually `user`. Entries
/// for github.com and *.ghe.com hosts coexist in the same file.
#[cfg(feature = "quota-sync")]
fn collect_credential_accounts(value: &Value, accounts: &mut Vec<CopilotAccount>) {
    let Some(map) = value.as_object() else {
        return;
    };
    let mut found_host_entry = false;
    for (key, entry) in map {
        let host = key.split(':').next().unwrap_or_default().trim();
        if host.is_empty() || !host.contains('.') {
            continue;
        }
        let Some(token) = entry
            .get("oauth_token")
            .or_else(|| entry.get("access_token"))
            .or_else(|| entry.get("token"))
            .and_then(Value::as_str)
            .and_then(clean_token)
        else {
            continue;
        };
        found_host_entry = true;
        accounts.push(CopilotAccount {
            host: host.to_string(),
            login: entry
                .get("user")
                .and_then(Value::as_str)
                .and_then(clean_token),
            token,
        });
    }
    if !found_host_entry {
        // Unknown file shape: fall back to the old any-token walk, attributed
        // to github.com like the historical single-account behavior.
        if let Some(token) = find_token_in_value(value) {
            accounts.push(CopilotAccount {
                host: config::DEFAULT_HOST.to_string(),
                login: None,
                token,
            });
        }
    }
}

#[cfg(feature = "quota-sync")]
struct GhHost {
    host: String,
    user: Option<String>,
    token: Option<String>,
}

/// gh's hosts.yml lists every signed-in host (keys at zero indent) with the
/// active `user:` and, for non-keyring setups, an inline `oauth_token:`.
/// Keyring setups still list the host, so the token comes from
/// `gh auth token --hostname <host>`.
#[cfg(feature = "quota-sync")]
fn parse_gh_hosts(raw: &str) -> Vec<GhHost> {
    let mut hosts: Vec<GhHost> = Vec::new();
    for line in raw.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        if indent == 0 {
            if let Some(host) = trimmed.strip_suffix(':') {
                let host = host.trim().trim_matches('"');
                if !host.is_empty() && !host.contains(' ') && host.contains('.') {
                    hosts.push(GhHost {
                        host: host.to_string(),
                        user: None,
                        token: None,
                    });
                }
            }
            continue;
        }
        let Some(current) = hosts.last_mut() else {
            continue;
        };
        // Trailing space keeps "users:" (the nested multi-user block) from
        // matching the "user:" scalar.
        if let Some(value) = yaml_scalar(trimmed, "user: ") {
            current.user.get_or_insert(value);
        } else if let Some(value) = yaml_scalar(trimmed, "oauth_token: ") {
            current.token.get_or_insert(value);
        }
    }
    hosts
}

#[cfg(feature = "quota-sync")]
fn yaml_scalar(line: &str, key: &str) -> Option<String> {
    let value = line
        .strip_prefix(key)?
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(feature = "quota-sync")]
fn gh_cli_accounts() -> Vec<CopilotAccount> {
    let Some(dir) = config::gh_config_dir() else {
        return Vec::new();
    };
    let Ok(raw) = fs::read_to_string(dir.join("hosts.yml")) else {
        // No hosts.yml: preserve the historical bare `gh auth token` probe
        // for the default host.
        return gh_token_for_host(None)
            .map(|token| {
                vec![CopilotAccount {
                    host: config::DEFAULT_HOST.to_string(),
                    login: None,
                    token,
                }]
            })
            .unwrap_or_default();
    };
    parse_gh_hosts(&raw)
        .into_iter()
        .filter_map(|entry| {
            let token = entry
                .token
                .or_else(|| gh_token_for_host(Some(&entry.host)))?;
            Some(CopilotAccount {
                host: entry.host,
                login: entry.user,
                token,
            })
        })
        .collect()
}

#[cfg(feature = "quota-sync")]
fn env_account() -> Option<CopilotAccount> {
    let token = config::ENV_TOKEN_VARS
        .iter()
        .find_map(|var| std::env::var(var).ok().as_deref().and_then(clean_token))?;
    let host = config::ENV_HOST_VARS
        .iter()
        .find_map(|var| {
            std::env::var(var)
                .ok()
                .map(|host| host.trim().to_string())
                .filter(|host| !host.is_empty())
        })
        .unwrap_or_else(|| config::DEFAULT_HOST.to_string());
    Some(CopilotAccount {
        host,
        login: None,
        token,
    })
}

#[cfg(feature = "quota-sync")]
fn dedup_accounts(accounts: Vec<CopilotAccount>) -> Vec<CopilotAccount> {
    let mut seen_tokens: HashSet<(String, String)> = HashSet::new();
    let mut seen_logins: HashSet<(String, String)> = HashSet::new();
    let mut out = Vec::new();
    for account in accounts {
        if !seen_tokens.insert((account.host.clone(), account.token.clone())) {
            continue;
        }
        if let Some(login) = &account.login {
            if !seen_logins.insert((account.host.clone(), login.clone())) {
                continue;
            }
        }
        out.push(account);
    }
    out
}

/// Hard deadline for `gh auth token`; gh can block indefinitely on a locked
/// keyring, and this path runs on the shared background Refresher thread.
#[cfg(feature = "quota-sync")]
const GH_TOKEN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[cfg(feature = "quota-sync")]
fn gh_token_for_host(host: Option<&str>) -> Option<String> {
    // Resolve gh to an absolute path so Finder-launched GUI processes (which
    // inherit launchd's minimal PATH) still find Homebrew/user-local installs.
    let gh = crate::tools::paths::resolve_executable("gh")
        .map(std::path::PathBuf::into_os_string)
        .unwrap_or_else(|| "gh".into());
    let mut command = Command::new(gh);
    command.args(["auth", "token"]);
    if let Some(host) = host {
        command.args(["--hostname", host]);
    }
    let output = run_with_timeout(command, GH_TOKEN_TIMEOUT)?;
    if !output.status.success() {
        return None;
    }
    std::str::from_utf8(&output.stdout)
        .ok()
        .and_then(clean_token)
}

#[cfg(feature = "quota-sync")]
fn run_with_timeout(
    mut command: Command,
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    use std::process::Stdio;

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(feature = "quota-sync")]
fn find_token_in_value(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["oauth_token", "access_token", "token"] {
                if let Some(token) = map.get(key).and_then(Value::as_str).and_then(clean_token) {
                    return Some(token);
                }
            }
            map.values().find_map(find_token_in_value)
        }
        Value::Array(items) => items.iter().find_map(find_token_in_value),
        _ => None,
    }
}

#[cfg(feature = "quota-sync")]
fn clean_token(raw: &str) -> Option<String> {
    let token = raw.trim();
    (!token.is_empty()).then(|| token.to_string())
}

fn percent_remaining_from_balance(snapshot: &QuotaSnapshot, remaining: Option<f64>) -> Option<f64> {
    let entitlement = snapshot.entitlement?;
    if entitlement <= 0.0 {
        return None;
    }
    Some((remaining? / entitlement * 100.0).clamp(0.0, 100.0))
}

fn token_based_billing_enabled(value: Option<&Value>) -> Option<bool> {
    let value = value?;
    value
        .as_bool()
        .or_else(|| value.get("enabled").and_then(Value::as_bool))
}

fn copilot_plan_type(usage: &CopilotUsage) -> Option<String> {
    let normalized = match usage.access_type_sku.as_deref() {
        Some("free_limited_copilot" | "free_educational_quota") => Some("copilot_free"),
        Some("monthly_subscriber_quota") => Some("copilot_pro"),
        Some("plus_monthly_subscriber_quota") => Some("copilot_pro_plus"),
        Some("copilot_standalone_seat_quota" | "copilot_business_seat") => Some("copilot_business"),
        Some("copilot_enterprise_seat_quota" | "copilot_enterprise_seat") => {
            Some("copilot_enterprise")
        }
        _ => None,
    };
    normalized.map(str::to_string).or_else(|| {
        usage.copilot_plan.as_deref().map(|plan| {
            match plan {
                "business" => "copilot_business",
                "enterprise" => "copilot_enterprise",
                "individual_max" => "copilot_pro_plus",
                "individual_edu" => "copilot_education",
                other => other,
            }
            .to_string()
        })
    })
}

fn parse_reset(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()?;
            date.and_hms_opt(0, 0, 0)
                .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
        })
}

fn window_minutes_for(limit_id: &str) -> u64 {
    let id = limit_id.to_ascii_lowercase();
    if id.contains("week") || id.contains("seven_day") || id.contains("7_day") {
        10_080
    } else {
        43_200
    }
}

/// GitHub moved every Copilot plan to AI-credit billing on 2026-06-01. The
/// usage payload kept the legacy `premium_interactions` key for backwards
/// compatibility, but its values are AI-credit units from that date on.
fn credits_era_start() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap()
}

fn human_limit_name(limit_id: &str, credits_era: bool) -> String {
    if credits_era && limit_id.eq_ignore_ascii_case("premium_interactions") {
        return "AI Credits".into();
    }
    let mut words = Vec::new();
    for part in limit_id.split(['_', '-']).filter(|part| !part.is_empty()) {
        let mut chars = part.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        words.push(first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase());
    }
    if words.is_empty() {
        "Copilot".into()
    } else {
        words.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(unix, feature = "quota-sync"))]
    #[test]
    fn run_with_timeout_kills_overrunning_commands() {
        let mut command = Command::new("sleep");
        command.arg("5");
        let started = std::time::Instant::now();

        let output = run_with_timeout(command, std::time::Duration::from_millis(200));

        assert!(output.is_none());
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
    }

    #[cfg(all(unix, feature = "quota-sync"))]
    #[test]
    fn run_with_timeout_returns_fast_command_output() {
        let mut command = Command::new("echo");
        command.arg("ok");

        let output = run_with_timeout(command, std::time::Duration::from_secs(5)).unwrap();

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ok");
    }

    #[test]
    fn enterprise_seat_with_real_quota_keeps_ai_credits_row() {
        // Real enterprise-seat payload shape verified 2026-07-11: a genuine
        // per-seat credit quota plus unlimited chat/completions placeholders.
        let raw = r#"{
          "observed_at": "2026-07-11T17:30:00Z",
          "payload": {
            "copilot_plan": "enterprise",
            "access_type_sku": "copilot_enterprise_seat_quota",
            "quota_reset_date_utc": "2026-08-01T00:00:00.000Z",
            "quota_snapshots": {
              "chat": { "entitlement": 0, "percent_remaining": 100, "unlimited": true, "token_based_billing": true },
              "completions": { "entitlement": 0, "percent_remaining": 100, "unlimited": true, "token_based_billing": true },
              "premium_interactions": {
                "entitlement": 3900,
                "percent_remaining": 99.3,
                "remaining": 3875,
                "quota_remaining": 3875.5,
                "unlimited": false,
                "overage_permitted": true,
                "token_based_billing": true
              }
            }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].limit_id, "premium_interactions");
        assert_eq!(limits[0].limit_name.as_deref(), Some("AI Credits"));
        assert_eq!(limits[0].plan_type.as_deref(), Some("copilot_enterprise"));
        let credits = limits[0].credits.as_ref().unwrap();
        assert_eq!(credits.balance, Some(3875.5));
        assert_eq!(credits.total, Some(3900.0));
        assert_eq!(credits.additional_usage, Some(true));
        let window = limits[0].primary.unwrap();
        assert!((window.used_percent - 0.7).abs() < 1e-9);
        assert_eq!(
            window.resets_at.map(|dt| dt.to_rfc3339()),
            Some("2026-08-01T00:00:00+00:00".to_string())
        );
    }

    #[test]
    fn business_placeholder_payload_emits_org_managed_row() {
        // Business seats return only zero-entitlement placeholders and no
        // reset date — previously this parsed to nothing at all.
        let raw = r#"{
          "observed_at": "2026-07-03T09:00:00Z",
          "payload": {
            "copilot_plan": "business",
            "access_type_sku": "copilot_business_seat",
            "token_based_billing": true,
            "quota_snapshots": {
              "chat": { "entitlement": 0, "percent_remaining": 100, "unlimited": true, "token_based_billing": true },
              "completions": { "entitlement": 0, "percent_remaining": 100, "unlimited": true, "token_based_billing": true },
              "premium_interactions": { "entitlement": 0, "percent_remaining": 100, "unlimited": true, "overage_permitted": true, "token_based_billing": true }
            }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].limit_id, ORG_MANAGED_LIMIT_ID);
        assert_eq!(
            limits[0].limit_name.as_deref(),
            Some(crate::copy::copy().usage.org_managed_credits.as_str())
        );
        assert_eq!(limits[0].plan_type.as_deref(), Some("copilot_business"));
        let window = limits[0].primary.unwrap();
        assert_eq!(window.used_percent, 0.0);
        assert_eq!(window.resets_at, None);
        let credits = limits[0].credits.as_ref().unwrap();
        assert!(credits.unlimited);
        assert_eq!(credits.balance, None);
        assert!(limits[0].observed_at.is_some());
    }

    #[test]
    fn account_labeled_sidecar_suffixes_ids_and_names() {
        let raw = r#"{
          "observed_at": "2026-07-11T17:30:00Z",
          "host": "github.com",
          "login": "R-McKendrick_Node4",
          "payload": {
            "copilot_plan": "enterprise",
            "access_type_sku": "copilot_enterprise_seat_quota",
            "quota_reset_date_utc": "2026-08-01T00:00:00.000Z",
            "quota_snapshots": {
              "premium_interactions": {
                "entitlement": 3900,
                "percent_remaining": 99.3,
                "quota_remaining": 3875.5,
                "unlimited": false,
                "token_based_billing": true
              }
            }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(limits.len(), 1);
        assert_eq!(
            limits[0].limit_id,
            "premium_interactions@github.com/R-McKendrick_Node4"
        );
        assert_eq!(
            limits[0].limit_name.as_deref(),
            Some("AI Credits · R-McKendrick_Node4")
        );
        assert_eq!(limits[0].plan_type.as_deref(), Some("copilot_enterprise"));
    }

    #[test]
    fn any_sidecar_present_detects_per_account_files() {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-copilot-sidecars-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let legacy = dir.join(config::LIMIT_SIDECAR_FILE);

        assert!(!any_sidecar_present(&legacy));

        fs::write(dir.join("copilot-github.com-octocat.json"), "{}").unwrap();
        assert!(any_sidecar_present(&legacy));

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "quota-sync")]
    #[test]
    fn credential_files_yield_host_keyed_accounts() {
        let value: Value = serde_json::from_str(
            r#"{
              "github.com:Iv1.b507a08c87ecfe98": { "user": "octocat", "oauth_token": "gho_personal" },
              "octocorp.ghe.com:Iv1.deadbeef": { "user": "octo_corp", "oauth_token": "gho_work" }
            }"#,
        )
        .unwrap();

        let mut accounts = Vec::new();
        collect_credential_accounts(&value, &mut accounts);
        accounts.sort_by(|a, b| a.host.cmp(&b.host));

        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].host, "github.com");
        assert_eq!(accounts[0].login.as_deref(), Some("octocat"));
        assert_eq!(accounts[0].token, "gho_personal");
        assert_eq!(accounts[1].host, "octocorp.ghe.com");
        assert_eq!(accounts[1].token, "gho_work");
    }

    #[cfg(feature = "quota-sync")]
    #[test]
    fn unknown_credential_shapes_fall_back_to_any_token() {
        let value: Value = serde_json::from_str(r#"{"nested": {"token": "gho_x"}}"#).unwrap();

        let mut accounts = Vec::new();
        collect_credential_accounts(&value, &mut accounts);

        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].host, "github.com");
        assert_eq!(accounts[0].login, None);
        assert_eq!(accounts[0].token, "gho_x");
    }

    #[cfg(feature = "quota-sync")]
    #[test]
    fn gh_hosts_yaml_lists_hosts_users_and_inline_tokens() {
        let raw = "github.com:\n    users:\n        octocat:\n            oauth_token: gho_inline\n    user: octocat\n    oauth_token: gho_inline\n    git_protocol: https\noctocorp.ghe.com:\n    user: worky\n    git_protocol: ssh\n";

        let hosts = parse_gh_hosts(raw);

        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].host, "github.com");
        assert_eq!(hosts[0].user.as_deref(), Some("octocat"));
        assert_eq!(hosts[0].token.as_deref(), Some("gho_inline"));
        assert_eq!(hosts[1].host, "octocorp.ghe.com");
        assert_eq!(hosts[1].user.as_deref(), Some("worky"));
        assert_eq!(hosts[1].token, None);
    }

    #[cfg(feature = "quota-sync")]
    #[test]
    fn dedup_accounts_prefers_the_earliest_source() {
        let accounts = vec![
            CopilotAccount {
                host: "github.com".into(),
                login: Some("octocat".into()),
                token: "gho_a".into(),
            },
            CopilotAccount {
                host: "github.com".into(),
                login: None,
                token: "gho_a".into(),
            },
            CopilotAccount {
                host: "github.com".into(),
                login: Some("octocat".into()),
                token: "gho_b".into(),
            },
            CopilotAccount {
                host: "octocorp.ghe.com".into(),
                login: Some("octocat".into()),
                token: "gho_c".into(),
            },
        ];

        let deduped = dedup_accounts(accounts);

        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].token, "gho_a");
        assert_eq!(deduped[0].login.as_deref(), Some("octocat"));
        assert_eq!(deduped[1].host, "octocorp.ghe.com");
    }

    #[test]
    fn plan_fallbacks_map_seat_and_edu_plans() {
        for (plan, expected) in [
            ("business", "copilot_business"),
            ("enterprise", "copilot_enterprise"),
            ("individual_max", "copilot_pro_plus"),
            ("individual_edu", "copilot_education"),
            ("individual_pro", "individual_pro"),
        ] {
            let raw = format!(
                r#"{{"copilot_plan": "{plan}", "quota_snapshots": {{"premium_interactions": {{"entitlement": 100, "remaining": 50, "unlimited": false}}}}}}"#
            );

            let limits = parse_sidecar_str(&raw, None).unwrap();

            assert_eq!(
                limits[0].plan_type.as_deref(),
                Some(expected),
                "plan {plan}"
            );
        }
    }

    #[test]
    fn parses_quota_snapshots() {
        let raw = r#"{
          "observed_at": "2026-01-15T12:00:00Z",
          "payload": {
            "copilot_plan": "individual_pro",
            "quota_reset_date": "2026-02-01",
            "quota_snapshots": {
              "chat": {
                "entitlement": 0,
                "percent_remaining": 100.0,
                "unlimited": true,
                "timestamp_utc": "2026-01-15T12:01:00Z"
              },
              "premium_interactions": {
                "entitlement": 300,
                "percent_remaining": 31.16,
                "remaining": 93,
                "unlimited": false,
                "timestamp_utc": "2026-01-15T12:02:00Z"
              }
            }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].tool, config::TOOL_ID);
        assert_eq!(limits[0].limit_id, "premium_interactions");
        assert_eq!(
            limits[0].limit_name.as_deref(),
            Some("Premium Interactions")
        );
        assert_eq!(limits[0].plan_type.as_deref(), Some("individual_pro"));
        assert_eq!(limits[0].primary.unwrap().window_minutes, 43_200);
        assert_eq!(limits[0].primary.unwrap().used_percent, 68.84);
        assert_eq!(
            limits[0].credits.as_ref().and_then(|c| c.balance),
            Some(93.0)
        );
    }

    #[test]
    fn labels_premium_interactions_as_ai_credits_after_billing_switch() {
        // Post-2026-06-01 payloads keep the legacy quota key but report
        // AI-credit units, and the reset date moved to quota_reset_date_utc.
        let raw = r#"{
          "observed_at": "2026-07-05T12:00:00Z",
          "payload": {
            "copilot_plan": "individual",
            "access_type_sku": "monthly_subscriber_quota",
            "quota_reset_date_utc": "2026-08-01T00:00:00.000Z",
            "token_based_billing": { "enabled": true },
            "quota_snapshots": {
              "premium_interactions": {
                "entitlement": 1000,
                "percent_remaining": 40.0,
                "remaining": 400,
                "quota_remaining": 399.5,
                "overage_permitted": false,
                "unlimited": false
              }
            }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].limit_id, "premium_interactions");
        assert_eq!(limits[0].limit_name.as_deref(), Some("AI Credits"));
        assert_eq!(limits[0].plan_type.as_deref(), Some("copilot_pro"));
        assert_eq!(
            limits[0]
                .credits
                .as_ref()
                .and_then(|credits| credits.additional_usage),
            Some(false)
        );
        assert_eq!(limits[0].primary.unwrap().used_percent, 60.0);
        assert_eq!(
            limits[0]
                .primary
                .unwrap()
                .resets_at
                .map(|dt| dt.to_rfc3339()),
            Some("2026-08-01T00:00:00+00:00".to_string())
        );
        assert_eq!(
            limits[0].credits.as_ref().and_then(|c| c.balance),
            Some(399.5)
        );
    }

    #[test]
    fn observation_date_alone_switches_to_ai_credits_naming() {
        let raw = r#"{
          "copilot_plan": "individual_pro",
          "quota_reset_date": "2026-08-01",
          "quota_snapshots": {
            "premium_interactions": {
              "entitlement": 1000,
              "remaining": 250,
              "unlimited": false,
              "timestamp_utc": "2026-06-02T09:00:00Z"
            }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].limit_name.as_deref(), Some("AI Credits"));
        assert_eq!(limits[0].primary.unwrap().used_percent, 75.0);
    }

    #[test]
    fn explicit_legacy_billing_overrides_post_switch_observation_date() {
        let raw = r#"{
          "copilot_plan": "individual_pro",
          "quota_reset_date": "2026-08-01",
          "token_based_billing": false,
          "quota_snapshots": {
            "premium_interactions": {
              "entitlement": 1500,
              "remaining": 1200,
              "unlimited": false,
              "timestamp_utc": "2026-07-02T09:00:00Z"
            }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(
            limits[0].limit_name.as_deref(),
            Some("Premium Interactions")
        );
    }

    #[test]
    fn derives_percent_remaining_from_balance_when_needed() {
        let raw = r#"{
          "copilot_plan": "individual_pro",
          "quota_reset_date": "2026-02-01",
          "quota_snapshots": {
            "premium_interactions": {
              "entitlement": 200,
              "remaining": 50,
              "unlimited": false
            }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].primary.unwrap().used_percent, 75.0);
    }
}
