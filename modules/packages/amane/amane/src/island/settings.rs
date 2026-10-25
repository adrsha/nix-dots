//! In-island vertical settings, with independent accordion sections.
use super::*;
use amane::{Bluetooth, Network, ScrollArea, TextInput, Widget};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::{fs, path::PathBuf, process::Command};
#[derive(Clone, Copy, PartialEq)]
enum Section {
    History,
    Network,
    Bluetooth,
    Customize,
}
struct Settings {
    open: bool,
    sections: [bool; 4],
    sidebar_labels: bool,
    hovered: Option<usize>,
    page_started: Instant,
    sidebar_width: amane::Animation,
    motion: Spring,
    target_height: f32,
    /// Manual override; `None` follows the theme's CORNER_RADIUS.
    radius: Option<f32>,
    ssid: Option<String>,
    password: String,
    status: String,
    theme_busy: bool,
    themes: Vec<Theme>,
    /// Theme currently opened in the editor.
    editing_theme: Option<String>,
    /// One entry per KEY=VALUE line of the edited theme, in file order.
    editor: Vec<EditorField>,
    /// Media files from the wallpapers directory, sorted by folder.
    wallpapers: Vec<WallItem>,
    /// Position of the wallpaper preview carousel.
    wallpaper_index: usize,
    /// Sidebar category filter; "all" shows every folder.
    wallpaper_filter: String,
}
/// A media file from the wallpapers directory.
#[derive(Clone)]
struct WallItem {
    path: String,
    name: String,
    /// Subfolder inside ~/Pictures/walls ("walls" for loose files).
    category: String,
    /// Video files are shown as thumbnails and painted with mpvpaper.
    video: bool,
}
/// An editable KEY=VALUE line of the current theme-editor session. The input id
/// is leaked once per session so `TextInput::set_text` can seed it and the
/// identity stays stable while the rows rebuild every frame.
#[derive(Clone)]
struct EditorField {
    key: String,
    value: String,
    id: &'static str,
}
#[derive(Clone)]
struct Theme {
    name: String,
    colors: Vec<Color>,
}
impl Service for Settings {
    fn new() -> Self {
        let mut this = Self {
            open: false,
            sections: [true, false, false, false],
            sidebar_labels: false,
            hovered: None,
            page_started: Instant::now(),
            sidebar_width: amane::Animation::new(60.0).duration(Duration::from_millis(300)),
            motion: Spring::new(0.0),
            target_height: 0.0,
            radius: fs::read_to_string(state_dir().join("border-radius"))
                .ok()
                .and_then(|s| s.trim().parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .map(|v| v.clamp(0.0, 96.0)),
            ssid: None,
            password: String::new(),
            status: String::new(),
            theme_busy: false,
            themes: themes(),
            editing_theme: None,
            editor: vec![],
            wallpapers: wallpapers(),
            wallpaper_index: 0,
            wallpaper_filter: "all".into(),
        };
        // Point the carousel at the wallpaper that is currently active.
        this.wallpaper_index = active_wallpaper_index(&this.wallpapers);
        this
    }
    fn interval() -> Duration {
        Duration::from_millis(35)
    }
    fn update(&mut self) -> bool {
        let target = if self.open {
            content_height(self).min(620.0)
        } else {
            0.0
        };
        self.motion.to(target);
        let changed = self.target_height != target;
        self.target_height = target;
        changed
    }
}
pub fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".local/state")
        })
        .join("amane")
}
pub(super) fn radius() -> f32 {
    let manual = Settings::read().radius; // release this lock before taking the theme's
    manual.unwrap_or_else(crate::theme::corner_radius)
}
pub(super) fn open() -> bool {
    Settings::read().open
}
pub(super) fn height() -> f32 {
    Settings::read().motion.precise_value().max(0.0)
}
pub(super) fn keyboard() -> bool {
    let s = Settings::read();
    s.open && s.ssid.is_some() && s.sections[1]
}
pub(super) fn close() {
    let mut s = Settings::write();
    s.open = false;
    s.password.clear();
    s.ssid = None;
    s.motion.to(0.0);
    drop(s);
    TextInput::set_text("amane-wifi-password", "");
}
pub(super) fn toggle() {
    popups::dismiss();
    let mut s = Settings::write();
    s.open = !s.open;
    if s.open {
        let target = content_height(&s).min(620.0);
        s.motion.to(target);
    } else {
        s.motion.to(0.0);
    }
    drop(s);
    let mut i = Island::write();
    i.expanded = true;
    i.leave_deadline = None;
    i.sync();
}
fn index(section: Section) -> usize {
    match section {
        Section::History => 0,
        Section::Network => 1,
        Section::Bluetooth => 2,
        Section::Customize => 3,
    }
}
fn section_toggle(section: Section) {
    let mut s = Settings::write();
    s.sections = [false; 4];
    s.sections[index(section)] = true;
    s.page_started = Instant::now();
    let target = content_height(&s);
    s.motion.to(target);
}
fn content_height(_s: &Settings) -> f32 {
    500.0
}
fn content_width(s: &Settings) -> f32 {
    PANEL.0 - 48.0 - s.sidebar_width.value() - 20.0
}
pub fn command(args: &[String]) -> String {
    match args.first().map(String::as_str) {
        Some("hide") => close(),
        Some("history" | "network" | "bluetooth" | "customization") => {
            if !open() {
                toggle();
            }
            let section = match args[0].as_str() {
                "history" => Section::History,
                "network" => Section::Network,
                "bluetooth" => Section::Bluetooth,
                _ => Section::Customize,
            };
            let mut s = Settings::write();
            s.sections = [false; 4];
            s.sections[index(section)] = true;
            s.page_started = Instant::now();
            let target = content_height(&s).min(620.0);
            s.motion.to(target);
        }
        _ => toggle(),
    }
    "ok".into()
}
fn settings_foreground() -> Color {
    crate::theme::readable(crate::theme::palette().foreground, setting_surface(), 4.5)
}
fn settings_secondary() -> Color {
    crate::theme::readable(crate::theme::palette().secondary, setting_surface(), 4.5)
}
fn label(text: impl Into<String>) -> Text {
    Text::new(text).size(14.0).color(settings_foreground())
}
fn button(text: impl Into<String>, width: f32, action: impl Fn() + 'static) -> Rectangle {
    let text = text.into();
    let key = format!("settings:{text}");
    let ck = key.clone();
    let t = label(&text).weight(Weight::SemiBold);
    let tw = match Widget::width(&t) {
        amane::Size::Fixed(w) => w.min(width - 20.0),
        _ => width - 20.0,
    };
    Rectangle::new()
        .width(width)
        .height(36.0)
        .scale(crate::feedback::scale(&key))
        .radius(12.0)
        .fill(blend_color(
            crate::theme::palette().blue,
            crate::theme::background(),
            0.80,
        ))
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                action()
            }
        })
        .child(
            Rectangle::new()
                .width(tw)
                .height(22.0)
                .align_child(Center, Center)
                .child(t.elide()),
        )
}
fn contents(s: &Settings) -> Column {
    let panel = content_width(s) + 48.0 - 18.0;
    // Drop the history lock before the shared card renderer reads live IDs.
    let history_entries = history::History::read().entries.clone();
    let net = Network::read();
    let bt = Bluetooth::read();
    let mut rows: Vec<Box<dyn Widget>> = vec![];
    if s.sections[0] {
        // Up to 200 cards, each needing text layout and file checks. Building them
        // all every frame is what made opening settings lag, so only the cards
        // near the viewport are real; the rest are same-height spacers.
        let card_width = notices::WIDTH.min(panel - 48.0);
        let offset = settings_scroll::offset("settings-history-page");
        let (near_top, near_bottom) = (offset - 500.0, offset + 1000.0);
        let mut separated: Vec<Box<dyn Widget>> = vec![];
        let mut y = 0.0;
        for (i, entry) in history_entries.iter().rev().enumerate() {
            let height = notices::history_card_height(entry, card_width);
            if i > 0 {
                separated.push(Box::new(
                    Rectangle::new()
                        .width(card_width - 48.0)
                        .height(1.0)
                        .fill(blend_color(
                            crate::theme::palette().blue,
                            Color::BLACK,
                            0.75,
                        )),
                ) as Box<dyn Widget>);
                y += 15.0;
            }
            if y + height >= near_top && y <= near_bottom {
                separated.push(Box::new(notices::history_card(entry, card_width)));
            } else {
                separated.push(Box::new(Rectangle::new().width(card_width).height(height)));
            }
            y += height + 14.0;
        }
        if separated.is_empty() {
            separated.push(Box::new(label("No saved notifications yet.")));
        }
        rows.push(Box::new(
            Column::new(separated).width(panel - 48.0).gap(14.0),
        ));
        rows.push(Box::new(button("Clear history", 140.0, history::clear)));
    }
    if s.sections[1] {
        rows.extend(network_page(s, &net, panel - 48.0));
    }
    if s.sections[2] {
        rows.extend(bluetooth_page(&bt, panel - 48.0));
    }
    if s.sections[3] {
        rows.push(Box::new(
            label("Background").size(17.0).weight(Weight::SemiBold),
        ));
        rows.push(Box::new(amoled_switch(panel - 48.0, false)));
        rows.push(Box::new(amoled_switch(panel - 48.0, true)));

        rows.push(Box::new(section_label(
            "Wallpaper",
            &format!(
                "{} file{} in ~/Pictures/walls",
                s.wallpapers.len(),
                if s.wallpapers.len() == 1 { "" } else { "s" }
            ),
            panel - 48.0,
        )));
        rows.push(Box::new(wallpaper_section(s, panel - 48.0)));

        rows.push(Box::new(
            label("Main island visibility")
                .size(17.0)
                .weight(Weight::SemiBold),
        ));
        rows.push(Box::new(visibility_switch(false, panel - 48.0)));
        rows.push(Box::new(visibility_switch(true, panel - 48.0)));
        let note = label("Independent permissions. IPC can still hide or reveal the island.")
            .size(12.0)
            .color(settings_secondary())
            .wrap()
            .max_lines(2);
        let nh = note.height_in(panel - 48.0);
        rows.push(Box::new(
            Rectangle::new().width(panel - 48.0).height(nh).child(note),
        ));

        rows.push(Box::new(
            label("Edge activation").size(17.0).weight(Weight::SemiBold),
        ));
        rows.push(Box::new(
            label("Reveal each island by touching its screen edge.")
                .size(13.0)
                .color(settings_secondary()),
        ));
        for name in crate::edge_activation::NAMES {
            rows.push(Box::new(edge_switch(name, panel - 48.0)));
        }
        rows.push(Box::new(
            label("IPC activation always stays available.")
                .size(12.0)
                .color(crate::theme::palette().cyan),
        ));

        let radius = s.radius.unwrap_or_else(crate::theme::corner_radius);
        let preview = Rectangle::new()
            .width(106.0)
            .height(70.0)
            .radius((radius * 0.6).min(32.0))
            .border(4.0, crate::theme::palette().blue)
            .fill(crate::theme::palette().background)
            .align_child(Center, Center)
            .child(
                label(format!("{} px", radius as u32))
                    .size(20.0)
                    .weight(Weight::SemiBold),
            );
        let controls = Column::new(children![
            label("Screen curvature")
                .size(16.0)
                .weight(Weight::SemiBold),
            Row::new(children![
                button("−", 40.0, || change_radius(-4.0)),
                button("+", 40.0, || change_radius(4.0)),
                button("Reset", 68.0, reset_radius)
            ])
            .gap(8.0)
        ])
        .gap(10.0);
        rows.push(Box::new(
            Rectangle::new()
                .width(panel - 48.0)
                .height(100.0)
                .padding(12.0)
                .radius(16.0)
                .fill(setting_surface())
                .child(
                    Row::new(children![preview, controls])
                        .gap(18.0)
                        .align(Center),
                ),
        ));
        rows.push(Box::new(
            label("Theme library").size(17.0).weight(Weight::SemiBold),
        ));
        let current = crate::theme::current_name();
        rows.push(Box::new(
            label(if s.theme_busy {
                s.status.clone()
            } else {
                format!("Active theme · {current}")
            })
            .size(13.0)
            .color(crate::theme::palette().cyan),
        ));
        rows.push(Box::new(
            label("Click a card to apply it, right-click to edit it.")
                .size(12.0)
                .color(settings_secondary()),
        ));
        rows.extend(theme_editor_rows(s, panel - 48.0));
        let mut cards = vec![];
        for chunk in s.themes.chunks(2) {
            let mut tiles = vec![];
            for theme in chunk {
                let name = theme.name.clone();
                let key = format!("theme:{name}");
                let ck = key.clone();
                let edit_name = name.clone();
                let swatches = theme
                    .colors
                    .iter()
                    .map(|c| {
                        Box::new(
                            Rectangle::new()
                                .width(30.0)
                                .height(22.0)
                                .radius(5.0)
                                .fill(*c),
                        ) as Box<dyn Widget>
                    })
                    .collect();
                tiles.push(Box::new(
                    Rectangle::new()
                        .width((panel - 60.0) / 2.0)
                        .height(88.0)
                        .scale(crate::feedback::scale(&key))
                        .padding(12.0)
                        .radius(14.0)
                        .fill(if theme.name == current {
                            blend_color(
                                crate::theme::palette().blue,
                                crate::theme::background(),
                                0.78,
                            )
                        } else {
                            setting_surface()
                        })
                        .cursor(Pointer)
                        .on_click(move |b| {
                            if b == Button::Left {
                                crate::feedback::click(&ck);
                                apply_theme(name.clone())
                            } else if b == Button::Right {
                                crate::feedback::click(&ck);
                                edit_theme(edit_name.clone())
                            }
                        })
                        .child(
                            Column::new(children![
                                Row::new(swatches).gap(5.0),
                                label(if theme.name == current {
                                    format!("●  {}", theme.name)
                                } else {
                                    theme.name.clone()
                                })
                                .weight(Weight::SemiBold)
                            ])
                            .gap(12.0),
                        ),
                ) as Box<dyn Widget>);
            }
            cards.push(Box::new(Row::new(tiles).gap(12.0)) as Box<dyn Widget>);
        }
        rows.push(Box::new(Column::new(cards).gap(10.0)));
    }
    if !s.status.is_empty() {
        let t = label(&s.status)
            .size(13.0)
            .color(settings_secondary())
            .wrap()
            .max_lines(3);
        let h = t.height_in(panel - 48.0);
        rows.push(Box::new(
            Rectangle::new().width(panel - 48.0).height(h).child(t),
        ));
    }
    Column::new(rows).width(panel - 48.0).gap(12.0)
}
fn connect_selected() {
    let (ssid, password) = {
        let mut s = Settings::write();
        let values = (s.ssid.clone(), std::mem::take(&mut s.password));
        s.status = "Connection requested. NetworkManager status updates automatically.".into();
        s.ssid = None;
        values
    };
    TextInput::set_text("amane-wifi-password", "");
    if let Some(ssid) = ssid {
        Network::connect(
            &ssid,
            if password.is_empty() {
                None
            } else {
                Some(&password)
            },
        );
    }
}
fn change_radius(delta: f32) {
    let value = radius() + delta;
    set_radius(value);
}
fn set_radius(value: f32) {
    let value = if value.is_finite() {
        value.clamp(0.0, 96.0)
    } else {
        return;
    };
    Settings::write().radius = Some(value);
    persist_radius();
}
/// Drop the manual override so the radius follows the active theme again.
fn reset_radius() {
    Settings::write().radius = None;
    persist_radius();
}
static RADIUS_FILE: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// Saves whatever the radius override is *when this runs*, one writer at a time, so
/// overlapping saves (e.g. `+` then Reset) always converge on the latest state.
fn persist_radius() {
    std::thread::spawn(|| {
        let _guard = RADIUS_FILE.lock().unwrap_or_else(|e| e.into_inner());
        let current = Settings::read().radius;
        let dir = state_dir();
        let path = dir.join("border-radius");
        match current {
            Some(value) => {
                let _ = fs::create_dir_all(&dir);
                let tmp = dir.join("border-radius.tmp");
                if fs::write(&tmp, format!("{value}\n")).is_ok() {
                    let _ = fs::rename(tmp, path);
                }
            }
            None => {
                let _ = fs::remove_file(path);
            }
        }
    });
}
fn themes() -> Vec<Theme> {
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
        .join("theme-manager/themes");
    let mut themes = vec![];
    if let Ok(list) = fs::read_dir(dir) {
        for entry in list.flatten() {
            if !entry
                .file_type()
                .is_ok_and(|t| t.is_file() || t.is_symlink())
            {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(text) = fs::read_to_string(entry.path()) else {
                continue;
            };
            let colors = ["BASE00", "BLUE", "GREEN", "PURPLE", "YELLOW"]
                .iter()
                .filter_map(|key| {
                    text.lines()
                        .filter_map(|l| l.split_once('='))
                        .find(|(k, _)| k.trim() == *key)
                        .and_then(|(_, v)| parse_hex(v.trim()))
                })
                .collect();
            themes.push(Theme { name, colors });
        }
    }
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    themes
}
fn parse_hex(s: &str) -> Option<Color> {
    let rgb = u32::from_str_radix(s.strip_prefix('#')?.get(..6)?, 16).ok()?;
    Some(Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8))
}
fn setting_surface() -> Color {
    blend_color(
        crate::theme::palette().blue,
        crate::theme::background(),
        0.94,
    )
}
fn switch_track(on: bool, accent: Color) -> Rectangle {
    Rectangle::new()
        .width(40.0)
        .height(24.0)
        .radius(Full)
        .fill(if on {
            accent
        } else {
            blend_color(accent, crate::theme::background(), 0.78)
        })
        .child(
            Rectangle::new()
                .width(18.0)
                .height(18.0)
                .radius(Full)
                .translate(if on { 19.0 } else { 3.0 }, 3.0)
                .fill(if on {
                    crate::theme::background()
                } else {
                    accent
                }),
        )
}
fn toggle_row(
    glyph: &str,
    title: &str,
    detail: &str,
    width: f32,
    on: bool,
    accent: Color,
    action: impl Fn() + 'static,
) -> Rectangle {
    let accent = crate::theme::readable(accent, setting_surface(), 3.0);
    let tw = (width - 112.0).max(40.0);
    let subtitle = label(detail)
        .size(12.0)
        .color(settings_secondary())
        .wrap()
        .max_lines(2);
    let sh = subtitle.height_in(tw);
    Rectangle::new()
        .width(width)
        .height((sh + 34.0).max(66.0))
        .padding(10.0)
        .radius(14.0)
        .fill(setting_surface())
        .cursor(Pointer)
        .scale(crate::feedback::scale(&format!("switch:{title}")))
        .on_click({
            let key = format!("switch:{title}");
            move |b| {
                if b == Button::Left {
                    crate::feedback::click(&key);
                    action()
                }
            }
        })
        .align_child(Start, Center)
        .child(
            Row::new(children![
                Text::new(glyph)
                    .font("Material Icons Round")
                    .size(24.0)
                    .tight()
                    .color(accent),
                Column::new(children![
                    label(title).size(14.0).weight(Weight::SemiBold),
                    Rectangle::new().width(tw).height(sh).child(subtitle)
                ])
                .width(tw)
                .gap(4.0),
                switch_track(on, accent)
            ])
            .gap(12.0)
            .align(Center),
        )
}
fn amoled_switch(width: f32, global: bool) -> Rectangle {
    let on = if global {
        crate::theme::global_amoled()
    } else {
        crate::theme::amoled()
    };
    toggle_row(
        "\u{e3a7}",
        if global {
            "System AMOLED"
        } else {
            "Amane AMOLED"
        },
        if global {
            "Theme manager · other applications"
        } else {
            "This shell only · independent of system themes"
        },
        width,
        on,
        crate::theme::palette().cyan,
        move || {
            if global {
                set_amoled(!on)
            } else {
                match crate::theme::set_amoled(!on) {
                    Ok(()) => {}
                    Err(e) => Settings::write().status = format!("Cannot save Amane AMOLED: {e}"),
                }
            }
        },
    )
}
fn set_amoled(enabled: bool) {
    {
        let mut s = Settings::write();
        if s.theme_busy {
            return;
        }
        s.theme_busy = true;
        s.status = "Updating AMOLED mode…".into();
    }
    std::thread::spawn(move || {
        let output = Command::new("theme-set")
            .args(["--amoled", if enabled { "on" } else { "off" }])
            .output();
        let result = match output {
            Ok(o) if o.status.success() => format!(
                "AMOLED {}. Theme manager will reload the palette.",
                if enabled { "enabled" } else { "disabled" }
            ),
            Ok(o) => format!(
                "AMOLED update failed: {}",
                String::from_utf8_lossy(&o.stderr)
            ),
            Err(e) => format!("Cannot launch theme-set: {e}"),
        };
        let mut s = Settings::write();
        s.theme_busy = false;
        s.status = result;
    });
}
fn apply_theme(name: String) {
    {
        let mut state = Settings::write();
        if state.theme_busy {
            return;
        }
        state.theme_busy = true;
        state.status = format!("Applying {name}…");
    }
    std::thread::spawn(move || {
        let output = Command::new("theme-set").arg(&name).output();
        let result = match output {
            Ok(o) if o.status.success() => {
                let mut confirmed = false;
                for _ in 0..50 {
                    if crate::theme::current_name() == name {
                        confirmed = true;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                if confirmed {
                    format!("Theme {name} is active.")
                } else {
                    format!(
                        "Theme {name} was requested, but theme-daemon has not applied it. Check the theme-daemon user service."
                    )
                }
            }
            Ok(o) => format!(
                "theme-set failed: {}",
                String::from_utf8_lossy(&o.stderr)
                    .chars()
                    .take(240)
                    .collect::<String>()
            ),
            Err(e) => format!("Cannot launch theme-set: {e}"),
        };
        let mut state = Settings::write();
        state.theme_busy = false;
        state.status = result;
    });
}

fn theme_file(name: &str) -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    config.join("theme-manager/themes").join(name)
}

/// Every location the theme file should be written to: the theme-set-visible
/// path (`~/.config/theme-manager/themes`) plus the live file behind the
/// symlink when they differ. Duplicates single-target setups.
fn theme_write_targets(name: &str) -> Vec<PathBuf> {
    let direct = theme_file(name);
    let mut targets = vec![direct.clone()];
    if let Ok(canonical) = fs::canonicalize(&direct) {
        if canonical != direct {
            targets.push(canonical);
        }
    }
    targets
}

// ---------------------------------------------------------------- theme editor

fn group_name(key: &str) -> Option<&'static str> {
    match key {
        "VARIANT" | "MODE" | "OPACITY" => Some("General"),
        "BASE00" => Some("Base 16"),
        "WHITE" => Some("Base 30"),
        "CORNER_RADIUS" => Some("Shaping"),
        _ => None,
    }
}

