// A dynamic island: one pill at the top of the screen that grows to show what
// just happened, then shrinks back to the clock.
//
//   idle          the clock
//   osd           a volume or brightness bar
//   notification  title and text, click to open it (or dismiss it)
//
// Events only set deadlines. `wanted_mode` turns the deadlines into what to
// show: the osd first (you just pressed a key), then a notification, then idle.
// An explicitly opened panel stays open while controls are used.
// All of it runs in `Island::update`, the view only reads and draws.

use std::time::{Duration, Instant, SystemTime};

use amane::{
    Align, Audio, Button, Center, Color, Column, End, Full, Horizontal, LayerWindow, Media,
    Notifications, Padding, Pointer, Rectangle, Row, Service, Start, Text, Vertical, Weight,
    children,
};

// Keep the layer surface snug to the animated pill, not a transparent canvas.
const GAP_TOP: f32 = 8.0;

// width and height of the pill in each mode
const IDLE: (f32, f32) = (modules::total(modules::COLLAPSED, false), 36.0);
const OSD: (f32, f32) = (310.0, 46.0);
const ROW_PADDING: f32 = 24.0;
const ROW_GAP: f32 = 20.0;
const CONTROL_GAP: f32 = 4.0;
const CLOCK_WIDTH: f32 = 96.0;
const MEDIA_WIDTH: f32 = 174.0;
const TRANSPORT_WIDTH: f32 = 180.0;
const CONTROL_WIDTH: f32 = 32.0;
const PANEL: (f32, f32) = (modules::total(modules::EXPANDED, true), 46.0);

mod border;
mod clients;
mod exposure;
// mod fullscreen;
pub(crate) mod fullscreen;
mod history;
mod modules;
pub(crate) mod motion;
mod network_status;
mod notice_hints;
mod notices;
pub mod popups;
pub mod settings;
mod settings_scroll;
mod visibility;
mod visualizer;
mod workspaces;
use motion::Spring;
const OSD_FOR: Duration = Duration::from_millis(2500);

const BAR_WIDTH: f32 = 100.0;
const BAR_HEIGHT: f32 = 6.0;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Idle,
    Osd,
    Panel,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Osd {
    Volume,
    Brightness,
}

impl Osd {
    fn label(self) -> &'static str {
        match self {
            Osd::Volume => "Volume",
            Osd::Brightness => "Brightness",
        }
    }
}

pub struct Island {
    // what the pill shows right now
    mode: Mode,
    previous_mode: Mode,
    transition: Instant,
    expanded: bool,
    leave_deadline: Option<Instant>,
    width: Spring,
    height: Spring,
    expansion: Spring,

    osd: Osd,
    percent: u8,
    muted: bool,
    osd_until: Instant,

    // the newest notification already shown, so each one pops up once

    // `None` until the first look, so starting the shell doesn't show the volume
    last_audio: Option<(u8, bool)>,
    last_brightness: Option<u8>,
}

impl Service for Island {
    fn new() -> Self {
        let (width, height) = IDLE;

        Self {
            mode: Mode::Idle,
            previous_mode: Mode::Idle,
            transition: Instant::now() - Duration::from_secs(1),
            expanded: false,
            leave_deadline: None,
            width: Spring::new(width),
            height: Spring::new(height),
            expansion: Spring::new(0.0),

            osd: Osd::Volume,
            percent: 0,
            muted: false,
            osd_until: Instant::now(),

            last_audio: None,
            last_brightness: None,
        }
    }

    // a poll that finds nothing costs nothing, so this can be quick
    fn interval() -> Duration {
        Duration::from_millis(50)
    }

    fn update(&mut self) -> bool {
        let mut changed = self.check_audio();

        // Notifications render independently below either island mode.

        let brightness = crate::ddc::percent();
        changed |= brightness != self.last_brightness;
        self.last_brightness = brightness;

        if self.expanded
            && !settings::open()
            && self
                .leave_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.expanded = false;
            self.leave_deadline = None;
            popups::dismiss();
            changed = true;
        }
        changed |= self.sync();

        changed
    }
}

