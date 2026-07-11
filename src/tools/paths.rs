use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub fn home() -> Option<PathBuf> {
    dirs::home_dir()
}

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("tokenuse"))
}

pub fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join("tokenuse"))
}

pub fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Resolve a program name to an executable path using the inherited PATH
/// plus well-known install directories. GUI launches (Finder/Dock) inherit
/// launchd's minimal PATH, which misses Homebrew and user-local installs.
pub fn resolve_executable(program: &str) -> Option<PathBuf> {
    resolve_executable_in(
        program,
        std::env::var_os("PATH").as_deref(),
        &extra_executable_dirs(),
    )
}

fn resolve_executable_in(
    program: &str,
    path_var: Option<&OsStr>,
    extra_dirs: &[PathBuf],
) -> Option<PathBuf> {
    let candidate = Path::new(program);
    if candidate.components().count() > 1 && candidate.is_file() {
        return Some(candidate.to_path_buf());
    }

    let path_dirs: Vec<PathBuf> = path_var
        .map(|path| std::env::split_paths(path).collect())
        .unwrap_or_default();
    for dir in &path_dirs {
        if let Some(found) = executable_in_dir(dir, program) {
            return Some(found);
        }
    }
    for dir in extra_dirs {
        if path_dirs.iter().any(|on_path| on_path == dir) {
            continue;
        }
        if let Some(found) = executable_in_dir(dir, program) {
            return Some(found);
        }
    }
    None
}

fn executable_in_dir(dir: &Path, program: &str) -> Option<PathBuf> {
    let candidate = dir.join(program);
    if candidate.is_file() {
        return Some(candidate);
    }
    #[cfg(windows)]
    {
        let candidate = dir.join(format!("{program}.exe"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn extra_executable_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        return Vec::new();
    }
    let mut dirs = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    if let Some(home) = home() {
        dirs.push(home.join(".local/bin"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "tokenuse-paths-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"#!/bin/sh\n").unwrap();
        path
    }

    fn path_var(dirs: &[&Path]) -> std::ffi::OsString {
        std::env::join_paths(dirs).unwrap()
    }

    #[test]
    fn resolves_from_path_before_extra_dirs() {
        let on_path = TempDir::new("onpath");
        let extra = TempDir::new("extra");
        let expected = touch(on_path.path(), "probe");
        touch(extra.path(), "probe");

        let found = resolve_executable_in(
            "probe",
            Some(&path_var(&[on_path.path()])),
            &[extra.path().to_path_buf()],
        );

        assert_eq!(found, Some(expected));
    }

    #[test]
    fn falls_back_to_extra_dirs_when_absent_from_path() {
        let on_path = TempDir::new("onpath");
        let extra = TempDir::new("extra");
        let expected = touch(extra.path(), "probe");

        let found = resolve_executable_in(
            "probe",
            Some(&path_var(&[on_path.path()])),
            &[extra.path().to_path_buf()],
        );

        assert_eq!(found, Some(expected));
    }

    #[test]
    fn falls_back_to_extra_dirs_when_path_is_unset() {
        let extra = TempDir::new("extra");
        let expected = touch(extra.path(), "probe");

        let found = resolve_executable_in("probe", None, &[extra.path().to_path_buf()]);

        assert_eq!(found, Some(expected));
    }

    #[test]
    fn returns_none_when_missing_everywhere() {
        let on_path = TempDir::new("onpath");
        let extra = TempDir::new("extra");

        let found = resolve_executable_in(
            "probe",
            Some(&path_var(&[on_path.path()])),
            &[extra.path().to_path_buf()],
        );

        assert_eq!(found, None);
    }

    #[test]
    fn explicit_paths_pass_through_untouched() {
        let dir = TempDir::new("direct");
        let expected = touch(dir.path(), "probe");
        let raw = expected.to_str().unwrap();

        let found = resolve_executable_in(raw, None, &[]);

        assert_eq!(found, Some(expected));
    }
}