fn edit_theme(name: String) {
    let text = fs::read_to_string(theme_file(&name)).unwrap_or_default();
    let mut fields = vec![];
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let (key, value) = line.split_once('=').unwrap();
        let key = key.trim().to_string();
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }
        let id: &'static str = Box::leak(
            format!("theme-edit:{name}::{key}").into_boxed_str(),
        );
        fields.push(EditorField {
            key,
            id,
            value: value.trim().to_string(),
        });
    }
    let mut s = Settings::write();
    for f in &s.editor {
        TextInput::set_text(f.id, "");
    }
    for f in &fields {
        TextInput::set_text(f.id, &f.value);
    }
    s.editor = fields;
    s.editing_theme = Some(name);
}

fn close_editor() {
    let mut s = Settings::write();
    for f in &s.editor {
        TextInput::set_text(f.id, "");
    }
    s.editor.clear();
    s.editing_theme = None;
}

/// Rewrites the theme file with the editor's values. Preserves comments and
/// layout by replacing the right-hand side of each known KEY= line; keys that
/// the original file lacked are appended. Falls back to a pkexec relink
/// (password dialog) when the target lives in the read-only Nix store.
fn save_theme_edits(apply: bool) {
    let (name, fields) = {
        let mut s = Settings::write();
        if s.theme_busy {
            return;
        }
        let Some(name) = s.editing_theme.clone() else {
            return;
        };
        s.theme_busy = true;
        s.status = format!("Saving {name}…");
        (name, s.editor.clone())
    };
    std::thread::spawn(move || {
        let file = theme_file(&name);
        let original = fs::read_to_string(&file).unwrap_or_default();
        let pairs: Vec<(String, String)> = fields
            .iter()
            .map(|f| (f.key.clone(), f.value.clone()))
            .collect();
        let out = merge_theme_values(&original, &pairs);
        // Write both candidates at once: the theme-set-visible path and the
        // live file behind the symlink, so an edit never depends on the link.
        let targets = theme_write_targets(&name);
        let mut saved: Vec<String> = vec![];
        let mut errors: Vec<String> = vec![];
        for target in targets {
            match fs::write(&target, &out) {
                Ok(()) => saved.push(target.display().to_string().chars().take(60).collect()),
                Err(e) => errors.push(format!("{} ({e})", target.display())),
            }
        }
        let result = if saved.is_empty() {
            // The Nix store copy cannot be written directly; point the user
            // at the live-folder link and the relink button instead.
            let tmp = state_dir().join("theme-edit.tmp");
            let staged = fs::create_dir_all(tmp.parent().unwrap())
                .and_then(|_| fs::write(&tmp, &out))
                .is_ok();
            if staged {
                format!(
                    "Theme files are read-only ({}); the draft went to {} — link themesDir to a live folder in theme_manager.nix, or use Relink.",
                    errors.join("; "),
                    tmp.display()
                )
            } else {
                format!("Save failed: {}", errors.join("; "))
            }
        } else {
            let mut s = Settings::write();
            s.themes = themes();
            if errors.is_empty() {
                format!("Saved to {} location(s).", saved.len())
            } else {
                format!(
                    "Saved to {} location(s); failed: {}",
                    saved.len(),
                    errors.join("; ")
                )
            }
        };
        let mut s = Settings::write();
        s.theme_busy = false;
        s.status = result;
        drop(s);
        if apply {
            apply_theme(name);
        }
    });
}