impl Island {
    // brightness comes in through IPC, so ddc.rs calls this
    pub fn show_osd(&mut self, osd: Osd, percent: u8, muted: bool) {
        exposure::reveal();
        self.osd = osd;
        self.percent = percent;
        self.muted = muted;
        self.osd_until = Instant::now() + OSD_FOR;

        if self.expanded {
            popups::osd(osd);
        }
        self.sync();
    }

    fn check_audio(&mut self) -> bool {
        let (volume, muted) = {
            let audio = Audio::read();

            (audio.volume(), audio.muted())
        };

        let last = self.last_audio.replace((volume, muted));

        if last.is_none_or(|last| last == (volume, muted)) {
            return false;
        }

        self.show_osd(Osd::Volume, volume, muted);

        true
    }

    fn wanted_mode(&self) -> Mode {
        let now = Instant::now();

        if self.expanded {
            Mode::Panel
        } else if now < self.osd_until {
            Mode::Osd
        } else {
            Mode::Idle
        }
    }

    // moves the pill to what should be showing now, true when that changed
    fn sync(&mut self) -> bool {
        let mode = self.wanted_mode();

        let changed = mode != self.mode;
        if changed {
            self.previous_mode = self.mode;
            self.transition = Instant::now();
        }
        self.mode = mode;

        let (width, height) = match mode {
            Mode::Idle => IDLE,
            Mode::Osd => OSD,
            Mode::Panel => PANEL,
        };

        self.expansion
            .to(if mode == Mode::Panel { 1.0 } else { 0.0 });
        self.width.to(width);
        self.height.to(height);
        exposure::collapsed(hide_eligible(
            mode,
            self.previous_mode,
            self.transition.elapsed(),
        ));

        changed
    }
}

fn hide_eligible(mode: Mode, previous: Mode, elapsed: Duration) -> bool {
    mode != Mode::Panel && (previous != Mode::Panel || elapsed >= Duration::from_millis(620))
}

struct Clock {
    time: String,
    date: String,
}

impl Service for Clock {
    fn new() -> Self {
        Self {
            time: now(),
            date: date(),
        }
    }

    fn update(&mut self) -> bool {
        let time = now();

        let date = date();
        let changed = time != self.time || date != self.date;
        self.date = date;

        self.time = time;

        changed
    }
}

fn now() -> String {
    amane::output("date +%H:%M").trim().to_string()
}

fn date() -> String {
    amane::output("date +'%A, %d %B %Y · %Z'")
        .trim()
        .to_string()
}

fn content_motion(seconds: f32) -> (bool, f32, f32) {
    if seconds < 0.08 {
        let remaining = (1.0 - seconds / 0.08).clamp(0.0, 1.0);
        (true, remaining * remaining, 1.0)
    } else {
        let progress = ((seconds - 0.08) / 0.26).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - progress).powi(3);
        (false, eased, 1.0)
    }
}

fn blend_color(a: Color, b: Color, t: f32) -> Color {
    let mix = |x: u8, y: u8| {
        (f32::from(x) + (f32::from(y) - f32::from(x)) * t.clamp(0.0, 1.0)).round() as u8
    };
    Color::rgb(
        mix(a.red(), b.red()),
        mix(a.green(), b.green()),
        mix(a.blue(), b.blue()),
    )
}

fn icon_gradient(color: Color) -> amane::Gradient {
    amane::Gradient::linear(
        120.0,
        [
            (
                0.0,
                blend_color(color, crate::theme::palette().foreground, 0.20),
            ),
            (0.48, color),
            (1.0, blend_color(color, Color::BLACK, 0.25)),
        ],
    )
}

