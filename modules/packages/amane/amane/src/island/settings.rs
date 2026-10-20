//! In-island vertical settings, with independent accordion sections.
use super::*;
use amane::{Bluetooth, Network, TextInput, Widget};
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
}
#[derive(Clone)]
struct Theme {
    name: String,
    colors: Vec<Color>,
}
impl Service for Settings {
    fn new() -> Self {
        Self {
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
        }
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
pub(super) fn state_dir() -> PathBuf {
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
        let mut cards = vec![];
        for chunk in s.themes.chunks(2) {
            let mut tiles = vec![];
            for theme in chunk {
                let name = theme.name.clone();
                let key = format!("theme:{name}");
                let ck = key.clone();
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
}