/// Rebuilds a theme file from the original text with edited values: the
/// KEY=VALUE line layout, values and comments survive; unknown keys append.
fn merge_theme_values(original: &str, fields: &[(String, String)]) -> String {
    let mut used = vec![false; fields.len()];
    let mut out = String::new();
    for line in original.lines() {
        if line.trim_start().starts_with('#') || !line.contains('=') {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let Some((prefix, value)) = line.split_once('=') else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let _ = value;
        let key = prefix.trim();
        let found = fields
            .iter()
            .zip(&used)
            .position(|((key2, _), taken)| !*taken && key2 == key);
        if let Some(i) = found {
            used[i] = true;
            out.push_str(prefix);
            out.push('=');
            out.push_str(fields[i].1.trim());
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    for ((key, value), taken) in fields.iter().zip(&used) {
        if !*taken && !key.is_empty() {
            out.push_str(&format!("{key}={}\n", value.trim()));
        }
    }
    out
}

/// Runs `pkexec nixos-rebuild switch` so a theme directory held by the Nix
/// store gets relinked; polkit raises the password dialog in the session.
fn relink_nix() {
    let mut s = Settings::write();
    if s.theme_busy {
        return;
    }
    s.theme_busy = true;
    s.status = "Password dialog open — rebuilding NixOS to relink themes…".into();
    std::thread::spawn(|| {
        let home = std::env::var("HOME").unwrap_or_default();
        let script = format!("exec nixos-rebuild switch --flake {home}/flakes#$(hostname)");
        let output = Command::new("pkexec").args(["sh", "-c", &script]).output();
        let result = match output {
            Ok(o) if o.status.success() => "Nix relink complete.".into(),
            Ok(o) => format!(
                "Relink failed: {}",
                String::from_utf8_lossy(&o.stderr)
                    .chars()
                    .take(240)
                    .collect::<String>()
            ),
            Err(e) => format!(
                "pkexec could not start ({e}); a polkit agent must run inside the session."
            ),
        };
        let mut s = Settings::write();
        s.theme_busy = false;
        s.status = result;
    });
}

fn theme_editor_rows(s: &Settings, width: f32) -> Vec<Box<dyn Widget>> {
    let mut rows: Vec<Box<dyn Widget>> = vec![];
    if let Some(editing) = &s.editing_theme {
        rows.push(Box::new(
            label(format!("Editing · {editing}"))
                .size(17.0)
                .weight(Weight::SemiBold),
        ));
        rows.push(Box::new(
            Row::new(children![
                button("Cancel", 84.0, close_editor),
                button("Save", 84.0, || save_theme_edits(false)),
                button("Save · Apply", 130.0, || save_theme_edits(true)),
                button("Relink (Nix)", 130.0, relink_nix),
            ])
            .gap(8.0),
        ));
        let hint = label(
            "Saving writes the theme file directly. Relink runs nixos-rebuild with a password prompt for store-linked themes.",
        )
        .size(12.0)
        .color(settings_secondary())
        .wrap()
        .max_lines(2);
        let hint_height = hint.height_in(width);
        rows.push(Box::new(
            Rectangle::new()
                .width(width)
                .height(hint_height)
                .child(hint),
        ));
    }
    for (i, field) in s.editor.iter().enumerate() {
        if let Some(group) = group_name(&field.key) {
            rows.push(Box::new(label(group).size(15.0).weight(Weight::SemiBold)));
        }
        rows.push(Box::new(editor_row(s, i, width)));
    }
    rows
}

fn editor_row(s: &Settings, index: usize, width: f32) -> Rectangle {
    let field = &s.editor[index];
    let color = parse_hex(&field.value);
    let id = field.id;
    Rectangle::new()
        .width(width)
        .height(40.0)
        .padding(6.0)
        .radius(10.0)
        .fill(setting_surface())
        .align_child(Start, Center)
        .child(
            Row::new(children![
                Rectangle::new()
                    .width(28.0)
                    .height(20.0)
                    .radius(5.0)
                    .fill(color.unwrap_or(crate::theme::palette().red))
                    .border(if color.is_some() { 0.0 } else { 1.0 }, crate::theme::palette().red),
                label(&field.key).size(12.0),
                Rectangle::new()
                    .width(150.0)
                    .height(26.0)
                    .child(
                        TextInput::new(id)
                            .size(13.0)
                            .color(settings_foreground())
                            .on_change(move |text| {
                                let mut s = Settings::write();
                                if let Some(f) = s.editor.get_mut(index) {
                                    f.value = text;
                                }
                            }),
                    ),
            ])
            .gap(10.0),
        )
}

// ------------------------------------------------------------------ wallpapers

/// Recursively (two levels) lists media files under ~/Pictures/walls, skipping
/// theme-manager's own generated files. Category is the subfolder name.
fn wallpapers() -> Vec<WallItem> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let root = home.join("Pictures/walls");
    let mut found = vec![];
    let mut stack = vec![(root.clone(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(list) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in list.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if name.starts_with("current") || name.starts_with("readme") || name.starts_with("README")
            {
                continue;
            }
            if path.is_dir() {
                if depth < 2 {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            let Some(video) = wall_kind(&name) else {
                continue;
            };
            let category = dir
                .file_name()
                .filter(|d| *d != root.file_name().unwrap_or_default())
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or_else(|| "walls".into());
            found.push(WallItem {
                path: path.to_string_lossy().into_owned(),
                name,
                category,
                video,
            });
        }
    }
    found.sort_by(|a, b| a.category.cmp(&b.category).then(a.path.cmp(&b.path)));
    found.truncate(60);
    found
}

/// (is a video file?) based on the extension; `None` means unsupported.
fn wall_kind(name: &str) -> Option<bool> {
    let ext = name.rsplit('.').next()?.to_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "png" => false,
        "webp" | "gif" | "bmp" | "tga" | "tiff" | "avif" => false,
        "mp4" | "m4v" | "webm" | "mkv" | "mov" | "avi" | "mpg" | "mpeg" => true,
        _ => return None,
    })
}

/// Files the shell decodes natively — everything else needs a generated thumb.
fn wall_native(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "jpg" | "jpeg" | "png")
}

/// Cached ffmpeg/imagemagick thumbnails: source path → displayable png.
static WALL_THUMBS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
static WALL_THUMB_QUEUED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// The image the shell can show for `item`: natives pass through, webp/gif/
/// videos use a generated (cached) thumbnail; queued generations start here.
fn wall_display(item: &WallItem) -> String {
    if !item.video && wall_native(&item.name) {
        return item.path.clone();
    }
    let path = item.path.clone();
    if let Some(thumb) = WALL_THUMBS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|m| m.get(&path).cloned())
    {
        return thumb;
    }
    let queued = WALL_THUMB_QUEUED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock();
    let Ok(pending_guard) = queued else {
        return String::new();
    };
    if pending_guard.contains(&path) {
        return String::new();
    }
    drop(pending_guard);
    let mut pending = WALL_THUMB_QUEUED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap();
    if pending.insert(path.clone()) {
        let item = item.clone();
        std::thread::spawn(move || {
            let thumb = generate_wall_thumb(&item);
            let mut map = WALL_THUMBS
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
                .unwrap();
            if let Some(thumb) = thumb {
                map.insert(item.path.clone(), thumb);
            }
            WALL_THUMB_QUEUED
                .get_or_init(|| Mutex::new(HashSet::new()))
                .lock()
                .unwrap()
                .remove(&item.path);
        });
    }
    String::new()
}

/// Generates (or finds cached) a png snapshot for non-native media.
fn generate_wall_thumb(item: &WallItem) -> Option<String> {
    let dst = wall_thumb_path(&item.path)?;
    if !dst.exists() {
        let ok = if item.video {
            match ffmpeg_bin() {
                Some(ffmpeg) => Command::new(ffmpeg)
                    .args(["-y", "-v", "error", "-ss", "1", "-i", &item.path, "-frames:v", "1"])
                    .arg(&dst)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false),
                None => false,
            }
        } else {
            match magick_bin() {
                Some(magick) => Command::new(magick)
                    .arg(format!("{}[0]", item.path))
                    .arg(&dst)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false),
                None => match ffmpeg_bin() {
                    Some(ffmpeg) => Command::new(ffmpeg)
                        .args(["-y", "-v", "error", "-i", &item.path, "-frames:v", "1"])
                        .arg(&dst)
                        .stdin(std::process::Stdio::null())
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false),
                    None => false,
                },
            }
        };
        if !ok {
            return None;
        }
    }
    Some(dst.to_string_lossy().into_owned())
}