fn media_button(glyph: &str, action: impl Fn() + 'static, primary: bool) -> Rectangle {
    let key = format!("media:{glyph}");
    let ck = key.clone();
    let button = Rectangle::new()
        .width(32.0)
        .height(32.0)
        .scale(crate::feedback::scale(&key))
        .radius(Full)
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| {
            if matches!(button, Button::Left) {
                crate::feedback::click(&ck);
                action();
            }
        });
    if !primary {
        return button.fill(Color::TRANSPARENT).child(
            Text::new(glyph)
                .font("Material Icons Round")
                .size(23.0)
                .tight()
                .color(crate::theme::palette().foreground),
        );
    }
    // Explicit geometry avoids font side bearings/baselines. The triangle's
    // visual centroid, not its bounding-box center, is at the button center.
    let path = if glyph == "\u{e037}" {
        amane::Path::new()
            .move_to(10.8, 11.0)
            .quad_to(10.8, 7.5, 13.7, 9.2)
            .line_to(22.0, 13.8)
            .quad_to(25.8, 16.0, 22.0, 18.2)
            .line_to(13.7, 22.8)
            .quad_to(10.8, 24.5, 10.8, 21.0)
            .close()
    } else {
        // Capsule bars, with true semicircular ends rather than square corners.
        amane::Path::new()
            .move_to(12.0, 8.5)
            .cubic_to(13.4, 8.5, 14.5, 9.6, 14.5, 11.0)
            .line_to(14.5, 21.0)
            .cubic_to(14.5, 24.3, 9.5, 24.3, 9.5, 21.0)
            .line_to(9.5, 11.0)
            .cubic_to(9.5, 9.6, 10.6, 8.5, 12.0, 8.5)
            .close()
            .move_to(20.0, 8.5)
            .cubic_to(21.4, 8.5, 22.5, 9.6, 22.5, 11.0)
            .line_to(22.5, 21.0)
            .cubic_to(22.5, 24.3, 17.5, 24.3, 17.5, 21.0)
            .line_to(17.5, 11.0)
            .cubic_to(17.5, 9.6, 18.6, 8.5, 20.0, 8.5)
            .close()
    };
    use amane::Shape;
    // Keep a 32px hit target, with a 24px-high colored symbol and no disk.
    button.fill(Color::TRANSPARENT).child(
        Rectangle::new().width(32.0).height(32.0).scale(1.65).child(
            amane::Canvas::new()
                .width(32.0)
                .height(32.0)
                .shapes(vec![Box::new(path.fill(crate::theme::palette().green))]),
        ),
    )
}

