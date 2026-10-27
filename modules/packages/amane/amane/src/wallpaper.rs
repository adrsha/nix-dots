//! Read-only wallpaper state for the settings panel.
//!
//! theme-manager's daemon owns the background processes (awww for stills,
//! mpvpaper for videos) and publishes the resolved wallpaper in `active.json`.
//! Amane only needs to know which file is active so the picker can highlight
//! it; selecting a wallpaper writes the theme file and asks the daemon to
//! regenerate the palette and paint.
use std::fs;
use std::path::PathBuf;

/// The wallpaper the active theme asks for. The theme file is authoritative,
/// including an explicit `WALLPAPER=none`, so a stale snapshot cannot make a
/// cleared wallpaper look active. The snapshot is the fallback (and resolves
/// `WALLPAPER=current`).
pub fn active() -> String {
    let path = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".local/state/theme/active.json");
    let snapshot = fs::read_to_string(path).unwrap_or_default();
    if let Some(name) = json_string(&snapshot, "name") {
        if let Some(value) = theme_wallpaper(&name) {
            if value.eq_ignore_ascii_case("current") {
                return json_string(&snapshot, "WALLPAPER").unwrap_or_default();
            }
            return value;
        }
    }
    json_string(&snapshot, "WALLPAPER").unwrap_or_default()
}

/// Reads a quoted string field from the flat `active.json` snapshot.
fn json_string(text: &str, key: &str) -> Option<String> {
    let (_, tail) = text.split_once(&format!("\"{key}\""))?;
    let (_, value) = tail.split_once(':')?;
    let value = value.trim_start().strip_prefix('"')?;
    Some(value.split('"').next().unwrap_or("").to_string())
}

/// The `WALLPAPER=` value from the named theme file, if it declares one.
fn theme_wallpaper(name: &str) -> Option<String> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    let text = fs::read_to_string(config.join("theme-manager/themes").join(name)).ok()?;
    text.lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "WALLPAPER")
        .map(|(_, value)| value.trim().trim_matches('"').to_string())
}