/// Stable cache path for a source file's thumbnail.
fn wall_thumb_path(src: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let dir = state_dir().join("wall-thumbs");
    let _ = fs::create_dir_all(&dir);
    let name = std::path::Path::new(src)
        .file_stem()?
        .to_string_lossy()
        .into_owned();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut hash);
    Some(dir.join(format!("{}-{:08x}.png", name, hash.finish())))
}

static FFMPEG_BIN: OnceLock<Option<PathBuf>> = OnceLock::new();
static MAGICK_BIN: OnceLock<Option<PathBuf>> = OnceLock::new();

/// ffmpeg from PATH, else a store checkout found by glob.
fn ffmpeg_bin() -> Option<PathBuf> {
    FFMPEG_BIN
        .get_or_init(|| resolve_bin("ffmpeg").or_else(|| store_bin("ffmpeg", "bin/ffmpeg")))
        .clone()
}

/// imagemagick from PATH, else a store checkout found by glob.
fn magick_bin() -> Option<PathBuf> {
    MAGICK_BIN
        .get_or_init(|| resolve_bin("magick").or_else(|| store_bin("imagemagick", "bin/magick")))
        .clone()
}

/// A plain PATH lookup that also tolerates a missing binary.
fn resolve_bin(name: &str) -> Option<PathBuf> {
    let probed = Command::new(name).arg("-version").output();
    if probed.map(|o| o.status.success()).unwrap_or(false) {
        return Some(PathBuf::from(name));
    }
    None
}

fn store_bin(prefix: &str, tail: &str) -> Option<PathBuf> {
    let store = PathBuf::from("/nix/store");
    let mut candidates: Vec<PathBuf> = fs::read_dir(&store)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with(prefix))
                .unwrap_or(false)
        })
        .collect();
    candidates.sort();
    candidates
        .into_iter()
        .rev()
        .map(|p| p.join(tail))
        .find(|bin| bin.exists())
}

/// The raw WALLPAPER value recorded by the theme daemon ("none" until a wall
/// has been applied or a theme declares one).
fn active_wallpaper() -> String {
    let path = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".local/state/theme/active.json");
    let text = fs::read_to_string(path).unwrap_or_default();
    text.split_once("\"WALLPAPER\"")
        .and_then(|(_, tail)| tail.split_once(':'))
        .map(|(_, v)| {
            v.trim_start()
                .trim_start_matches('"')
                .split('"')
                .next()
                .unwrap_or("")
        })
        .unwrap_or("")
        .to_string()
}

/// Carousel start position; the slide that matches the applied wallpaper.
fn active_wallpaper_index(walls: &[WallItem]) -> usize {
    let active = active_wallpaper();
    if active.is_empty() || active == "none" {
        return 0;
    }
    walls
        .iter()
        .position(|w| w.path == active)
        .or_else(|| {
            let stem = std::path::Path::new(&active)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned());
            walls.iter().position(|w| {
                std::path::Path::new(&w.path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .as_deref()
                    == stem.as_deref()
            })
        })
        .unwrap_or(0)
}