pub fn view() -> LayerWindow {
    // A fullscreen app is up: keep the layer surface mapped but invisible.
    // Unmapping (attach NULL buffer) makes mango close the surface, which
    // removes the last window and shuts the whole shell down.
    if fullscreen::blocked() {
        return LayerWindow::new()
            .width(Full)
            .height(Full)
            .keyboard(amane::Keyboard::None)
            .input_region(vec![])
            .anchor_horizontal(Horizontal::Middle)
            .anchor_vertical(Vertical::Top)
            .namespace("island")
            .child(
                Rectangle::new()
                    .width(1.0)
                    .height(1.0)
                    .fill(Color::TRANSPARENT),
            );
    }
    let island = Island::read();
    let clock = Clock::read();

    let expansion = island.expansion.precise_value().clamp(0.0, 1.0);
    let missing = modules::unused_visualizer_width(false) * (1.0 - expansion)
        + modules::unused_visualizer_width(true) * expansion;
    let width = (island.width.value()
        - if matches!(island.mode, Mode::Idle | Mode::Panel) {
            missing
        } else {
            0.0
        })
    .max(48.0);
    let width = if settings::open() {
        width.max(PANEL.0)
    } else {
        width
    };
    let height = island.height.value();

    let elapsed = island.transition.elapsed().as_secs_f32();
    let (outgoing, opacity, scale) = content_motion(elapsed);
    if elapsed < 0.34 {
        amane::request_frame();
    }
    let mode = if outgoing {
        island.previous_mode
    } else {
        island.mode
    };
    let shared = matches!(island.mode, Mode::Idle | Mode::Panel);
    let content: Box<dyn amane::Widget> = if shared {
        Box::new(modules::view(&island, &clock, width, height))
    } else {
        let content_width = match mode {
            Mode::Idle => IDLE.0,
            Mode::Osd => OSD.0,
            Mode::Panel => PANEL.0,
        } - if mode == Mode::Panel {
            ROW_PADDING * 2.0
        } else {
            36.0
        };
        let content = Rectangle::new()
            .width(content_width)
            .height(height)
            .align_child(Center, Center)
            .opacity(opacity)
            .scale(scale);
        let content = match mode {
            Mode::Idle => content.cursor(Pointer).child(
                Text::new(&clock.time)
                    .size(16.0)
                    .weight(Weight::SemiBold)
                    .color(crate::theme::palette().foreground),
            ),
            Mode::Osd => content.child(osd(&island)),
            Mode::Panel => content.child(modules::view(&island, &clock, content_width, height)),
        };
        let blur = 0.0;
        let content = amane::Stack::new(children![
            content,
            Rectangle::new()
                .width(content_width)
                .height(height)
                .blur(blur)
                .fill(Color::TRANSPARENT),
        ])
        .width(content_width)
        .height(height);
        Box::new(content)
    };
    let row_height = height;
    let extra = if island.mode == Mode::Panel {
        settings::height()
    } else {
        0.0
    };
    let row_content = amane::Stack::new(vec![content])
        .width(width)
        .height(row_height);
    let content: Box<dyn amane::Widget> = Box::new(
        amane::Stack::new(children![
            row_content,
            settings::view().translate((width - PANEL.0) / 2.0, row_height)
        ])
        .width(width)
        .height(row_height + extra),
    );
    let height = height + extra;
    let pill = pill(width, height)
        .padding(Padding {
            left: if shared {
                0.0
            } else if mode == Mode::Panel {
                ROW_PADDING
            } else {
                18.0
            },
            right: if shared {
                0.0
            } else if mode == Mode::Panel {
                ROW_PADDING
            } else {
                18.0
            },
            top: 0.0,
            bottom: 0.0,
        })
        .child(amane::Stack::new(vec![content]).width(width).height(height));

    let scene = popups::scene(width, height, island.mode == Mode::Panel);
    let hide = exposure::amount();
    let shift = GAP_TOP - hide * (height + GAP_TOP + 8.0);
    let scene = notices::decorate(scene, width, height, shift);
    // A stable output-sized surface keeps compositor-local pointer coordinates
    // fixed while the island grows or popups alter the scene envelope.
    let (screen_width, screen_height) = amane::window_size();
    let screen_width = screen_width.max(PANEL.0 + 72.0);
    let screen_height = screen_height.max(600.0);
    let canvas_width = scene.width;
    let dx = (screen_width - canvas_width) / 2.0;
    let mut input = scene.input;
    for area in &mut input {
        area.x += dx.round() as i32;
        area.y += (shift - scene.top).round() as i32;
    }
    let sensor_width = screen_width / 2.0;
    let sensor_x = (screen_width - sensor_width) / 2.0;
    if crate::edge_activation::enabled("main") && (exposure::occupied() || exposure::tucked()) {
        input.push(amane::InputArea {
            x: sensor_x as i32,
            y: 0,
            width: sensor_width as i32,
            height: border::THICKNESS as i32,
        });
    }
    let pill = pill.translate(((canvas_width - width) / 2.0).round(), scene.top);
    let main_modal = island.expanded || settings::open();
    if main_modal {
        input.push(amane::InputArea {
            x: 0,
            y: 0,
            width: screen_width as i32,
            height: screen_height as i32,
        });
    }
    let moving = Rectangle::new()
        .width(canvas_width)
        .height(scene.height)
        .on_click(|_| {})
        .translate(dx, shift - scene.top)
        .child(
            amane::Stack::new(vec![scene.background, Box::new(pill), scene.content])
                .width(canvas_width)
                .height(scene.height),
        );
    let sensor = Rectangle::new()
        .width(sensor_width)
        .height(border::THICKNESS)
        .translate(sensor_x, 0.0)
        .fill(Color::TRANSPARENT)
        .on_hover(|inside| {
            if crate::edge_activation::enabled("main") {
                exposure::pointer(inside)
            }
        });
    let outside = Rectangle::new()
        .width(if main_modal { screen_width } else { 0.0 })
        .height(if main_modal { screen_height } else { 0.0 })
        .fill(Color::TRANSPARENT)
        .on_click(|_| dismiss_outside());
    let layer = amane::Stack::new(children![
        outside,
        border::frame(screen_width, screen_height),
        border::merge(screen_width, width, height, shift, hide),
        moving,
        sensor,
    ])
    .width(screen_width)
    .height(screen_height);
    LayerWindow::new()
        .width(Full)
        .height(Full)
        .keyboard(if settings::keyboard() {
            amane::Keyboard::OnDemand
        } else {
            amane::Keyboard::None
        })
        .on_key(|key| {
            if key == amane::Key::Escape {
                settings::close()
            }
        })
        .input_region(input)
        .anchor_horizontal(Horizontal::Middle)
        .anchor_vertical(Vertical::Top)
        .namespace("island")
        .child(layer)
}

