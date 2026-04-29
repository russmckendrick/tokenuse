use std::path::PathBuf;

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
    std::env::var_os(var).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}