/// Sets the current theme's WALLPAPER variable (or clears it), re-applies the
/// theme, and repaints directly: awww for stills, mpvpaper for videos. Dynamic
/// palettes are regenerated by theme-set from the new image.
fn set_wallpaper(image: Option<String>, video: bool) {
    std::thread::spawn(move || {
        let theme = crate::theme::current_name();
        if theme.is_empty() {
            let mut s = Settings::write();
            s.status = "No active theme — apply one first.".into();
            return;
        }
        {
            let mut s = Settings::write();
            if s.theme_busy {
                return;
            }
            s.theme_busy = true;
            s.status = format!("Switching wallpaper for {theme}…");
        }
        let value = image.as_deref().unwrap_or("none");
        let file = theme_file(&theme);
        let original = fs::read_to_string(&file).unwrap_or_default();
        let mut out = String::new();
        let mut replaced = false;
        for line in original.lines() {
            let key = line.split('=').next().map(str::trim).unwrap_or("");
            if key == "WALLPAPER" {
                out.push_str(&format!("WALLPAPER={value}\n"));
                replaced = true;
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        if !replaced {
            out.push_str(&format!("WALLPAPER={value}\n"));
        }
        // Dual write: the theme-set-visible copy and the live file, both
        // updated with the new WALLPAPER so theme-set always sees it.
        let mut wrote_any = false;
        let mut failures: Vec<String> = vec![];
        for target in theme_write_targets(&theme) {
            match fs::write(&target, &out) {
                Ok(()) => wrote_any = true,
                Err(e) => failures.push(format!("{} ({e})", target.display())),
            }
        }
        if !wrote_any {
            let mut s = Settings::write();
            s.status = format!(
                "Could not write {theme}: {}; theme files are only writable when themesDir links a live folder.",
                failures.join("; ")
            );
            s.theme_busy = false;
            return;
        }
        {
            let mut s = Settings::write();
            s.themes = themes();
            if failures.is_empty() {
                s.status = format!("Wallpaper saved — running theme-set for {theme}…");
            } else {
                s.status = format!(
                    "Wallpaper saved ({} failed) — running theme-set for {theme}…",
                    failures.join("; ")
                );
            }
        }
        // Synchronous apply: theme-set regenerates every config, then awww
        // paints the image — the pinned theme-manager paints its wallpaper via
        // Quickshell, which is absent on this machine, so the repaint is ours.
        let mut notes: Vec<String> = vec![];
        match Command::new("theme-set").arg(&theme).output() {
            Ok(o) if o.status.success() => {
                note_theme_applied(&theme, &mut notes);
            }
            Ok(o) => notes.push(format!(
                "theme-set failed: {}",
                String::from_utf8_lossy(&o.stderr)
                    .chars()
                    .take(120)
                    .collect::<String>()
            )),
            Err(e) => notes.push(format!("Cannot launch theme-set: {e}")),
        }
        notes.push(paint_wallpaper(image.as_deref(), video));
        let mut s = Settings::write();
        s.status = notes.join(" ");
        s.theme_busy = false;
    });
}

/// theme-daemon applies its config generation async after the state write; a
/// short wait with confirmation keeps the status honest about "applied".
fn note_theme_applied(theme: &str, notes: &mut Vec<String>) {
    for _ in 0..30 {
        if crate::theme::current_name() == theme {
            notes.push(format!("theme-set applied {theme}."));
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    notes.push(format!(
        "theme-set requested {theme}, but theme-daemon has not confirmed it."
    ));
}

/// Paints (or clears) the wallpaper — replacing the dead Quickshell path of
/// the pinned theme-manager: awww paints stills, mpvpaper plays videos.
fn paint_wallpaper(image: Option<&str>, video: bool) -> String {
    match image {
        Some(file) if video => {
            kill_quiet("awww-daemon");
            match Command::new("mpvpaper")
                .args(["-o", "loop", "ALL"])
                .arg(file)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(_) => "Video playing with mpvpaper.".into(),
                Err(_) => {
                    "mpvpaper is not installed yet — rebuild the system to add it.".into()
                }
            }
        }
        Some(file) => {
            kill_quiet("mpvpaper");
            let running = Command::new("awww")
                .arg("query")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !running {
                let daemon = Command::new("awww-daemon")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                if daemon.is_err() {
                    return "awww-daemon could not start.".into();
                }
                std::thread::sleep(Duration::from_millis(600));
            }
            match Command::new("awww").arg("img").arg(file).output() {
                Ok(o) if o.status.success() => "Painted with awww.".into(),
                Ok(o) => format!(
                    "awww img failed: {}",
                    String::from_utf8_lossy(&o.stderr)
                        .chars()
                        .take(120)
                        .collect::<String>()
                ),
                Err(e) => format!("awww unavailable: {e}"),
            }
        }
        None => {
            kill_quiet("mpvpaper");
            kill_quiet("awww-daemon");
            "Wallpaper stopped (backdrop only).".into()
        }
    }
}

/// Terminates a process best-effort, silencing "no process found" noise.
fn kill_quiet(name: &str) {
    let _ = Command::new("killall")
        .args([name, "--quiet"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn wallpaper_root() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Pictures/walls")
}

/// Opens the wallpapers folder in the desktop file manager.
fn open_wallpaper_folder() {
    let root = wallpaper_root();
    let _ = fs::create_dir_all(&root);
    let _ = Command::new("xdg-open")
        .arg(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Rescans the wallpaper folders, keeping the current selection when possible.
fn refresh_wallpapers() {
    let mut s = Settings::write();
    let previous = s.wallpapers.get(s.wallpaper_index).map(|w| w.path.clone());
    s.wallpapers = wallpapers();
    s.wallpaper_index = previous
        .and_then(|path| s.wallpapers.iter().position(|w| w.path == path))
        .unwrap_or_else(|| active_wallpaper_index(&s.wallpapers));
    let count = s.wallpapers.len();
    s.wallpaper_index = s.wallpaper_index.min(count.saturating_sub(1));
    let indices = wall_indices(&s);
    if !indices.contains(&s.wallpaper_index) {
        s.wallpaper_index = indices.first().copied().unwrap_or(0);
    }
    s.status = format!(
        "Wallpaper library refreshed · {count} file{}.",
        if count == 1 { "" } else { "s" }
    );
}

/// Indices into s.wallpapers that match the sidebar folder filter.
fn wall_indices(s: &Settings) -> Vec<usize> {
    s.wallpapers
        .iter()
        .enumerate()
        .filter(|(_, w)| s.wallpaper_filter == "all" || s.wallpaper_filter == w.category)
        .map(|(i, _)| i)
        .collect()
}

/// Jumps the featured preview to a gallery row.
fn select_wallpaper(index: usize) {
    let mut s = Settings::write();
    if index < s.wallpapers.len() {
        s.wallpaper_index = index;
    }
}

/// Switches the sidebar folder filter (clicking the active filter goes back to
/// "All") and keeps the carousel on a visible slide.
fn filter_wallpaper(category: String) {
    let mut s = Settings::write();
    s.wallpaper_filter = if s.wallpaper_filter == category {
        "all".into()
    } else {
        category
    };
    let indices = wall_indices(&s);
    if !indices.is_empty() && !indices.contains(&s.wallpaper_index) {
        s.wallpaper_index = indices[0];
    }
}

/// Trashes the selected wallpaper from ~/Pictures/walls (recoverable via the
/// desktop Trash) and refreshes the list.
fn remove_wallpaper(index: usize) {
    let item = {
        let mut s = Settings::write();
        let item = s.wallpapers.get(index).cloned();
        if let Some(item) = &item {
            s.status = format!("Removing {}…", item.name);
        }
        item
    };
    let Some(item) = item else { return };
    std::thread::spawn(move || {
        let was_active = active_wallpaper() == item.path;
        let out = Command::new("gio")
            .args(["trash", "--", &item.path])
            .output();
        let status = match out {
            Ok(o) if o.status.success() => {
                let hint = if was_active {
                    " It was active — apply another one to repaint."
                } else {
                    ""
                };
                format!(
                    "Trashed {} (recoverable in the desktop Trash).{}",
                    item.name, hint
                )
            }
            Ok(o) => format!(
                "Could not trash {}: {}",
                item.name,
                String::from_utf8_lossy(&o.stderr)
                    .chars()
                    .take(120)
                    .collect::<String>()
            ),
            Err(e) => format!("Could not trash {}: {e}", item.name),
        };
        let mut s = Settings::write();
        s.wallpapers = wallpapers();
        let count = s.wallpapers.len();
        s.wallpaper_index = s.wallpaper_index.min(count.saturating_sub(1));
        let indices = wall_indices(&s);
        if !indices.contains(&s.wallpaper_index) {
            s.wallpaper_index = indices.first().copied().unwrap_or(0);
        }
        s.status = status;
    });
}

/// The height of the featured preview. The gallery beside it matches it.
const WALL_HERO_H: f32 = 220.0;

/// Text drawn over the featured image, always light regardless of the theme.
fn wallpaper_hero_text(text: impl Into<String>, size: f32) -> Text {
    Text::new(text).size(size).color(Color::WHITE)
}

/// A small accent link used for secondary wallpaper actions.
fn wallpaper_link(text: &str, color: Color, action: impl Fn() + 'static) -> Rectangle {
    let key = format!("wall-link:{text}");
    let ck = key.clone();
    let content = Text::new(text)
        .size(11.0)
        .weight(Weight::SemiBold)
        .color(color);
    let width = match Widget::width(&content) {
        amane::Size::Fixed(width) => width + 10.0,
        _ => 80.0,
    };
    Rectangle::new()
        .width(width)
        .height(22.0)
        .radius(8.0)
        .cursor(Pointer)
        .scale(crate::feedback::scale(&ck))
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                action()
            }
        })
        .align_child(Center, Center)
        .child(content)
}

/// A filled primary action, full width beneath the featured preview.
fn wallpaper_primary(text: &str, width: f32, action: impl Fn() + 'static) -> Rectangle {
    let key = format!("wall-action:{text}");
    let ck = key.clone();
    let fill = blend_color(crate::theme::palette().cyan, crate::theme::background(), 0.14);
    let color = crate::theme::readable(crate::theme::background(), fill, 4.5);
    let content = Text::new(text)
        .size(12.0)
        .weight(Weight::SemiBold)
        .color(color)
        .elide();
    Rectangle::new()
        .width(width)
        .height(38.0)
        .radius(12.0)
        .fill(fill)
        .cursor(Pointer)
        .scale(crate::feedback::scale(&ck))
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                action()
            }
        })
        .align_child(Center, Center)
        .child(
            Rectangle::new()
                .width(width - 20.0)
                .height(20.0)
                .align_child(Center, Center)
                .child(content),
        )
}

/// A compact status pill drawn over a wallpaper image.
fn wallpaper_badge(text: &str, width: f32, fill: Color) -> Rectangle {
    let color = crate::theme::readable(crate::theme::background(), fill, 4.5);
    Rectangle::new()
        .width(width)
        .height(22.0)
        .radius(Full)
        .fill(fill)
        .align_child(Center, Center)
        .child(
            Text::new(text)
                .size(9.5)
                .weight(Weight::SemiBold)
                .color(color),
        )
}

/// A soft theme-coloured fill for media that has no decoded preview yet.
fn wallpaper_placeholder(width: f32, height: f32, text: &str, radius: f32) -> Rectangle {
    let palette = crate::theme::palette();
    let top = blend_color(palette.blue, crate::theme::background(), 0.82);
    let bottom = blend_color(palette.cyan, crate::theme::background(), 0.76);
    Rectangle::new()
        .width(width)
        .height(height)
        .radius(radius)
        .clip()
        .fill(amane::Gradient::linear(
            135.0,
            [(0.0, top), (1.0, bottom)],
        ))
        .align_child(Center, Center)
        .child(
            Text::new(text)
                .size(11.0)
                .color(crate::theme::readable(Color::WHITE, top, 3.0)),
        )
}

fn wallpaper_chip_label(label: &str, count: usize) -> String {
    format!("{label}  {count}")
}

fn wallpaper_chip_width(text: &str) -> f32 {
    let content = Text::new(text).size(11.0);
    let width = match Widget::width(&content) {
        amane::Size::Fixed(width) => width,
        _ => text.chars().count() as f32 * 6.8,
    };
    (width + 22.0).clamp(56.0, 176.0)
}

/// A folder/category filter chip. `filter` is the value stored in settings,
/// which is distinct from the human-readable label ("all" vs "All").
fn wallpaper_chip(
    label: &str,
    filter: String,
    count: usize,
    active_filter: bool,
) -> Rectangle {
    let key = format!("wall-filter:{filter}");
    let ck = key.clone();
    let text = wallpaper_chip_label(label, count);
    let width = wallpaper_chip_width(&text);
    let accent = crate::theme::palette().cyan;
    let fill = if active_filter {
        blend_color(accent, crate::theme::background(), 0.12)
    } else {
        setting_surface()
    };
    let color = if active_filter {
        crate::theme::readable(crate::theme::background(), fill, 4.5)
    } else {
        settings_foreground()
    };
    let content = Text::new(text)
        .size(11.0)
        .weight(if active_filter {
            Weight::SemiBold
        } else {
            Weight::Regular
        })
        .color(color)
        .elide();
    Rectangle::new()
        .width(width)
        .height(28.0)
        .radius(14.0)
        .fill(fill)
        .cursor(Pointer)
        .scale(crate::feedback::scale(&ck))
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                filter_wallpaper(filter.clone());
            }
        })
        .align_child(Center, Center)
        .child(
            Rectangle::new()
                .width(width - 14.0)
                .height(17.0)
                .align_child(Center, Center)
                .child(content),
        )
}

