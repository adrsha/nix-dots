//! Live theme colors from theme-manager; global state is changed only by theme-set.
use amane::Color;
use std::{path::PathBuf, sync::LazyLock};

#[derive(Clone, Copy, PartialEq)]
pub struct Palette {
    pub foreground: Color,
    pub secondary: Color,
    pub blue: Color,
    pub red: Color,
    pub cyan: Color,
    pub yellow: Color,
    pub green: Color,
    pub surface: Color,
    pub raised: Color,
    pub background: Color,
    /// Surface opacity from the theme's top-level `opacity`, within MIN_OPACITY..=1.
    pub opacity: f32,
    /// Default screen-corner radius from the theme's `CORNER_RADIUS`.
    pub corner_radius: f32,
}

const MIN_OPACITY: f32 = 0.3; // a theme must never make the shell invisible
const DEFAULT_RADIUS: f32 = 24.0;
const MAX_RADIUS: f32 = 96.0;

/// First number after `"key":`, tolerating quotes (`"20"`) and bare values (`1`).
fn number(text: &str, key: &str) -> Option<f32> {
    let pattern = format!("\"{key}\"");
    // Only a quoted key followed directly by ':' counts; the same text used as a
    // value (e.g. a theme named "opacity") is skipped.
    text.match_indices(&pattern).find_map(|(at, _)| {
        let tail = text[at + pattern.len()..].trim_start().strip_prefix(':')?;
        let tail = tail.trim_start().trim_start_matches('"');
        let end = tail
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')))
            .unwrap_or(tail.len());
        tail[..end].parse::<f32>().ok().filter(|n| n.is_finite())
    })
}
/// (opacity, corner radius), always in range, with safe defaults when absent or malformed.
fn metrics(text: &str) -> (f32, f32) {
    (
        number(text, "opacity")
            .unwrap_or(1.0)
            .clamp(MIN_OPACITY, 1.0),
        number(text, "CORNER_RADIUS")
            .unwrap_or(DEFAULT_RADIUS)
            .clamp(0.0, MAX_RADIUS),
    )
}

fn load_palette() -> Palette {
    let path = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".local/state/theme/active.json");
    let text = std::fs::read_to_string(path).unwrap_or_default();
    // The theme manager emits flat string-to-hex maps under `colors`.
    let colors = text.split_once("\"colors\"").map_or("", |(_, tail)| tail);
    let colors = colors.split('}').next().unwrap_or("");
    let original = original_background(&text, color(colors, "BASE00", Color::rgb(46, 52, 64)));
    let (opacity, corner_radius) = metrics(&text);
    Palette {
        foreground: color(colors, "WHITE", Color::rgb(216, 222, 233)),
        secondary: color(colors, "LIGHT_GREY", Color::rgb(144, 153, 171)),
        cyan: color(colors, "CYAN", Color::rgb(136, 192, 208)),
        blue: color(colors, "BLUE", Color::rgb(129, 161, 193)),
        red: color(colors, "RED", Color::rgb(191, 97, 106)),
        yellow: color(colors, "YELLOW", Color::rgb(235, 203, 139)),
        green: color(colors, "GREEN", Color::rgb(163, 190, 140)),
        surface: color(colors, "ONE_BG", Color::rgb(59, 66, 82)),
        raised: color(colors, "ONE_BG2", Color::rgb(67, 76, 94)),
        background: original,
        opacity,
        corner_radius,
    }
}