fn pill(width: f32, height: f32) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(height)
        .radius((height / 2.0).min(34.0))
        .fill(Color::TRANSPARENT)
        .padding(Padding {
            top: 0.0,
            right: 18.0,
            bottom: 0.0,
            left: 18.0,
        })
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(clicked)
        .on_hover(pointer_presence)
}

fn osd(island: &Island) -> Row {
    let (value, fraction) = if island.muted {
        (String::from("Muted"), 0.0)
    } else {
        (
            format!("{}%", island.percent),
            f32::from(island.percent) / 100.0,
        )
    };

    // fixed-width cells keep the bar from shifting when the text changes
    Row::new(children![
        cell(
            76.0,
            Start,
            Text::new(island.osd.label())
                .size(15.0)
                .weight(Weight::Medium)
                .color(crate::theme::palette().secondary)
        ),
        bar(fraction),
        cell(
            44.0,
            End,
            Text::new(value)
                .size(15.0)
                .weight(Weight::SemiBold)
                .color(crate::theme::palette().foreground)
        ),
    ])
    .gap(10.0)
    .align(Center)
}

fn cell(width: f32, align: impl Into<Align>, content: impl amane::Widget + 'static) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(20.0)
        .align_child(align, Center)
        .child(content)
}

fn bar(fraction: f32) -> Rectangle {
    let track = Rectangle::new()
        .width(BAR_WIDTH)
        .height(BAR_HEIGHT)
        .radius(Full)
        .fill(crate::theme::palette().surface);

    let filled = BAR_WIDTH * fraction.clamp(0.0, 1.0);

    // a sliver narrower than a pixel isn't worth drawing
    if filled < 1.0 {
        return track;
    }

    track.child(
        Rectangle::new()
            .width(filled)
            .height(BAR_HEIGHT)
            .radius(Full)
            .fill(crate::theme::palette().blue),
    )
}

// Child popup hover handlers call this too, so moving into a popup is not a leave.
fn pointer_presence(inside: bool) {
    exposure::pointer(inside);
    let mut state = Island::write();
    state.leave_deadline = if inside || !state.expanded {
        None
    } else {
        Some(Instant::now() + Duration::from_secs(3))
    };
}

fn dismiss_outside() {
    popups::dismiss();
    settings::close();
    let mut state = Island::write();
    state.expanded = false;
    state.leave_deadline = None;
    state.osd_until = Instant::now();
    state.sync();
    exposure::hide();
}
fn clicked(button: Button) {
    if !matches!(button, Button::Left) {
        return;
    }
    popups::dismiss();
    settings::close();
    let mut island = Island::write();
    island.expanded = !island.expanded;
    island.leave_deadline = if island.expanded {
        Some(Instant::now() + Duration::from_secs(3))
    } else {
        None
    };
    island.osd_until = Instant::now();
    island.sync();
}

// IPC and panel buttons use the same controls. Audio changes are asynchronous;
// check_audio displays the value actually reported by the sound server.
pub fn volume(arguments: &[String]) -> String {
    let Some(argument) = arguments.first() else {
        return String::from("usage: volume up|down|mute|show|<0-100>");
    };
    let (current, muted) = {
        let audio = Audio::read();
        (audio.volume(), audio.muted())
    };
    match argument.as_str() {
        "show" => Island::write().show_osd(Osd::Volume, current, muted),
        "mute" => Audio::toggle_mute(),
        value => {
            let next = match value {
                "up" => i32::from(current) + 5,
                "down" => i32::from(current) - 5,
                number => match number.parse::<i32>() {
                    Ok(value) => value,
                    Err(_) => return format!("bad volume argument: {number}"),
                },
            }
            .clamp(0, 100) as u8;
            Audio::set_volume(next);
            // Show even at the limit, where no audio change event is emitted.
            Island::write().show_osd(Osd::Volume, current, muted);
        }
    }
    String::from("ok")
}

pub fn media(arguments: &[String]) -> String {
    match arguments.first().map(String::as_str) {
        Some("play-pause") => Media::play_pause(),
        Some("next") => Media::next(),
        Some("previous") => Media::previous(),
        _ => return String::from("usage: media play-pause|next|previous"),
    }
    String::from("ok")
}

