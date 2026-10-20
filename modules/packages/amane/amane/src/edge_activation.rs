//! Persistent per-island edge reveal preferences. IPC activation is independent.
use amane::Service;
use std::{path::PathBuf, time::Duration};
pub const NAMES: [&str; 5] = ["main", "screenshot", "record", "session", "search"];
struct Preferences {
    enabled: [bool; 5],
}
fn path() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state")
        })
        .join("amane/edge-activation")
}
impl Service for Preferences {
    fn new() -> Self {
        let mut enabled = [true; 5];
        if let Ok(text) = std::fs::read_to_string(path()) {
            for line in text.lines() {
                if let Some((name, value)) = line.split_once('=') {
                    if let Some(i) = NAMES.iter().position(|n| *n == name) {
                        enabled[i] = value != "off";
                    }
                }
            }
        }
        Self { enabled }
    }
    fn listen() {
        let mut previous = None;
        loop {
            let values = Self::read().enabled;
            if previous != Some(values) {
                let text = NAMES
                    .iter()
                    .zip(values)
                    .map(|(n, v)| format!("{n}={}\n", if v { "on" } else { "off" }))
                    .collect::<String>();
                let p = path();
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let tmp = p.with_extension("tmp");
                if std::fs::write(&tmp, text)
                    .and_then(|_| std::fs::rename(&tmp, &p))
                    .is_ok()
                {
                    previous = Some(values);
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}
pub fn enabled(name: &str) -> bool {
    NAMES
        .iter()
        .position(|n| *n == name)
        .is_some_and(|i| Preferences::read().enabled[i])
}
pub fn toggle(name: &str) {
    if let Some(i) = NAMES.iter().position(|n| *n == name) {
        let mut s = Preferences::write();
        s.enabled[i] = !s.enabled[i];
    }
}
pub fn command(args: &[String]) -> String {
    if args.is_empty() || args[0] == "status" {
        return NAMES
            .iter()
            .map(|n| format!("{n}: {}", if enabled(n) { "on" } else { "off" }))
            .collect::<Vec<_>>()
            .join("\n");
    }
    let name = match args[0].as_str() {
        "island" => "main",
        "recording" => "record",
        "sessions" => "session",
        n => n,
    };
    let Some(i) = NAMES.iter().position(|n| *n == name) else {
        return "usage: edge main|screenshot|record|session|search on|off|toggle|status".into();
    };
    match args.get(1).map(String::as_str) {
        Some("on" | "enable") => Preferences::write().enabled[i] = true,
        Some("off" | "disable") => Preferences::write().enabled[i] = false,
        Some("toggle") | None => toggle(name),
        Some("status") => {}
        _ => return "usage: edge NAME on|off|toggle|status".into(),
    }
    format!("{name}: {}", if enabled(name) { "on" } else { "off" })
}
