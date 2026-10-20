//! Independent automatic-hide and fullscreen permissions for the main island.
use super::*;
#[derive(Clone, Copy, PartialEq)]
struct Preferences {
    auto_hide: bool,
    over_fullscreen: bool,
}
impl Service for Preferences {
    fn new() -> Self {
        decode(&std::fs::read_to_string(path()).unwrap_or_default())
    }
    fn listen() {
        let mut previous = None;
        loop {
            let prefs = *Self::read();
            if previous != Some(prefs) {
                let p = path();
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let text = format!(
                    "auto_hide={}\nover_fullscreen={}\n",
                    prefs.auto_hide, prefs.over_fullscreen
                );
                let tmp = p.with_extension("tmp");
                if std::fs::write(&tmp, text)
                    .and_then(|_| std::fs::rename(tmp, p))
                    .is_ok()
                {
                    previous = Some(prefs);
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}
fn path() -> std::path::PathBuf {
    settings::state_dir().join("main-visibility")
}
fn decode(text: &str) -> Preferences {
    let mut p = Preferences {
        auto_hide: true,
        over_fullscreen: false,
    };
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            if let Ok(v) = value.parse::<bool>() {
                match key {
                    "auto_hide" => p.auto_hide = v,
                    "over_fullscreen" => p.over_fullscreen = v,
                    _ => {}
                }
            }
        }
    }
    p
}
pub(super) fn auto_hide() -> bool {
    Preferences::read().auto_hide
}
pub(super) fn over_fullscreen() -> bool {
    Preferences::read().over_fullscreen
}
pub(super) fn toggle_auto_hide() {
    let mut p = Preferences::write();
    p.auto_hide = !p.auto_hide;
}
pub(super) fn toggle_fullscreen() {
    let mut p = Preferences::write();
    p.over_fullscreen = !p.over_fullscreen;
}
pub(super) fn should_tuck(
    forced: bool,
    occupied: bool,
    revealed: bool,
    ready: bool,
    auto: bool,
) -> bool {
    ready && (forced || (auto && occupied && !revealed))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_preserve_existing_behavior() {
        let p = decode("");
        assert!(p.auto_hide);
        assert!(!p.over_fullscreen);
    }
    #[test]
    fn permissions_load_independently() {
        let p = decode("auto_hide=false\nover_fullscreen=true\n");
        assert!(!p.auto_hide);
        assert!(p.over_fullscreen);
    }
    #[test]
    fn ipc_hide_remains_possible_without_auto_hide() {
        assert!(should_tuck(true, true, true, true, false));
        assert!(!should_tuck(false, true, false, true, false));
        assert!(should_tuck(false, true, false, true, true));
        assert!(!should_tuck(true, true, false, false, true));
    }
}