/// The featured preview with an image scrim, filename, badges and apply action.
fn wallpaper_hero(s: &Settings, width: f32, active: &str) -> Column {
    let index = s
        .wallpaper_index
        .min(s.wallpapers.len().saturating_sub(1));
    let item = s.wallpapers.get(index).cloned();
    let selected_active = item
        .as_ref()
        .is_some_and(|w| w.path == active);
    let display = item
        .as_ref()
        .map(wall_display)
        .unwrap_or_default();

    let mut layers: Vec<Box<dyn Widget>> = vec![];
    if display.is_empty() {
        let text = match &item {
            Some(w) if w.video => "Preparing video preview…",
            Some(_) => "Preparing preview…",
            None => "No wallpapers in this folder",
        };
        layers.push(Box::new(wallpaper_placeholder(
            width,
            WALL_HERO_H,
            text,
            16.0,
        )));
    } else {
        layers.push(Box::new(
            Rectangle::new()
                .width(width)
                .height(WALL_HERO_H)
                .radius(16.0)
                .clip()
                .fill(
                    amane::Image::cover(display)
                        .thumbnail(width as u32, WALL_HERO_H as u32),
                ),
        ));
    }

    let mut badges = 12.0;
    if selected_active {
        layers.push(Box::new(
            wallpaper_badge("ACTIVE", 62.0, crate::theme::palette().cyan)
                .translate(badges, 12.0),
        ));
        badges += 68.0;
    }
    if let Some(w) = &item {
        if w.video {
            layers.push(Box::new(
                wallpaper_badge("VIDEO", 58.0, Color::rgba(0, 0, 0, 170))
                    .translate(badges, 12.0),
            ));
        }
    }

    if let Some(w) = &item {
        // A dark gradient keeps the caption readable over bright wallpapers.
        layers.push(Box::new(
            Rectangle::new()
                .width(width)
                .height(96.0)
                .translate(0.0, WALL_HERO_H - 96.0)
                .fill(amane::Gradient::linear(
                    0.0,
                    [
                        (0.0, Color::rgba(0, 0, 0, 205)),
                        (1.0, Color::rgba(0, 0, 0, 0)),
                    ],
                )),
        ));
        let detail = format!(
            "{}  ·  {}",
            w.category,
            if w.video { "Video" } else { "Image" }
        );
        let caption = Column::new(children![
            Rectangle::new()
                .width((width - 28.0).max(20.0))
                .height(18.0)
                .child(wallpaper_hero_text(w.name.clone(), 14.0).weight(Weight::SemiBold).elide()),
            Rectangle::new()
                .width((width - 28.0).max(20.0))
                .height(14.0)
                .child(
                    wallpaper_hero_text(detail, 10.5)
                        .color(Color::rgba(255, 255, 255, 190))
                        .elide(),
                ),
        ])
        .gap(2.0)
        .width((width - 28.0).max(20.0));
        layers.push(Box::new(
            Rectangle::new()
                .width((width - 28.0).max(20.0))
                .height(38.0)
                .translate(14.0, WALL_HERO_H - 50.0)
                .child(caption),
        ));
    }

    if selected_active {
        layers.push(Box::new(
            Rectangle::new()
                .width(width)
                .height(WALL_HERO_H)
                .radius(16.0)
                .border(2.0, crate::theme::palette().cyan),
        ));
    }

    let key = format!(
        "wall-hero:{}",
        item.as_ref().map(|w| w.path.as_str()).unwrap_or("empty")
    );
    let ck = key.clone();
    let apply = item.clone();
    let mut preview = Rectangle::new()
        .width(width)
        .height(WALL_HERO_H)
        .radius(16.0)
        .clip()
        .fill(setting_surface())
        .scale(crate::feedback::scale(&key));
    if let Some(item) = apply {
        preview = preview.cursor(Pointer).on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                set_wallpaper(Some(item.path.clone()), item.video);
            }
        });
    }
    let preview = preview.child(
        amane::Stack::new(layers)
            .width(width)
            .height(WALL_HERO_H),
    );

    let mut actions: Vec<Box<dyn Widget>> = vec![];
    match &item {
        Some(w) => {
            let apply = w.clone();
            let label = if selected_active {
                "Reapply wallpaper"
            } else if w.video {
                "Play video wallpaper"
            } else {
                "Apply wallpaper"
            };
            actions.push(Box::new(wallpaper_primary(label, width, move || {
                set_wallpaper(Some(apply.path.clone()), apply.video);
            })));
            let remove_index = index;
            actions.push(Box::new(
                Row::new(children![
                    wallpaper_link("Stop wallpaper", settings_secondary(), || {
                        set_wallpaper(None, false)
                    }),
                    wallpaper_link(
                        "Remove file",
                        crate::theme::readable(
                            crate::theme::palette().red,
                            setting_surface(),
                            4.5
                        ),
                        move || remove_wallpaper(remove_index),
                    ),
                ])
                .gap(14.0)
                .justify(amane::SpaceBetween)
                .width(width),
            ));
        }
        None => {
            actions.push(Box::new(wallpaper_primary(
                "Open wallpapers folder",
                width,
                open_wallpaper_folder,
            )));
            actions.push(Box::new(
                Row::new(children![
                    wallpaper_link(
                        "Rescan library",
                        crate::theme::palette().cyan,
                        refresh_wallpapers
                    ),
                    wallpaper_link("Stop wallpaper", settings_secondary(), || {
                        set_wallpaper(None, false)
                    }),
                ])
                .gap(14.0)
                .justify(amane::SpaceBetween)
                .width(width),
            ));
        }
    }

    let mut rows: Vec<Box<dyn Widget>> = vec![Box::new(preview)];
    rows.extend(actions);
    Column::new(rows).width(width).gap(9.0)
}

/// A single row in the gallery: thumbnail, name, folder/type and active dot.
fn wallpaper_gallery_row(
    s: &Settings,
    index: usize,
    width: f32,
    active: &str,
) -> Rectangle {
    let Some(item) = s.wallpapers.get(index).cloned() else {
        return Rectangle::new().width(width).height(0.0);
    };
    let display = wall_display(&item);
    let selected = s.wallpaper_index == index;
    let is_active = item.path == active;
    let key = format!("wall-row:{}", item.path);
    let ck = key.clone();
    let thumb_w = 66.0;
    let thumb_h = 44.0;
    let fill = if selected {
        blend_color(
            crate::theme::palette().cyan,
            crate::theme::background(),
            0.80,
        )
    } else {
        setting_surface()
    };
    let thumb = if display.is_empty() {
        wallpaper_placeholder(thumb_w, thumb_h, "…", 8.0)
    } else {
        Rectangle::new()
            .width(thumb_w)
            .height(thumb_h)
            .radius(8.0)
            .clip()
            .fill(amane::Image::cover(display).thumbnail(thumb_w as u32, thumb_h as u32))
    };
    let info_w = (width - 14.0 - thumb_w - 10.0 - if is_active { 18.0 } else { 0.0 })
        .max(36.0);
    let detail = format!(
        "{}  ·  {}",
        item.category,
        if item.video { "Video" } else { "Image" }
    );
    let info = Column::new(children![
        Rectangle::new()
            .width(info_w)
            .height(16.0)
            .child(
                label(item.name.clone())
                    .size(12.0)
                    .weight(if selected {
                        Weight::SemiBold
                    } else {
                        Weight::Regular
                    })
                    .elide(),
            ),
        Rectangle::new()
            .width(info_w)
            .height(13.0)
            .child(
                label(detail)
                    .size(10.0)
                    .color(settings_secondary())
                    .elide(),
            ),
    ])
    .gap(2.0)
    .width(info_w);
    let mut contents: Vec<Box<dyn Widget>> = vec![
        Box::new(thumb) as Box<dyn Widget>,
        Box::new(info) as Box<dyn Widget>,
    ];
    if is_active {
        contents.push(Box::new(
            Rectangle::new()
                .width(8.0)
                .height(8.0)
                .radius(Full)
                .fill(crate::theme::palette().cyan),
        ));
    }

    let mut row = Rectangle::new()
        .width(width)
        .height(58.0)
        .padding(7.0)
        .radius(12.0)
        .fill(fill)
        .cursor(Pointer)
        .scale(crate::feedback::scale(&key));
    if selected {
        row = row.border(
            1.0,
            blend_color(
                crate::theme::palette().cyan,
                crate::theme::background(),
                0.35,
            ),
        );
    }
    row.on_click(move |b| {
        if b == Button::Left {
            crate::feedback::click(&ck);
            select_wallpaper(index);
        }
    })
    .child(Row::new(contents).gap(10.0).align(Center))
}

/// The scannable wallpaper list beside the featured preview.
fn wallpaper_gallery(s: &Settings, width: f32, active: &str) -> Column {
    let indices = wall_indices(s);
    let mut rows: Vec<Box<dyn Widget>> = vec![];
    for index in &indices {
        rows.push(Box::new(wallpaper_gallery_row(
            s, *index, width, active,
        )));
    }
    let list: Box<dyn Widget> = if rows.is_empty() {
        Box::new(
            Rectangle::new()
                .width(width)
                .height(110.0)
                .radius(14.0)
                .fill(setting_surface())
                .align_child(Center, Center)
                .child(
                    Column::new(children![
                        label("No media files here").size(12.0).weight(Weight::SemiBold),
                        label("Add images or videos to ~/Pictures/walls")
                            .size(10.0)
                            .color(settings_secondary()),
                        wallpaper_link(
                            "Open folder",
                            crate::theme::palette().cyan,
                            open_wallpaper_folder
                        ),
                    ])
                    .gap(5.0)
                    .align(Center),
                ),
        )
    } else {
        Box::new(
            ScrollArea::new(
                "wall-gallery-scroll",
                Column::new(rows).width(width - 8.0).gap(6.0),
            )
            .width(width)
            .height(WALL_HERO_H + 39.0),
        )
    };
    let heading = Row::new(children![
        Column::new(children![
            label("Gallery").size(13.0).weight(Weight::SemiBold),
            label(format!("{} shown", indices.len()))
                .size(10.0)
                .color(settings_secondary()),
        ])
        .gap(1.0),
        Row::new(children![
            wallpaper_link(
                "Rescan",
                crate::theme::palette().cyan,
                refresh_wallpapers
            ),
            wallpaper_link("Open", settings_secondary(), open_wallpaper_folder),
        ])
        .gap(8.0),
    ])
    .justify(amane::SpaceBetween)
    .align(Center)
    .width(width);
    Column::new(vec![Box::new(heading) as Box<dyn Widget>, list])
        .gap(8.0)
        .width(width)
}