pub fn toggle(arguments: &[String]) -> String {
    if arguments
        .first()
        .is_some_and(|a| a == "reveal" || a == "show")
    {
        exposure::reveal();
        return "ok".into();
    }
    let hidden = exposure::tucked();
    popups::dismiss();
    settings::close();
    let mut island = Island::write();
    if hidden {
        island.expanded = false;
        exposure::reveal();
    } else if island.expanded {
        island.expanded = false;
        exposure::hide();
    } else {
        island.expanded = true;
        exposure::reveal();
    }
    island.leave_deadline = None;
    island.osd_until = Instant::now();
    island.sync();
    "ok".into()
}

/// Tucks the island away so the colour picker's capture sees the desktop, not
/// the shell. The reveal sensor and IPC still bring it back afterwards.
pub fn hide_for_picker() {
    popups::dismiss();
    settings::close();
    let mut island = Island::write();
    island.expanded = false;
    island.leave_deadline = None;
    island.osd_until = Instant::now();
    island.sync();
    exposure::hide();
}

fn adjust(control: Osd, direction: &str) {
    let arguments = [direction.to_string()];
    match control {
        Osd::Volume => {
            volume(&arguments);
        }
        Osd::Brightness => {
            let reply = crate::ddc::brightness(&arguments);
            if crate::ddc::percent().is_none() {
                eprintln!("{reply}");
            }
        }
    }
}

// A single slim status row. Each section has a fixed budget so metadata
// changes never push the audio/brightness controls off the island.
fn reveal_slider(control: Osd) {
    popups::show(match control {
        Osd::Volume => popups::Popup::Volume,
        Osd::Brightness => popups::Popup::Brightness,
    });
}

fn control_color(control: Osd) -> Color {
    match control {
        Osd::Volume => crate::theme::palette().blue,
        Osd::Brightness => crate::theme::palette().yellow,
    }
}

// Third control: opens the HSV color-picker popup next to volume/brightness.
fn palette_icon() -> Rectangle {
    let key = "control:palette";
    Rectangle::new()
        .width(CONTROL_WIDTH)
        .height(36.0)
        .scale(crate::feedback::scale(key))
        .radius(Full)
        .fill(crate::theme::surface())
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_hover(|entered| popups::hover(popups::Popup::Color, entered))
        .on_click(move |button| {
            if matches!(button, Button::Left) {
                crate::feedback::click(key);
                popups::show(popups::Popup::Color);
            }
        })
        .child(
            Text::new("\u{e40a}") // palette
                .font("Material Icons Round")
                .size(23.0)
                .weight(Weight::Bold)
                .color(crate::theme::palette().cyan),
        )
}

fn volume_glyph(level: u8, muted: bool) -> &'static str {
    match (muted, level) {
        (true, _) => "\u{e04f}",
        (_, 0) => "\u{e04e}",
        (_, 1..=49) => "\u{e04d}",
        _ => "\u{e050}",
    }
}

fn control_icon(control: Osd, muted: bool, selected: bool) -> Rectangle {
    let glyph = match control {
        Osd::Volume => volume_glyph(Audio::read().volume(), muted),
        Osd::Brightness => "\u{e1ac}", // brightness_6
    };
    let key = if control == Osd::Volume {
        "control:volume"
    } else {
        "control:brightness"
    };
    Rectangle::new()
        .width(CONTROL_WIDTH)
        .height(36.0)
        .scale(crate::feedback::scale(key))
        .radius(Full)
        .fill(if selected {
            crate::theme::palette().surface
        } else {
            crate::theme::surface()
        })
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_hover(move |entered| {
            popups::hover(
                match control {
                    Osd::Volume => popups::Popup::Volume,
                    Osd::Brightness => popups::Popup::Brightness,
                },
                entered,
            )
        })
        .on_click(move |button| match button {
            Button::Left => {
                crate::feedback::click(key);
                reveal_slider(control)
            }
            Button::Right if control == Osd::Volume => Audio::toggle_mute(),
            _ => {}
        })
        .on_scroll(move |scroll| {
            if scroll.y != 0.0 {
                reveal_slider(control);
                adjust(control, if scroll.y < 0.0 { "up" } else { "down" });
            }
        })
        .child(
            Text::new(glyph)
                .font("Material Icons Round")
                .size(23.0)
                .weight(Weight::Bold)
                .color(control_color(control)),
        )
}

