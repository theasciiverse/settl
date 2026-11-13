//! Where settl keeps its files.
//!
//! Settings, saves and the debug log go to the player's state folder; the
//! downloaded AI models go to the cache folder, since they can always be
//! downloaded again. A `~/.settl` folder from an earlier install is kept and
//! used as before.

use std::path::PathBuf;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

/// `~/.settl`, when an earlier install made it.
fn legacy() -> Option<PathBuf> {
    home().map(|h| h.join(".settl")).filter(|p| p.is_dir())
}

/// Settings, saves and the log: `$XDG_STATE_HOME/settl` or
/// `~/.local/state/settl` on Linux, Application Support on macOS,
/// `%LocalAppData%\settl` on Windows.
pub fn state_dir() -> PathBuf {
    if let Some(dir) = legacy() {
        return dir;
    }
    if cfg!(windows) {
        if let Some(d) = env_dir("LOCALAPPDATA") {
            return d.join("settl");
        }
    } else if cfg!(target_os = "macos") {
        if let Some(h) = home() {
            return h.join("Library/Application Support/settl");
        }
    } else if let Some(d) = env_dir("XDG_STATE_HOME") {
        return d.join("settl");
    } else if let Some(h) = home() {
        return h.join(".local/state/settl");
    }
    PathBuf::from(".settl")
}

/// Downloaded models: `$XDG_CACHE_HOME/settl` or `~/.cache/settl` on Linux,
/// `~/Library/Caches/settl` on macOS, `%LocalAppData%\settl\cache` on Windows.
pub fn cache_dir() -> PathBuf {
    if let Some(dir) = legacy() {
        return dir;
    }
    if cfg!(windows) {
        if let Some(d) = env_dir("LOCALAPPDATA") {
            return d.join("settl").join("cache");
        }
    } else if cfg!(target_os = "macos") {
        if let Some(h) = home() {
            return h.join("Library/Caches/settl");
        }
    } else if let Some(d) = env_dir("XDG_CACHE_HOME") {
        return d.join("settl");
    } else if let Some(h) = home() {
        return h.join(".cache/settl");
    }
    PathBuf::from(".settl")
}