/// Filters, featured preview and gallery. This replaces the old carousel so the
/// selected wallpaper stays visible while browsing the whole folder.
fn wallpaper_section(s: &Settings, width: f32) -> Column {
    let side_w = (width * 0.38).clamp(188.0, 244.0);
    let main_w = (width - side_w - 18.0).max(180.0);

    let mut categories: Vec<String> = s.wallpapers.iter().map(|w| w.category.clone()).collect();
    categories.sort();
    categories.dedup();
    let mut chips: Vec<(Box<dyn Widget>, f32)> = vec![];
    let all_text = wallpaper_chip_label("All", s.wallpapers.len());
    chips.push((
        Box::new(wallpaper_chip(
            "All",
            "all".into(),
            s.wallpapers.len(),
            s.wallpaper_filter == "all",
        )),
        wallpaper_chip_width(&all_text),
    ));
    for cat in &categories {
        let count = s
            .wallpapers
            .iter()
            .filter(|w| &w.category == cat)
            .count();
        if count == 0 {
            continue;
        }
        let text = wallpaper_chip_label(cat, count);
        chips.push((
            Box::new(wallpaper_chip(
                cat,
                cat.clone(),
                count,
                s.wallpaper_filter == *cat,
            )),
            wallpaper_chip_width(&text),
        ));
    }
    let mut chip_rows: Vec<Box<dyn Widget>> = vec![];
    let mut row: Vec<Box<dyn Widget>> = vec![];
    let mut row_width = 0.0;
    for (chip, chip_width) in chips {
        if !row.is_empty() && row_width + 6.0 + chip_width > width {
            chip_rows.push(Box::new(Row::new(row).gap(6.0)));
            row = vec![];
            row_width = 0.0;
        }
        if !row.is_empty() {
            row_width += 6.0;
        }
        row.push(chip);
        row_width += chip_width;
    }
    if !row.is_empty() {
        chip_rows.push(Box::new(Row::new(row).gap(6.0)));
    }

    let head = Column::new(chip_rows).gap(6.0);
    let active = active_wallpaper();
    let parts: Vec<Box<dyn Widget>> = children![
        wallpaper_hero(s, main_w, &active),
        Rectangle::new().width(1.0).height(WALL_HERO_H + 62.0).fill(setting_surface()),
        wallpaper_gallery(s, side_w, &active)
    ];

    Column::new(children![
        head,
        Row::new(parts).gap(14.0).align(Start)
    ])
    .gap(12.0)
}

fn section_label(title: &str, detail: &str, width: f32) -> Row {
    Row::new(children![
        label(title).size(16.0).weight(Weight::SemiBold),
        Rectangle::new()
            .width((width - 150.0).max(20.0))
            .height(24.0)
            .align_child(End, Center)
            .child(label(detail).size(12.0).color(settings_secondary()))
    ])
    .gap(10.0)
    .align(Center)
}
fn info_row(
    glyph: &str,
    title: &str,
    subtitle: &str,
    trailing: &str,
    width: f32,
    accent: Color,
) -> Rectangle {
    let accent = crate::theme::readable(accent, setting_surface(), 3.0);
    let subtitle = label(subtitle)
        .size(12.0)
        .color(settings_secondary())
        .wrap()
        .max_lines(3)
        .elide();
    let sh = subtitle.height_in((width - 160.0).max(90.0));
    Rectangle::new()
        .width(width)
        .height((sh + 38.0).max(68.0))
        .padding(10.0)
        .radius(14.0)
        .fill(setting_surface())
        .child(
            Row::new(children![
                Rectangle::new()
                    .width(38.0)
                    .height(40.0)
                    .align_child(Center, Center)
                    .child(
                        Text::new(glyph)
                            .font("Material Icons Round")
                            .size(29.0)
                            .tight()
                            .color(accent)
                    ),
                Column::new(children![
                    label(title).size(15.0).weight(Weight::SemiBold).elide(),
                    Rectangle::new()
                        .width((width - 160.0).max(90.0))
                        .height(sh)
                        .child(subtitle)
                ])
                .width((width - 160.0).max(90.0))
                .gap(5.0),
                Rectangle::new()
                    .width(80.0)
                    .height(30.0)
                    .align_child(End, Center)
                    .child(label(trailing).size(12.0).color(accent).elide())
            ])
            .gap(10.0)
            .align(Center),
        )
}
fn network_page(s: &Settings, net: &Network, width: f32) -> Vec<Box<dyn Widget>> {
    let p = crate::theme::palette();
    let status = network_status::snapshot();
    let active = status.links.iter().any(|l| l.contains("· up"));
    let title = if net.connecting() {
        "Connecting…"
    } else if net.connected() || active {
        "You're connected"
    } else {
        "No active connection"
    };
    let mut rows = children![
        summary_card(
            "\u{e63e}",
            title,
            if status.managed {
                "NetworkManager"
            } else {
                "Managed by systemd-networkd"
            },
            width,
            p.green
        ),
        section_label("Connections", "Live interfaces", width)
    ];
    for link in &status.links {
        let (name, rest) = link.split_once(" · ").unwrap_or((link, ""));
        let mut fields = rest.split_whitespace();
        let state = fields.next().unwrap_or("unknown");
        let addresses = fields.collect::<Vec<_>>().join("\n");
        let wireless = name.starts_with("wl");
        rows.push(Box::new(info_row(
            if wireless { "\u{e63e}" } else { "\u{e8be}" },
            if wireless {
                "Wi-Fi adapter"
            } else {
                "Ethernet"
            },
            &format!(
                "{name}{}{}",
                if addresses.is_empty() { "" } else { "\n" },
                addresses
            ),
            if state == "up" {
                "Connected"
            } else {
                "Inactive"
            },
            width,
            if state == "up" { p.green } else { p.blue },
        )));
    }
    if !status.managed {
        rows.push(Box::new(info_row("\u{e1da}","Wi-Fi controls unavailable","No Wi-Fi manager is running. Wired networking and your networkd configuration are unchanged.","",width,p.yellow)));
        return rows;
    }
    let on = net.wifi_enabled();
    rows.push(Box::new(
        Row::new(children![
            button(
                if on { "Wi-Fi on" } else { "Enable Wi-Fi" },
                130.0,
                move || Network::set_wifi(!on)
            ),
            button("Scan networks", 140.0, Network::scan)
        ])
        .gap(12.0),
    ));
    if !net.ssid().is_empty() {
        rows.push(Box::new(info_row(
            "\u{e63e}",
            net.ssid(),
            "Current Wi-Fi connection",
            &format!("{}%", net.strength()),
            width,
            p.green,
        )));
        rows.push(Box::new(button("Disconnect", 120.0, Network::disconnect)));
    }
    rows.push(Box::new(section_label(
        "Nearby networks",
        &format!("{} found", net.access_points().len()),
        width,
    )));
    for ap in net.access_points() {
        let name = ap.ssid().to_owned();
        let secured = ap.secured();
        let strength = ap.strength();
        let key = format!("ap:{name}");
        let ck = key.clone();
        rows.push(Box::new(
            info_row(
                "\u{e63e}",
                &name,
                if secured {
                    "Secured network"
                } else {
                    "Open network"
                },
                &format!("{strength}%"),
                width,
                p.blue,
            )
            .cursor(Pointer)
            .scale(crate::feedback::scale(&key))
            .on_click(move |b| {
                if b == Button::Left {
                    crate::feedback::click(&ck);
                    if secured {
                        TextInput::set_text("amane-wifi-password", "");
                        let mut state = Settings::write();
                        state.ssid = Some(name.clone());
                        state.password.clear();
                        state.status = String::new();
                    } else {
                        Network::connect(&name, None);
                    }
                }
            }),
        ));
    }
    if net.access_points().is_empty() {
        rows.push(Box::new(info_row(
            "\u{e1da}",
            "No networks found",
            "Enable Wi-Fi and scan to find nearby networks.",
            "",
            width,
            p.cyan,
        )));
    }
    if let Some(ssid) = &s.ssid {
        rows.push(Box::new(
            label(format!("Join {ssid}")).weight(Weight::SemiBold),
        ));
        rows.push(Box::new(
            Rectangle::new()
                .width(width)
                .height(44.0)
                .padding(10.0)
                .radius(14.0)
                .fill(blend_color(p.blue, Color::BLACK, 0.9))
                .child(
                    TextInput::new("amane-wifi-password")
                        .password()
                        .size(15.0)
                        .color(p.foreground)
                        .placeholder("Password · empty uses saved connection")
                        .on_change(|v| Settings::write().password = v)
                        .on_submit(|_| connect_selected()),
                ),
        ));
        rows.push(Box::new(button("Connect", 120.0, connect_selected)));
    }
    rows
}
fn bluetooth_page(bt: &Bluetooth, width: f32) -> Vec<Box<dyn Widget>> {
    let p = crate::theme::palette();
    let connected = bt.devices().iter().filter(|d| d.connected()).count();
    let mut rows = children![summary_card(
        "\u{e1a7}",
        if !bt.available() {
            "Bluetooth unavailable"
        } else if bt.powered() {
            "Ready to connect"
        } else {
            "Bluetooth is off"
        },
        &format!(
            "{connected} connected · {} known devices",
            bt.devices().len()
        ),
        width,
        p.cyan
    )];
    if !bt.available() {
        rows.push(Box::new(info_row(
            "\u{e1da}",
            "No adapter detected",
            "Bluetooth requires a supported adapter and the BlueZ service.",
            "",
            width,
            p.yellow,
        )));
        return rows;
    }
    let on = bt.powered();
    let scanning = bt.scanning();
    rows.push(Box::new(
        Row::new(children![
            button(
                if on {
                    "Turn Bluetooth off"
                } else {
                    "Turn Bluetooth on"
                },
                160.0,
                move || Bluetooth::set_powered(!on)
            ),
            button(
                if scanning {
                    "Stop discovery"
                } else {
                    "Find devices"
                },
                140.0,
                move || if scanning {
                    Bluetooth::stop_scan()
                } else {
                    Bluetooth::start_scan()
                }
            )
        ])
        .gap(12.0),
    ));
    for paired_group in [true, false] {
        let devices = bt
            .devices()
            .iter()
            .filter(|d| d.paired() == paired_group)
            .collect::<Vec<_>>();
        if devices.is_empty() {
            continue;
        }
        rows.push(Box::new(section_label(
            if paired_group {
                "Your devices"
            } else {
                "Nearby devices"
            },
            &format!(
                "{} {}",
                devices.len(),
                if devices.len() == 1 {
                    "device"
                } else {
                    "devices"
                }
            ),
            width,
        )));
        for d in devices {
            let path = d.path().to_owned();
            let connected = d.connected();
            let paired = d.paired();
            let state = if connected {
                "Connected"
            } else if paired {
                "Tap to connect"
            } else {
                "Tap to pair"
            };
            let glyph = if d.icon().contains("head") {
                "\u{e310}"
            } else if d.icon().contains("phone") {
                "\u{e324}"
            } else if d.icon().contains("keyboard") {
                "\u{e312}"
            } else if d.icon().contains("mouse") {
                "\u{e323}"
            } else {
                "\u{e1a7}"
            };
            let tail = d
                .battery()
                .map(|v| format!("{v}% battery"))
                .unwrap_or_else(|| {
                    if connected {
                        "Active".into()
                    } else {
                        String::new()
                    }
                });
            let key = format!("bt:{path}");
            let ck = key.clone();
            rows.push(Box::new(info_row(glyph,d.name(),&format!("{state}\n{}",d.address()),&tail,width,if connected{p.green}else{p.cyan}).cursor(Pointer).scale(crate::feedback::scale(&key)).on_click(move|b|if b==Button::Left{crate::feedback::click(&ck);if connected{Bluetooth::disconnect(&path)}else if paired{Bluetooth::connect(&path)}else{Bluetooth::pair(&path);Settings::write().status="Pairing requested. PIN/passkey devices may require a pairing agent.".into();}})));
        }
    }
    if bt.devices().is_empty() {
        rows.push(Box::new(info_row(
            "\u{e1b1}",
            if scanning {
                "Looking for devices…"
            } else {
                "No devices yet"
            },
            "Make your device discoverable, then choose Find devices.",
            "",
            width,
            p.blue,
        )));
    }
    rows
}