fn row_cell(
    width: f32,
    align: impl Into<Align>,
    content: impl amane::Widget + 'static,
) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(36.0)
        .align_child(align, Center)
        .child(content)
}

fn track_time(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn row_transport(media: &Media) -> Rectangle {
    let times = Text::new(format!(
        "{} / {}",
        track_time(media.position()),
        track_time(media.length())
    ))
    .size(12.0)
    .weight(Weight::SemiBold)
    .color(crate::theme::palette().foreground);
    let controls = Row::new(children![
        media_button("\u{e045}", Media::previous, false).fill(Color::TRANSPARENT),
        media_button(
            if media.playing() {
                "\u{e034}"
            } else {
                "\u{e037}"
            },
            Media::play_pause,
            true
        ),
        media_button("\u{e044}", Media::next, false).fill(Color::TRANSPARENT),
    ])
    .gap(0.0)
    .align(Center);
    let transport = Rectangle::new()
        .width(96.0)
        .height(36.0)
        .align_child(Center, Center)
        .child(controls);
    Rectangle::new()
        .width(TRANSPORT_WIDTH)
        .height(36.0)
        .align_child(Center, Center)
        .on_hover(|inside| popups::hover(popups::Popup::Media, inside))
        .child(
            Row::new(children![
                Rectangle::new()
                    .width(76.0)
                    .height(36.0)
                    .align_child(End, Center)
                    .child(times),
                transport,
            ])
            .gap(8.0)
            .align(Center),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_never_hides_and_collapse_must_finish_first() {
        assert!(!hide_eligible(
            Mode::Panel,
            Mode::Idle,
            Duration::from_secs(10)
        ));
        assert!(!hide_eligible(
            Mode::Idle,
            Mode::Panel,
            Duration::from_millis(619)
        ));
        assert!(hide_eligible(
            Mode::Idle,
            Mode::Panel,
            Duration::from_millis(620)
        ));
    }
    #[test]
    fn volume_symbols_follow_mute_silence_low_and_high_levels() {
        assert_eq!(volume_glyph(80, true), "\u{e04f}");
        assert_eq!(volume_glyph(0, false), "\u{e04e}");
        assert_eq!(volume_glyph(1, false), volume_glyph(49, false));
        assert_ne!(volume_glyph(49, false), volume_glyph(50, false));
    }
    #[test]
    fn row_track_times_use_minutes_and_padded_seconds() {
        assert_eq!(track_time(Duration::ZERO), "0:00");
        assert_eq!(track_time(Duration::from_secs(80)), "1:20");
        assert_eq!(track_time(Duration::from_secs(150)), "2:30");
    }

    #[test]
    fn content_transition_fades_out_then_returns_to_crisp_rest() {
        assert_eq!(content_motion(0.0), (true, 1.0, 1.0));
        assert!(content_motion(0.07).1 < 0.05);
        assert_eq!(content_motion(0.08).1, 0.0);
        assert!(content_motion(0.26).1 > 0.5);
        assert_eq!(content_motion(0.5), (false, 1.0, 1.0));
    }

    #[test]
    fn starts_collapsed_without_an_osd() {
        let island = Island::new();
        assert!(island.wanted_mode() == Mode::Idle);
        assert_eq!(island.width.value(), IDLE.0);
        assert_eq!(island.height.value(), IDLE.1);
    }

    #[test]
    fn opened_panel_remains_open_during_osd_changes() {
        let mut island = Island::new();
        island.expanded = true;
        island.show_osd(Osd::Volume, 75, false);
        assert!(island.mode == Mode::Panel);
        assert_eq!(island.percent, 75);
        island.expanded = false;
        island.sync();
        assert!(island.mode == Mode::Osd);
        island.osd_until = Instant::now();
        island.sync();
        assert!(island.mode == Mode::Idle);
    }

    #[test]
    fn osd_returns_to_collapsed_without_replacing_notification_stack() {
        let mut island = Island::new();
        island.osd_until = Instant::now() + Duration::from_secs(1);
        assert!(island.wanted_mode() == Mode::Osd);
        island.osd_until = Instant::now();
        assert!(island.wanted_mode() == Mode::Idle);
    }
}