pub static PALETTE: LazyLock<Palette> = LazyLock::new(load_palette);
struct Live {
    palette: Palette,
    name: String,
    amoled: bool,
    global_amoled: bool,
}
impl amane::Service for Live {
    fn new() -> Self {
        Self {
            palette: *PALETTE,
            name: active_name(),
            amoled: local_amoled_state(),
            global_amoled: amoled_state(),
        }
    }
    fn listen() {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let next = load_palette();
            let name = active_name();
            let amoled = local_amoled_state();
            let global_amoled = amoled_state();
            let changed = {
                let state = Self::read();
                state.palette != next
                    || state.name != name
                    || state.amoled != amoled
                    || state.global_amoled != global_amoled
            };
            if changed {
                let mut state = Self::write();
                state.palette = next;
                state.name = name;
                state.amoled = amoled;
                state.global_amoled = global_amoled;
            }
        }
    }
}
fn active_name() -> String {
    let path = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".local/state/theme/active.json");
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.split_once("\"name\"")
        .and_then(|(_, s)| s.split_once(':'))
        .map(|(_, s)| {
            s.trim_start()
                .trim_start_matches('"')
                .split('"')
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_default()
}
pub fn current_name() -> String {
    use amane::Service;
    Live::read().name.clone()
}
pub fn palette() -> Palette {
    use amane::Service;
    Live::read().palette
}

fn color(text: &str, key: &str, fallback: Color) -> Color {
    let Some((_, value)) = text.split_once(&format!("\"{key}\"")) else {
        return fallback;
    };
    let Some((_, value)) = value.split_once(':') else {
        return fallback;
    };
    let value = value.trim_start().trim_start_matches('"');
    let Some(hex) = value.strip_prefix('#').and_then(|s| s.get(..6)) else {
        return fallback;
    };
    let Ok(rgb) = u32::from_str_radix(hex, 16) else {
        return fallback;
    };
    Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_theme_colors_with_whitespace_and_fallback() {
        let input = r##"{"BLUE": "#81A1C1", "WHITE":"#D8DEE9"}"##;
        assert_eq!(
            color(input, "BLUE", Color::BLACK),
            Color::rgb(129, 161, 193)
        );
        assert_eq!(color(input, "MISSING", Color::BLACK), Color::BLACK);
        assert_eq!(
            color(r#"{"BLUE":"oops"}"#, "BLUE", Color::BLACK),
            Color::BLACK
        );
    }
}

fn amoled_state() -> bool {
    let p = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".local/state/theme/amoled");
    std::fs::read_to_string(p).is_ok_and(|s| matches!(s.trim(), "1" | "true" | "on" | "enabled"))
}
pub fn amoled() -> bool {
    use amane::Service;
    Live::read().amoled
}
pub fn background() -> Color {
    if amoled() {
        Color::BLACK
    } else {
        palette().background
    }
}
pub fn opacity() -> f32 {
    palette().opacity
}
pub fn corner_radius() -> f32 {
    palette().corner_radius
}
/// Background for plain surface fills. Keep `background()` for knobs, blends and
/// anything else that must stay opaque.
pub fn surface() -> Color {
    let c = background();
    Color::rgba(
        c.red(),
        c.green(),
        c.blue(),
        (255.0 * opacity()).round() as u8,
    )
}
/// Appends the shared color row (rgb + surface opacity) that goo.wgsl reads last.
/// Every goo shader caller must go through this.
pub fn shader_values(mut values: Vec<[f32; 4]>) -> Vec<[f32; 4]> {
    let c = background();
    values.push([
        c.red() as f32 / 255.0,
        c.green() as f32 / 255.0,
        c.blue() as f32 / 255.0,
        opacity(),
    ]);
    values
}

fn local_path() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state")
        })
        .join("amane/amoled")
}
fn local_amoled_state() -> bool {
    std::fs::read_to_string(local_path()).is_ok_and(|s| s.trim() == "on")
}
pub fn global_amoled() -> bool {
    use amane::Service;
    Live::read().global_amoled
}
pub fn set_amoled(on: bool) -> std::io::Result<()> {
    let p = local_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, if on { "on\n" } else { "off\n" })?;
    std::fs::rename(tmp, p)?;
    use amane::Service;
    Live::write().amoled = on;
    Ok(())
}
fn source_background(text: &str) -> Option<Color> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .find_map(|(k, v)| {
            if k.trim() != "BASE00" {
                return None;
            }
            let v = v.trim().trim_matches('\'').trim_matches('"');
            let hex = v.strip_prefix('#')?.get(..6)?;
            let n = u32::from_str_radix(hex, 16).ok()?;
            Some(Color::rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
        })
}
fn original_background(active: &str, fallback: Color) -> Color {
    // Global AMOLED overwrites BASE00 in active.json. Recover the original theme
    // background without changing global state, running a shell, or resolving imports.
    let name = active
        .split_once("\"name\"")
        .and_then(|(_, s)| s.split_once(':'))
        .map(|(_, s)| {
            s.trim_start()
                .trim_start_matches('"')
                .split('"')
                .next()
                .unwrap_or("")
        })
        .unwrap_or("");
    if name.is_empty() || name.contains('/') || name == ".." {
        return fallback;
    }
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    let source =
        std::fs::read_to_string(config.join("theme-manager/themes").join(name)).unwrap_or_default();
    let dynamic = source.lines().any(|s| {
        s.split_once('=')
            .is_some_and(|(k, v)| k.trim() == "MODE" && v.trim() == "dynamic")
    });
    let cache = local_path()
        .parent()
        .unwrap()
        .join(format!("background-{name}"));
    if !active
        .split_once("\"amoled\"")
        .and_then(|(_, s)| s.split_once(':'))
        .is_some_and(|(_, v)| v.trim_start().starts_with("true"))
    {
        let _ = std::fs::create_dir_all(cache.parent().unwrap());
        let value = fallback.hex();
        if std::fs::read_to_string(&cache).ok().as_deref() != Some(value.as_str()) {
            let _ = std::fs::write(&cache, value);
        }
        return fallback;
    }
    if !dynamic {
        if let Some(c) = source_background(&source) {
            return c;
        }
    }
    // Dynamic themes: preserve the last unmodified resolved wallpaper background.
    std::fs::read_to_string(cache)
        .ok()
        .and_then(|v| source_background(&format!("BASE00={v}")))
        .unwrap_or(fallback)
}