fn visibility_switch(fullscreen: bool, width: f32) -> Rectangle {
    let p = crate::theme::palette();
    let (on, glyph, title, detail, accent) = if fullscreen {
        (
            visibility::over_fullscreen(),
            "\u{e5d0}",
            "Allow over fullscreen",
            "Permit the island above fullscreen apps",
            p.cyan,
        )
    } else {
        (
            visibility::auto_hide(),
            "\u{e8f4}",
            "Auto-hide main island",
            "Off keeps it visible; IPC hiding still works",
            p.blue,
        )
    };
    toggle_row(glyph, title, detail, width, on, accent, move || {
        if fullscreen {
            visibility::toggle_fullscreen()
        } else {
            visibility::toggle_auto_hide()
        }
    })
}

fn edge_switch(name: &'static str, width: f32) -> Rectangle {
    let on = crate::edge_activation::enabled(name);
    let p = crate::theme::palette();
    let (glyph, title, detail, accent) = match name {
        "main" => ("\u{e5ce}", "Main island", "Top edge", p.blue),
        "screenshot" => ("\u{e3b0}", "Screenshots", "Lower-right edge", p.green),
        "record" => ("\u{e04b}", "Screen recording", "Upper-left edge", p.red),
        "session" => ("\u{e8ac}", "Session controls", "Lower-left edge", p.yellow),
        _ => ("\u{e8b6}", "Search", "Bottom edge", p.cyan),
    };
    toggle_row(glyph, title, detail, width, on, accent, move || {
        crate::edge_activation::toggle(name)
    })
}

fn summary_card(glyph: &str, title: &str, subtitle: &str, width: f32, color: Color) -> Rectangle {
    let color = crate::theme::readable(color, setting_surface(), 3.0);
    Rectangle::new()
        .width(width)
        .height(74.0)
        .padding(12.0)
        .radius(16.0)
        .fill(setting_surface())
        .child(
            Row::new(children![
                Rectangle::new()
                    .width(42.0)
                    .height(42.0)
                    .radius(13.0)
                    .fill(blend_color(color, crate::theme::background(), 0.82))
                    .align_child(Center, Center)
                    .child(
                        Text::new(glyph)
                            .font("Material Icons Round")
                            .size(26.0)
                            .tight()
                            .color(color)
                    ),
                Column::new(children![
                    label(title).size(16.0).weight(Weight::SemiBold),
                    label(subtitle).size(12.0).color(settings_secondary())
                ])
                .gap(5.0)
            ])
            .gap(12.0)
            .align(Center),
        )
}
fn sidebar(s: &Settings) -> Column {
    let w = s.sidebar_width.value();
    let expanded = s.sidebar_labels;
    let toggle = Rectangle::new()
        .width(w)
        .height(44.0)
        .radius(16.0)
        .fill(Color::TRANSPARENT)
        .cursor(Pointer)
        .align_child(Center, Center)
        .on_click(|b| {
            if b == Button::Left {
                crate::feedback::click("sidebar-toggle");
                let mut st = Settings::write();
                st.sidebar_labels = !st.sidebar_labels;
                let target = if st.sidebar_labels { 164.0 } else { 60.0 };
                st.sidebar_width.to(target);
            }
        })
        .scale(crate::feedback::scale("sidebar-toggle"))
        .child(
            Text::new("\u{e5d2}")
                .font("Material Icons Round")
                .size(24.0)
                .tight()
                .color(settings_foreground()),
        );
    let mut items = children![toggle];
    for (section, glyph, name) in [
        (Section::History, "\u{e889}", "History"),
        (Section::Network, "\u{e63e}", "Network"),
        (Section::Bluetooth, "\u{e1a7}", "Bluetooth"),
        (Section::Customize, "\u{e40a}", "Appearance"),
    ] {
        let selected = s.sections[index(section)];
        let accent = match section {
            Section::History => crate::theme::palette().blue,
            Section::Network => crate::theme::palette().green,
            Section::Bluetooth => crate::theme::palette().cyan,
            Section::Customize => crate::theme::palette().yellow,
        };
        let key = format!("nav:{name}");
        let ck = key.clone();
        let mut row = children![
            Text::new(glyph)
                .font("Material Icons Round")
                .size(25.0)
                .tight()
                .color(if selected {
                    accent
                } else {
                    blend_color(accent, settings_foreground(), 0.2)
                })
        ];
        if expanded && w > 130.0 {
            row.push(Box::new(label(name).size(14.0).weight(Weight::SemiBold)));
        }
        items.push(Box::new(
            Rectangle::new()
                .width(w)
                .height(48.0)
                .radius(14.0)
                .fill(if selected {
                    blend_color(accent, crate::theme::background(), 0.78)
                } else {
                    Color::TRANSPARENT
                })
                .scale(crate::feedback::scale(&key))
                .cursor(Pointer)
                .on_hover(move |inside| {
                    let mut st = Settings::write();
                    st.hovered = if inside { Some(index(section)) } else { None };
                })
                .align_child(
                    if expanded && w > 130.0 {
                        Align::Start
                    } else {
                        Align::Center
                    },
                    Center,
                )
                .padding(12.0)
                .on_click(move |b| {
                    if b == Button::Left {
                        crate::feedback::click(&ck);
                        section_toggle(section)
                    }
                })
                .child(Row::new(row).gap(10.0).align(Center)),
        ));
    }
    Column::new(items).width(w).gap(8.0)
}
pub(super) fn view() -> Rectangle {
    let s = Settings::read();
    let h = s.motion.precise_value().max(0.0);
    // A closed settings page used to build, measure and draw its entire hidden
    // tree on every island frame (including all theme cards).
    if h <= 0.5 && !s.open {
        return Rectangle::new().width(PANEL.0).height(0.0);
    }
    let cw = content_width(&s);
    let selected = s.sections.iter().position(|v| *v).unwrap_or(0);
    let (title, subtitle) = match selected {
        0 => ("Notifications", "History & recent activity"),
        1 => ("Network", "Connections & nearby networks"),
        2 => ("Bluetooth", "Devices & discovery"),
        _ => ("Appearance", "Personalize your shell"),
    };
    let heading = Column::new(children![
        label(title).size(23.0).weight(Weight::Bold),
        label(subtitle).size(13.0).color(settings_secondary())
    ])
    .gap(6.0)
    .width(cw);
    let page = Column::new(children![
        heading,
        settings_scroll::view(
            match selected {
                0 => "settings-history-page",
                1 => "settings-network-page",
                2 => "settings-bt-page",
                _ => "settings-custom-page",
            },
            contents(&s),
            cw,
            (h - 118.0).max(0.0)
        )
    ])
    .gap(20.0)
    .width(cw);
    let elapsed = s.page_started.elapsed().as_secs_f32();
    let page_opacity = (elapsed / 0.22).clamp(0.0, 1.0);
    if page_opacity < 1.0 {
        amane::request_frame();
    }
    let page = Rectangle::new()
        .width(cw)
        .height((h - 40.0).max(0.0))
        .opacity(page_opacity)
        .translate(8.0 * (1.0 - page_opacity), 0.0)
        .child(page);
    let mut layout = children![
        Row::new(children![sidebar(&s), page])
            .gap(20.0)
            .align(Start)
    ];
    if !s.sidebar_labels {
        if let Some(i) = s.hovered {
            let name = ["History", "Network", "Bluetooth", "Appearance"][i];
            layout.push(Box::new(
                Rectangle::new()
                    .width(124.0)
                    .height(38.0)
                    .radius(14.0)
                    .fill(crate::theme::surface())
                    .translate(s.sidebar_width.value() + 8.0, 56.0 + i as f32 * 56.0 + 8.0)
                    .align_child(Center, Center)
                    .child(label(name).weight(Weight::SemiBold)),
            ));
        }
    }
    Rectangle::new()
        .width(PANEL.0)
        .height(h)
        .padding(Padding {
            top: 20.0,
            left: 24.0,
            right: 24.0,
            bottom: 20.0,
        })
        // No opacity here: any opacity != 1 makes the renderer draw the whole
        // panel on a window-sized offscreen canvas every frame of the open
        // animation. The clip alone reveals the content as the height grows.
        .clip()
        .on_hover(pointer_presence)
        .on_click(|_| {})
        .child(
            amane::Stack::new(layout)
                .width(PANEL.0 - 48.0)
                .height((h - 40.0).max(0.0)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidebar_budgets_fit_single_page() {
        for w in [60.0, 164.0] {
            let available = PANEL.0 - 48.0 - w - 20.0;
            assert!(available >= 440.0);
            assert!(320.0 < available);
        }
    }
    #[test]
    fn theme_hex_parser_accepts_current_theme_format() {
        assert_eq!(parse_hex("#81A1C1"), Some(Color::rgb(129, 161, 193)));
        assert_eq!(parse_hex("no"), None);
    }
    #[test]
    fn theme_merge_preserves_layout_and_updates_values() {
        let original = "# aquarium theme\nVARIANT=dark\n\n# base_16\nBASE00 = '#20202A'\nWHITE         =#C6D0E9\nCORNER_RADIUS=20\n";
        let out = merge_theme_values(
            original,
            &[
                ("BASE00".into(), "#131C19".into()),
                ("WHITE".into(), " #444F4C ".into()),
            ],
        );
        assert_eq!(
            out,
            "# aquarium theme\nVARIANT=dark\n\n# base_16\nBASE00 =#131C19\nWHITE         =#444F4C\nCORNER_RADIUS=20\n"
        );
    }
    #[test]
    fn theme_merge_appends_new_keys() {
        let out = merge_theme_values("VARIANT=dark\n", &[("MODE".into(), "dynamic".into())]);
        assert_eq!(out, "VARIANT=dark\nMODE=dynamic\n");
    }
    #[test]
    fn theme_merge_skips_comment_lines() {
        let out = merge_theme_values(
            "# x=y\nBASE00=#20202A",
            &[("BASE00".into(), "#000000".into())],
        );
        assert_eq!(out, "# x=y\nBASE00=#000000\n");
    }
}
