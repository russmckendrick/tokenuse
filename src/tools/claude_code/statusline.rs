use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde_json::{json, Map, Value};

use super::config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallState {
    NotInstalled,
    InstalledWrapping(String),
    InstalledPassthrough,
    External(String),
}

#[derive(Debug, Clone)]
pub struct DetectionReport {
    pub settings_path: Option<PathBuf>,
    pub settings_exists: bool,
    pub current_command: Option<String>,
    pub wrapper_path: Option<PathBuf>,
    pub wrapper_exists: bool,
    pub wrapper_inner: Option<String>,
    pub state: InstallState,
}

#[derive(Debug, Clone)]
pub struct InstallReport {
    pub wrapper_path: PathBuf,
    pub settings_path: PathBuf,
    pub backup_path: Option<PathBuf>,
    pub previous_inner: Option<String>,
    pub already_installed: bool,
}

pub fn detect() -> Result<DetectionReport> {
    let settings_path = config::settings_path();
    let wrapper_path = config::wrapper_path();
    let wrapper_exists = wrapper_path.as_ref().map(|p| p.exists()).unwrap_or(false);

    let wrapper_inner = if wrapper_exists {
        wrapper_path.as_deref().and_then(read_marker_inner)
    } else {
        None
    };

    let settings_value = match settings_path.as_ref() {
        Some(p) if p.exists() => Some(read_settings_value(p)?),
        _ => None,
    };
    let settings_exists = settings_value.is_some();
    let current_command = settings_value
        .as_ref()
        .and_then(extract_status_line_command);

    let owned = match (current_command.as_deref(), wrapper_path.as_deref()) {
        (Some(cmd), Some(path)) => command_points_at_wrapper(cmd, path),
        _ => false,
    };

    let state = if owned {
        match wrapper_inner.as_deref() {
            Some("") | None => InstallState::InstalledPassthrough,
            Some(inner) => InstallState::InstalledWrapping(inner.to_string()),
        }
    } else {
        match current_command.clone() {
            None => InstallState::NotInstalled,
            Some(cmd) => InstallState::External(cmd),
        }
    };

    Ok(DetectionReport {
        settings_path,
        settings_exists,
        current_command,
        wrapper_path,
        wrapper_exists,
        wrapper_inner,
        state,
    })
}

pub fn install() -> Result<InstallReport> {
    let report = detect()?;
    let settings_path = report
        .settings_path
        .clone()
        .ok_or_else(|| eyre!("Could not resolve ~/.claude/settings.json (no home directory)"))?;
    let wrapper_path = report
        .wrapper_path
        .clone()
        .ok_or_else(|| eyre!("Could not resolve tokenuse statusline wrapper path"))?;

    let already_installed = matches!(
        report.state,
        InstallState::InstalledWrapping(_) | InstallState::InstalledPassthrough
    );

    let inner = match &report.state {
        InstallState::InstalledWrapping(inner) => Some(inner.clone()),
        InstallState::InstalledPassthrough => None,
        InstallState::External(cmd) => Some(cmd.clone()),
        InstallState::NotInstalled => None,
    };

    let limits_file = config::limit_sidecar()
        .ok_or_else(|| eyre!("Could not resolve tokenuse limits sidecar path"))?;
    write_wrapper_file(&wrapper_path, &limits_file, inner.as_deref())?;

    let mut backup_path = None;
    if !already_installed {
        backup_path = Some(write_settings_with_wrapper(&settings_path, &wrapper_path)?);
    }

    Ok(InstallReport {
        wrapper_path,
        settings_path,
        backup_path,
        previous_inner: inner,
        already_installed,
    })
}

pub fn install_manual() -> Result<PathBuf> {
    let wrapper_path = config::wrapper_path()
        .ok_or_else(|| eyre!("Could not resolve tokenuse statusline wrapper path"))?;
    let limits_file = config::limit_sidecar()
        .ok_or_else(|| eyre!("Could not resolve tokenuse limits sidecar path"))?;
    write_wrapper_file(&wrapper_path, &limits_file, None)?;
    Ok(wrapper_path)
}