/// Keep labels readable even for palettes with very low-contrast WHITE aliases.
pub fn readable(foreground: Color, background: Color, minimum: f32) -> Color {
    if contrast(foreground, background) >= minimum {
        return foreground;
    }
    let white = Color::rgb(245, 247, 250);
    let black = Color::rgb(17, 20, 26);
    let target = if contrast(white, background) > contrast(black, background) {
        white
    } else {
        black
    };
    for i in 1..=20 {
        let t = i as f32 / 20.0;
        let mix = |a: u8, b: u8| ((a as f32) * (1.0 - t) + b as f32 * t).round() as u8;
        let c = Color::rgb(
            mix(foreground.red(), target.red()),
            mix(foreground.green(), target.green()),
            mix(foreground.blue(), target.blue()),
        );
        if contrast(c, background) >= minimum {
            return c;
        }
    }
    target
}
fn luminance(c: Color) -> f32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.red()) + 0.7152 * f(c.green()) + 0.0722 * f(c.blue())
}
fn contrast(a: Color, b: Color) -> f32 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
#[cfg(test)]
mod background_tests {
    use super::*;
    #[test]
    fn parses_original_background() {
        assert_eq!(
            source_background("VARIANT=dark\nBASE00 = '#282A36'\n"),
            Some(Color::rgb(40, 42, 54))
        );
        assert_eq!(source_background("BASE00=invalid"), None);
    }
    #[test]
    fn text_contrast_works_on_dark_and_light() {
        for bg in [Color::rgb(19, 28, 25), Color::rgb(245, 245, 245)] {
            let fg = readable(Color::rgb(68, 79, 76), bg, 4.5);
            assert!(contrast(fg, bg) >= 4.5);
        }
    }
    #[test]
    fn shader_color_is_a_separate_row() {
        let values = shader_values(vec![[0.0; 4]; 3]);
        assert_eq!(values.len(), 4);
        assert!((MIN_OPACITY..=1.0).contains(&values[3][3]));
    }
    #[test]
    fn reads_opacity_and_radius_from_active_json() {
        let json = r##"{"version":1,"name":"aquarium","opacity":1,"colors":{"BLUE":"#cddbf9"},"values":{"BORDER_SIZE":"0","CORNER_RADIUS":"20"}}"##;
        assert_eq!(metrics(json), (1.0, 20.0));
        assert_eq!(
            metrics(r#"{"opacity":0.85,"values":{"CORNER_RADIUS":12}}"#),
            (0.85, 12.0)
        );
    }
    #[test]
    fn key_text_used_as_a_value_is_ignored() {
        assert_eq!(metrics(r#"{"name":"opacity","opacity":0.5}"#).0, 0.5);
        assert_eq!(metrics(r#"{"name":"opacity"}"#).0, 1.0);
        assert_eq!(metrics(r#"{ "opacity" : 0.6 }"#).0, 0.6);
    }
    #[test]
    fn metrics_are_defensive() {
        assert_eq!(metrics(""), (1.0, DEFAULT_RADIUS));
        assert_eq!(
            metrics(r#"{"opacity":"x","CORNER_RADIUS":"none"}"#),
            (1.0, DEFAULT_RADIUS)
        );
        assert_eq!(
            metrics(r#"{"opacity":0,"CORNER_RADIUS":"-5"}"#),
            (MIN_OPACITY, 0.0)
        );
        assert_eq!(
            metrics(r#"{"opacity":7,"CORNER_RADIUS":"900"}"#),
            (1.0, MAX_RADIUS)
        );
    }
}