pub fn uninstall() -> Result<InstallReport> {
    let report = detect()?;
    let settings_path = report
        .settings_path
        .clone()
        .ok_or_else(|| eyre!("Could not resolve ~/.claude/settings.json (no home directory)"))?;
    let wrapper_path = report
        .wrapper_path
        .clone()
        .ok_or_else(|| eyre!("Could not resolve tokenuse statusline wrapper path"))?;

    let was_owned = matches!(
        report.state,
        InstallState::InstalledWrapping(_) | InstallState::InstalledPassthrough
    );

    let mut backup_path = None;
    if was_owned && settings_path.exists() {
        backup_path = Some(restore_settings(
            &settings_path,
            report.wrapper_inner.as_deref(),
        )?);
    }

    if wrapper_path.exists() {
        fs::remove_file(&wrapper_path)
            .wrap_err_with(|| format!("remove {}", wrapper_path.display()))?;
    }

    Ok(InstallReport {
        wrapper_path,
        settings_path,
        backup_path,
        previous_inner: report.wrapper_inner.clone(),
        already_installed: false,
    })
}

fn read_settings_value(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path).wrap_err_with(|| format!("read {}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(&raw).wrap_err_with(|| format!("parse {}", path.display()))
}

fn extract_status_line_command(value: &Value) -> Option<String> {
    let status_line = value.get("statusLine")?;
    if let Some(s) = status_line.as_str() {
        return Some(s.to_string());
    }
    status_line
        .get("command")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn command_points_at_wrapper(command: &str, wrapper: &Path) -> bool {
    let wrapper_str = wrapper.to_string_lossy();
    if command.contains(wrapper_str.as_ref()) {
        return true;
    }
    if let Ok(canon) = wrapper.canonicalize() {
        let canon_str = canon.to_string_lossy();
        if command.contains(canon_str.as_ref()) {
            return true;
        }
    }
    false
}

fn read_marker_inner(wrapper: &Path) -> Option<String> {
    let raw = fs::read_to_string(wrapper).ok()?;
    for line in raw.lines().take(8) {
        if let Some(rest) = line.split_once(config::WRAPPER_MARKER).map(|(_, r)| r) {
            if let Some(start) = rest.find("inner=") {
                let after = &rest[start + "inner=".len()..];
                if let Some(quoted) = after.strip_prefix('\'') {
                    if let Some(end) = quoted.rfind('\'') {
                        let inner = unescape_single_quoted(&quoted[..end]);
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

fn unescape_single_quoted(quoted: &str) -> String {
    quoted.replace("'\\''", "'")
}

fn shell_single_quote(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

fn json_string_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn write_wrapper_file(wrapper_path: &Path, sidecar_path: &Path, inner: Option<&str>) -> Result<()> {
    if let Some(parent) = wrapper_path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    if let Some(parent) = sidecar_path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }

    let body = if cfg!(target_os = "windows") {
        powershell_wrapper(sidecar_path, inner)
    } else {
        bash_wrapper(sidecar_path, inner)
    };

    let tmp_path = wrapper_path.with_extension(format!(
        "{}.tmp",
        wrapper_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
    ));
    fs::write(&tmp_path, body.as_bytes())
        .wrap_err_with(|| format!("write {}", tmp_path.display()))?;
    fs::rename(&tmp_path, wrapper_path).wrap_err_with(|| {
        format!(
            "rename {} -> {}",
            tmp_path.display(),
            wrapper_path.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(wrapper_path)
            .wrap_err_with(|| format!("stat {}", wrapper_path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(wrapper_path, perms)
            .wrap_err_with(|| format!("chmod {}", wrapper_path.display()))?;
    }

    Ok(())
}

fn bash_wrapper(sidecar_path: &Path, inner: Option<&str>) -> String {
    let inner_marker = inner.unwrap_or("");
    let marker = format!(
        "# {} inner={}",
        config::WRAPPER_MARKER,
        shell_single_quote(inner_marker)
    );
    let sidecar_quoted = shell_single_quote(&sidecar_path.to_string_lossy());
    let inner_block = match inner {
        Some(cmd) if !cmd.is_empty() => format!(
            "printf '%s' \"$payload\" | {{ {cmd}; }} || true\n",
            cmd = cmd
        ),
        _ => "printf '%s\\n' 'Claude'\n".to_string(),
    };
    format!(
        "#!/usr/bin/env bash\n{marker}\nset -u\npayload=\"$(cat)\"\nsidecar={sidecar}\nmkdir -p \"$(dirname \"$sidecar\")\"\nprintf '%s' \"$payload\" > \"$sidecar.tmp\" && mv \"$sidecar.tmp\" \"$sidecar\"\n{inner_block}",
        marker = marker,
        sidecar = sidecar_quoted,
        inner_block = inner_block,
    )
}

fn powershell_wrapper(sidecar_path: &Path, inner: Option<&str>) -> String {
    let inner_marker = inner.unwrap_or("");
    let marker = format!(
        "# {} inner={}",
        config::WRAPPER_MARKER,
        shell_single_quote(inner_marker)
    );
    let sidecar_literal = json_string_escape(&sidecar_path.to_string_lossy());
    let inner_block = match inner {
        Some(cmd) if !cmd.is_empty() => {
            let cmd_escaped = json_string_escape(cmd);
            format!("try {{ $payload | & cmd /c {cmd_escaped} }} catch {{ }}\n")
        }
        _ => "Write-Output 'Claude'\n".to_string(),
    };
    format!(
        "{marker}\n$ErrorActionPreference = 'Continue'\n$payload = [Console]::In.ReadToEnd()\n$sidecar = {sidecar}\n$dir = Split-Path -Parent $sidecar\nNew-Item -ItemType Directory -Force -Path $dir | Out-Null\n[System.IO.File]::WriteAllText($sidecar, $payload, (New-Object System.Text.UTF8Encoding $false))\n{inner_block}",
        marker = marker,
        sidecar = sidecar_literal,
        inner_block = inner_block,
    )
}

fn write_settings_with_wrapper(settings_path: &Path, wrapper_path: &Path) -> Result<PathBuf> {
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }

    let mut value = if settings_path.exists() {
        read_settings_value(settings_path)?
    } else {
        Value::Object(Map::new())
    };

    let backup = if settings_path.exists() {
        backup_settings(settings_path)?
    } else {
        settings_path.with_extension("json.bak.new")
    };

    let object = value
        .as_object_mut()
        .ok_or_else(|| eyre!("settings.json must be a JSON object"))?;
    object.insert(
        "statusLine".to_string(),
        json!({
            "type": "command",
            "command": status_line_command(wrapper_path),
        }),
    );

    let pretty =
        serde_json::to_string_pretty(&value).wrap_err("serialize ~/.claude/settings.json")?;
    write_atomic(settings_path, pretty.as_bytes())?;
    Ok(backup)
}

fn restore_settings(settings_path: &Path, inner: Option<&str>) -> Result<PathBuf> {
    let backup = backup_settings(settings_path)?;
    let mut value = read_settings_value(settings_path)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| eyre!("settings.json must be a JSON object"))?;

    match inner {
        Some(cmd) if !cmd.is_empty() => {
            object.insert(
                "statusLine".to_string(),
                json!({ "type": "command", "command": cmd }),
            );
        }
        _ => {
            object.remove("statusLine");
        }
    }

    let pretty =
        serde_json::to_string_pretty(&value).wrap_err("serialize ~/.claude/settings.json")?;
    write_atomic(settings_path, pretty.as_bytes())?;
    Ok(backup)
}

fn backup_settings(settings_path: &Path) -> Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = settings_path.with_extension(format!("json.bak.{stamp}"));
    fs::copy(settings_path, &backup)
        .wrap_err_with(|| format!("backup {} -> {}", settings_path.display(), backup.display()))?;
    Ok(backup)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|s| s.to_str()).unwrap_or("")
    ));
    fs::write(&tmp, bytes).wrap_err_with(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .wrap_err_with(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

fn status_line_command(wrapper_path: &Path) -> String {
    let display = wrapper_path.to_string_lossy().to_string();
    if cfg!(target_os = "windows") {
        format!("powershell -ExecutionPolicy Bypass -File \"{}\"", display)
    } else if display.contains(' ') {
        format!("\"{}\"", display)
    } else {
        display
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn shell_single_quote_handles_embedded_quotes() {
        assert_eq!(shell_single_quote("cship"), "'cship'");
        assert_eq!(shell_single_quote("a b"), "'a b'");
        assert_eq!(shell_single_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_single_quote(""), "''");
    }

    #[test]
    fn read_marker_inner_parses_quoted_inner() {
        let dir = tempdir();
        let wrapper = dir.join("claude-code.sh");
        let body = format!(
            "#!/usr/bin/env bash\n# {} inner='cship --foo'\nset -u\n",
            config::WRAPPER_MARKER
        );
        fs::write(&wrapper, body).unwrap();
        assert_eq!(read_marker_inner(&wrapper), Some("cship --foo".to_string()));
    }

    #[test]
    fn read_marker_inner_parses_escaped_quote() {
        let dir = tempdir();
        let wrapper = dir.join("claude-code.sh");
        let body = format!(
            "#!/usr/bin/env bash\n# {} inner='it'\\''s'\nset -u\n",
            config::WRAPPER_MARKER
        );
        fs::write(&wrapper, body).unwrap();
        assert_eq!(read_marker_inner(&wrapper), Some("it's".to_string()));
    }

    #[test]
    fn read_marker_inner_empty_when_no_inner() {
        let dir = tempdir();
        let wrapper = dir.join("claude-code.sh");
        let body = format!(
            "#!/usr/bin/env bash\n# {} inner=''\nset -u\n",
            config::WRAPPER_MARKER
        );
        fs::write(&wrapper, body).unwrap();
        assert_eq!(read_marker_inner(&wrapper), Some(String::new()));
    }

    #[test]
    fn extract_status_line_handles_object_form() {
        let value: Value =
            serde_json::from_str(r#"{"statusLine": {"type": "command", "command": "cship"}}"#)
                .unwrap();
        assert_eq!(
            extract_status_line_command(&value),
            Some("cship".to_string())
        );
    }

    #[test]
    fn extract_status_line_handles_string_form() {
        let value: Value = serde_json::from_str(r#"{"statusLine": "cship"}"#).unwrap();
        assert_eq!(
            extract_status_line_command(&value),
            Some("cship".to_string())
        );
    }

    #[test]
    fn extract_status_line_returns_none_when_absent() {
        let value: Value = serde_json::from_str(r#"{"theme": "dark"}"#).unwrap();
        assert_eq!(extract_status_line_command(&value), None);
    }

    #[test]
    fn write_settings_preserves_unknown_fields() {
        let dir = tempdir();
        let settings = dir.join("settings.json");
        fs::write(
            &settings,
            r#"{"theme":"dark","permissions":{"allow":["X"]}}"#,
        )
        .unwrap();
        let wrapper = dir.join("wrapper.sh");
        write_settings_with_wrapper(&settings, &wrapper).unwrap();

        let raw = fs::read_to_string(&settings).unwrap();
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.get("theme").and_then(|v| v.as_str()), Some("dark"));
        assert!(parsed.get("permissions").is_some());
        let cmd = parsed
            .pointer("/statusLine/command")
            .and_then(|v| v.as_str())
            .unwrap();
        assert!(cmd.contains("wrapper.sh"));
    }

    #[test]
    fn write_settings_creates_file_when_missing() {
        let dir = tempdir();
        let settings = dir.join("nested").join("settings.json");
        let wrapper = dir.join("wrapper.sh");
        write_settings_with_wrapper(&settings, &wrapper).unwrap();
        assert!(settings.exists());
        let raw = fs::read_to_string(&settings).unwrap();
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert!(parsed
            .pointer("/statusLine/command")
            .and_then(|v| v.as_str())
            .unwrap()
            .contains("wrapper.sh"));
    }

    #[test]
    fn restore_settings_with_inner_replaces_command() {
        let dir = tempdir();
        let settings = dir.join("settings.json");
        fs::write(
            &settings,
            r#"{"statusLine":{"type":"command","command":"/path/to/wrapper.sh"}}"#,
        )
        .unwrap();
        restore_settings(&settings, Some("cship")).unwrap();
        let raw = fs::read_to_string(&settings).unwrap();
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            parsed
                .pointer("/statusLine/command")
                .and_then(|v| v.as_str()),
            Some("cship")
        );
    }

    #[test]
    fn restore_settings_without_inner_removes_status_line() {
        let dir = tempdir();
        let settings = dir.join("settings.json");
        fs::write(
            &settings,
            r#"{"theme":"dark","statusLine":{"type":"command","command":"/wrapper.sh"}}"#,
        )
        .unwrap();
        restore_settings(&settings, None).unwrap();
        let raw = fs::read_to_string(&settings).unwrap();
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert!(parsed.get("statusLine").is_none());
        assert_eq!(parsed.get("theme").and_then(|v| v.as_str()), Some("dark"));
    }

    #[test]
    fn bash_wrapper_embeds_marker_and_inner() {
        let sidecar = PathBuf::from("/tmp/limits/claude-code.json");
        let body = bash_wrapper(&sidecar, Some("cship"));
        assert!(body.contains(config::WRAPPER_MARKER));
        assert!(body.contains("inner='cship'"));
        assert!(body.contains("/tmp/limits/claude-code.json"));
        assert!(body.contains("cship"));
    }

    #[test]
    fn bash_wrapper_falls_back_to_claude() {
        let sidecar = PathBuf::from("/tmp/limits/claude-code.json");
        let body = bash_wrapper(&sidecar, None);
        assert!(body.contains("'Claude'"));
    }

    fn tempdir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("tokenuse-statusline-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
