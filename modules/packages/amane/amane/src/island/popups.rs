//! Shared background field: popup shapes originate inside their row controls.
use super::*;
use amane::{Animation, Easing, Gradient};

const POPUP_PADDING: f32 = 20.0;
const POPUP_GAP: f32 = 12.0;
pub(super) const BLOB_MS: u64 = 420;

#[derive(Clone, Copy, PartialEq)]
pub enum Popup {
    Calendar,
    Media,
    Volume,
    Brightness,
    Color,
    #[allow(dead_code)] // retained renderer compatibility; activation is removed
    Workspace(i64),
}

struct Hover {
    kind: Popup,
    open: bool,
    deadline: Option<Instant>,
    motion: Animation,
    source: (f32, f32, f32, f32),
    box_x: Spring,
    box_w: Spring,
    box_h: Spring,
    closed: Instant,
    calendar: Vec<String>,
    month: String,
    today: u8,
    volume: Animation,
    brightness: Animation,
    last_levels: (u8, u8),
    calendar_day: String,
    month_offset: i32,
    /// HSV picker state: hue in [0, 360), saturation/value in [0, 1].
    hue: f32,
    saturation: f32,
    value: f32,
    /// The format last copied and when, so only that row shows "Copied".
    copied: Option<(ColorFormat, Instant)>,
}

impl Service for Hover {
    fn new() -> Self {
        let (calendar_day, month, today, calendar) = local_calendar();
        let (hue, saturation, value) = rgb_to_hsv(crate::theme::palette().blue);
        Self {
            kind: Popup::Calendar,
            source: origin(Popup::Calendar),
            box_x: Spring::new(geometry(Popup::Calendar, calendar.len()).2),
            box_w: Spring::new(geometry(Popup::Calendar, calendar.len()).0),
            box_h: Spring::new(geometry(Popup::Calendar, calendar.len()).1),
            open: false,
            deadline: None,
            motion: Animation::new(0.0)
                .duration(Duration::from_millis(BLOB_MS))
                .easing(Easing::Linear),
            closed: Instant::now() - Duration::from_secs(1),
            calendar,
            month,
            today,
            calendar_day,
            month_offset: 0,
            last_levels: (0, 0),
            volume: Animation::new(0.0).duration(Duration::from_millis(220)),
            brightness: Animation::new(0.0).duration(Duration::from_millis(260)),
            hue,
            saturation,
            value,
            copied: None,
        }
    }
    fn interval() -> Duration {
        Duration::from_millis(30)
    }
    fn update(&mut self) -> bool {
        if fullscreen::hidden() && self.open {
            self.open = false;
            self.closed = Instant::now() - Duration::from_secs(1);
            self.motion = Animation::new(0.0)
                .duration(Duration::from_millis(BLOB_MS))
                .easing(Easing::Linear);
            return true;
        }
        let (w, h, x) = geometry(self.kind, self.calendar.len());
        self.box_x.to(x);
        self.box_w.to(w);
        self.box_h.to(h);
        let audio = Audio::read();
        let volume = if audio.muted() { 0 } else { audio.volume() };
        let brightness = crate::ddc::percent().unwrap_or(0);
        let changed = self.last_levels != (volume, brightness);
        self.last_levels = (volume, brightness);
        self.volume.to(f32::from(volume));
        self.brightness.to(f32::from(brightness));
        drop(audio);
        if self.open
            && self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.open = false;
            self.closed = Instant::now();
            self.motion.to(0.0);
            return true;
        }
        // The copied badge clears itself without any pointer activity.
        let copied_cleared = self
            .copied
            .take_if(|(_, at)| at.elapsed() >= Duration::from_millis(1500))
            .is_some();
        changed || copied_cleared
    }
}

fn switch(state: &mut Hover, kind: Popup) {
    state.source = origin(kind);
    if state.kind == kind {
        return;
    }
    let (w, h, x) = geometry(kind, state.calendar.len());
    if state.motion.value() > 0.01 {
        // Keep the current body alive and continuously retarget its rectangle.
        state.box_x.to(x);
        state.box_w.to(w);
        state.box_h.to(h);
    } else {
        state.source = origin(kind);
        state.box_x = Spring::new(x);
        state.box_w = Spring::new(w);
        state.box_h = Spring::new(h);
    }
    state.kind = kind;
}

// OSD events already hold Island's write lock: do not route through hover.
pub(super) fn osd(control: Osd) {
    let kind = if control == Osd::Volume {
        Popup::Volume
    } else {
        Popup::Brightness
    };
    exposure::reveal();
    let mut state = Hover::write();
    switch(&mut state, kind);
    if state.motion.value() < 0.01 {
        state.source = origin(kind);
    }
    state.open = true;
    state.deadline = Some(Instant::now() + Duration::from_secs(2));
    state.motion.to(1.0);
}

pub fn show(kind: Popup) {
    exposure::reveal();
    hover(kind, true);
    pointer_presence(false);
}

/// Opens the colour popup preset to `color`, so the picker can hand a sampled
/// pixel over for the user to copy as hex or rgb. Expands a collapsed island
/// (the picker tucked it away) so the popup is actually visible.
pub fn color_at(color: Color) {
    {
        let mut state = Hover::write();
        let (hue, saturation, value) = rgb_to_hsv(color);
        state.hue = hue;
        state.saturation = saturation;
        state.value = value;
        state.copied = None;
    }
    {
        let mut island = Island::write();
        island.expanded = true;
        island.leave_deadline = None;
        island.osd_until = Instant::now();
        island.sync();
    }
    exposure::reveal();
    show(Popup::Color);
}
pub fn hover(kind: Popup, inside: bool) {
    if matches!(kind, Popup::Workspace(_)) {
        return;
    }
    pointer_presence(inside);
    if inside && (fullscreen::hidden() || !Island::read().expanded) {
        return;
    }
    let mut state = Hover::write();
    if inside {
        switch(&mut state, kind);
        if state.motion.value() < 0.01 {
            state.source = origin(kind);
        }
        if kind == Popup::Calendar && (!state.open || state.kind != kind) {
            let (day, month, today, calendar) = local_calendar();
            if state.calendar_day != day {
                state.month_offset = 0;
                state.calendar_day = day;
                state.month = month;
                state.today = today;
                state.calendar = calendar;
            }
        }
        state.kind = kind;
        state.open = true;
        state.deadline = None;
        state.motion.to(1.0);
    } else if state.kind == kind {
        // A bridge across the small physical gap between separate surfaces.
        state.deadline = Some(Instant::now() + Duration::from_millis(300));
    }
}
pub fn dismiss() {
    let mut state = Hover::write();
    state.open = false;
    state.closed = Instant::now();
    state.motion.to(0.0);
}

fn icon(glyph: &str, color: Color, size: f32) -> Text {
    Text::new(glyph)
        .font("Material Icons Round")
        .size(size)
        .tight()
        .color(color)
}

/// Anchors are derived from the exact row budgets, never hand-tuned offsets.
fn anchor(kind: Popup) -> f32 {
    let controls_start = modules::position(modules::Module::Controls, true).unwrap_or(ROW_PADDING);
    let clock_start = modules::position(modules::Module::Time, true).unwrap_or(ROW_PADDING);
    let media_start = modules::position(modules::Module::Media, true).unwrap_or(ROW_PADDING);
    match kind {
        Popup::Workspace(id) => {
            let (x, w) = workspaces::bounds(id);
            x + w / 2.0
        }
        Popup::Calendar => clock_start + CLOCK_WIDTH / 2.0,
        Popup::Media => media_start + TRANSPORT_WIDTH / 2.0,
        Popup::Volume => controls_start + CONTROL_WIDTH / 2.0,
        Popup::Brightness => controls_start + CONTROL_WIDTH + CONTROL_GAP + CONTROL_WIDTH / 2.0,
        Popup::Color => {
            controls_start + (CONTROL_WIDTH + CONTROL_GAP) * 2.0 + CONTROL_WIDTH / 2.0
        }
    }
}

fn geometry(kind: Popup, weeks: usize) -> (f32, f32, f32) {
    let (width, height) = match kind {
        Popup::Calendar => (
            328.0,
            40.0 + 28.0 + 24.0 + weeks as f32 * 32.0 + (weeks as f32 + 1.0) * 8.0,
        ),
        Popup::Media => (352.0, 204.0),
        Popup::Workspace(id) => (328.0, workspaces::grid_height(id)),
        Popup::Volume | Popup::Brightness => (88.0, 40.0 + 154.0 + 24.0 + 20.0 + 16.0),
        Popup::Color => (
            PICKER_WIDTH + POPUP_PADDING * 2.0,
            PICKER_SQUARE_H
                + PICKER_GAP
                + PICKER_HUE_H
                + PICKER_GAP
                + PICKER_ROW_H
                + PICKER_GAP
                + PICKER_ROW_H
                + PICKER_GAP
                + PICKER_ROW_H
                + POPUP_PADDING * 2.0,
        ),
    };
    let x = (anchor(kind) - width / 2.0)
        .clamp(ROW_PADDING, PANEL.0 - ROW_PADDING - width)
        .round();
    (width, height, x)
}

// A single surface owns the whole shared alpha field. No layer boundary cuts
// off the inside of the island, and no hand-drawn neck is involved.
pub(super) struct Scene {
    pub width: f32,
    pub height: f32,
    pub top: f32,
    pub background: Box<dyn amane::Widget>,
    pub content: Box<dyn amane::Widget>,
    pub input: Vec<amane::InputArea>,
}

pub(super) fn blob_growth(p: f32) -> f32 {
    if p <= 0.0 {
        return 0.0;
    }
    if p >= 1.0 {
        return 1.0;
    }
    // Underdamped step: 5.3% overshoot, then exactly stationary at 480ms.
    let t = p * 0.48;
    let damping: f32 = 0.68;
    let frequency = 24.0 * (1.0 - damping * damping).sqrt();
    let decay = damping * 24.0;
    1.0 - (-decay * t).exp() * ((frequency * t).cos() + decay / frequency * (frequency * t).sin())
}

fn origin(kind: Popup) -> (f32, f32, f32, f32) {
    let width = match kind {
        Popup::Calendar => CLOCK_WIDTH,
        Popup::Workspace(id) => workspaces::bounds(id).1,
        Popup::Media => TRANSPORT_WIDTH,
        Popup::Volume | Popup::Brightness | Popup::Color => CONTROL_WIDTH,
    };
    (
        anchor(kind) - width / 2.0,
        (PANEL.1 - 36.0) / 2.0,
        width,
        36.0,
    )
}

pub(super) fn scene(bar_width: f32, bar_height: f32, expanded: bool) -> Scene {
    let state = Hover::read();
    let hidden = fullscreen::hidden();
    let active = expanded
        && !hidden
        && (state.open || state.closed.elapsed() < Duration::from_millis(BLOB_MS + 20));
    let p = if active {
        state.motion.value().clamp(0.0, 1.0)
    } else {
        0.0
    };
    let width = state.box_w.precise_value();
    let height = state.box_h.precise_value();
    let layout_shift = modules::unused_visualizer_width(true) / 2.0;
    let x = state.box_x.precise_value() + layout_shift;
    let padding = if active { 36.0 } else { 0.0 };
    let top = padding;
    let canvas_width = if active {
        PANEL.0 + padding * 2.0
    } else {
        bar_width
    };
    // Reserve the entire transition envelope up front, keeping the center and
    // content origins fixed while only the mask deforms.
    let canvas_height = if active {
        top + bar_height + POPUP_GAP + 400.0 + padding
    } else {
        bar_height
    };
    let bar_x = ((canvas_width - bar_width) / 2.0).round();
    let mut input = vec![amane::InputArea {
        x: bar_x as i32,
        y: top as i32,
        width: bar_width as i32,
        height: bar_height as i32,
    }];
    let empty = || Box::new(Rectangle::new().width(0.0).height(0.0)) as Box<dyn amane::Widget>;
    if !active {
        return Scene {
            width: canvas_width,
            height: canvas_height,
            top,
            background: Box::new(
                Rectangle::new()
                    .width(bar_width)
                    .height(bar_height)
                    .radius((bar_height / 2.0).min(34.0))
                    .fill(crate::theme::surface()),
            ),
            content: empty(),
            input,
        };
    }
    if !state.open {
        amane::request_frame();
    }
    let growth = blob_growth(p);
    // Source follows the currently selected control, even after retargeting
    // a box already onscreen. Dismissal must not reuse the first popup's anchor.
    let (ox, oy, ow, oh) = origin(state.kind);
    let ox = ox + layout_shift;
    let body_width = ow + (width - ow) * growth;
    let body_height = oh + (height - oh) * growth;
    let center_x = ox + ow / 2.0 + (x + width / 2.0 - ox - ow / 2.0) * growth;
    let center_y = oy + oh / 2.0 + (bar_height + POPUP_GAP + height / 2.0 - oy - oh / 2.0) * growth;
    let body = [
        padding + center_x - body_width / 2.0,
        top + center_y - body_height / 2.0,
        body_width,
        body_height,
    ];
    // Blur is strongest while shapes are pulling apart, resolves to an exact
    // rounded rectangle at rest. Levels controls the shared alpha isocontour.
    let sigma = 11.0 * (1.0 - ((p - 0.55) / 0.25).clamp(0.0, 1.0));
    let background = Rectangle::new().width(canvas_width).height(canvas_height);
    // file!() points to the targeted source, unlike the generated Cargo project.
    let shader_path = std::path::Path::new(file!())
        .parent()
        .unwrap()
        .join("goo.wgsl");
    let background = background
        .shader(shader_path)
        .shader_values(crate::theme::shader_values(vec![
            [bar_x, top, bar_width, bar_height],
            body,
            [
                (bar_height / 2.0).min(34.0),
                popup_radius(body_width, body_height, p),
                sigma,
                0.48,
            ],
        ]));
    let opacity = ((p - 0.57) / 0.25).clamp(0.0, 1.0);
    let kind = state.kind;
    let popup = match kind {
        Popup::Calendar => calendar(&state),
        Popup::Workspace(id) => workspaces::grid(id),
        Popup::Media => media_widget(),
        Popup::Volume => vertical(Osd::Volume, state.volume.value()),
        Popup::Brightness => vertical(Osd::Brightness, state.brightness.value()),
        Popup::Color => color_picker(&state),
    };
    // Readable content is drawn last, independently of the moving alpha mask.
    let content = Rectangle::new()
        .width(width)
        .height(height)
        .padding(POPUP_PADDING)
        .translate(padding + x, top + bar_height + POPUP_GAP)
        .opacity(opacity)
        .clip()
        .on_hover(move |inside| hover(kind, inside))
        .child(popup);
    // Continuous input envelope too: moving across the gap never drops the
    // surface or makes the shape flicker between isolated regions.
    input = vec![amane::InputArea {
        x: padding as i32,
        y: top as i32,
        width: PANEL.0 as i32,
        height: (bar_height + POPUP_GAP + height) as i32,
    }];
    Scene {
        width: canvas_width,
        height: canvas_height,
        top,
        background: Box::new(background),
        content: Box::new(content),
        input,
    }
}

fn calendar(state: &Hover) -> Column {
    let nav = |glyph: &str, delta: i32| {
        let key = format!("calendar:{delta}");
        let ck = key.clone();
        Rectangle::new()
            .width(32.0)
            .height(28.0)
            .scale(crate::feedback::scale(&key))
            .radius(Full)
            .fill(crate::theme::palette().surface)
            .align_child(Center, Center)
            .cursor(Pointer)
            .on_click(move |button| {
                if matches!(button, Button::Left) {
                    crate::feedback::click(&ck);
                    change_month(delta);
                }
            })
            .child(icon(glyph, crate::theme::palette().foreground, 22.0))
    };
    let mut rows = children![
        Row::new(children![
            nav("\u{e5cb}", -1),
            Rectangle::new()
                .width(208.0)
                .height(28.0)
                .align_child(Center, Center)
                .child(
                    Text::new(&state.month)
                        .size(20.0)
                        .weight(Weight::SemiBold)
                        .color(crate::theme::palette().foreground)
                ),
            nav("\u{e5cc}", 1),
        ])
        .gap(8.0)
        .align(Center)
    ];
    let mut days = Vec::new();
    for name in ["M", "T", "W", "T", "F", "S", "S"] {
        days.push(Box::new(cell(
            40.0,
            Center,
            Text::new(name)
                .size(13.0)
                .weight(Weight::SemiBold)
                .color(crate::theme::palette().secondary),
        )) as Box<dyn amane::Widget>);
    }
    rows.push(Box::new(Row::new(days).gap(1.0)));
    for line in &state.calendar {
        let mut cells: Vec<Box<dyn amane::Widget>> = Vec::new();
        for index in 0..7 {
            let day = line
                .get(index * 3..(index * 3 + 2).min(line.len()))
                .unwrap_or("")
                .trim();
            let current = day.parse::<u8>().ok() == Some(state.today);
            cells.push(Box::new(
                Rectangle::new()
                    .width(40.0)
                    .height(32.0)
                    .radius(Full)
                    .fill(if current {
                        crate::theme::palette().blue
                    } else {
                        Color::BLACK
                    })
                    .align_child(Center, Center)
                    .child(
                        Text::new(day)
                            .size(15.0)
                            .weight(Weight::SemiBold)
                            .color(if current {
                                Color::BLACK
                            } else {
                                crate::theme::palette().foreground
                            }),
                    ),
            ));
        }
        rows.push(Box::new(Row::new(cells).gap(1.0)));
    }
    Column::new(rows).width(288.0).gap(8.0).align(Center)
}

fn media_widget() -> Column {
    let media = Media::read();
    let artwork = Rectangle::new()
        .width(72.0)
        .height(72.0)
        .radius(18.0)
        .fill(Gradient::linear(
            35.0,
            [
                (0.0, crate::theme::palette().blue),
                (0.5, crate::theme::palette().green),
                (1.0, crate::theme::palette().yellow),
            ],
        ))
        .align_child(Center, Center)
        .child(icon("\u{e405}", Color::BLACK, 36.0));
    let title = if media.title().is_empty() {
        "Nothing playing"
    } else {
        media.title()
    };
    let artist = if media.artist().is_empty() {
        "Now Playing"
    } else {
        media.artist()
    };
    let progress = if media.length().is_zero() {
        0.0
    } else {
        media.position().as_secs_f32() / media.length().as_secs_f32()
    };
    let time =
        |duration: Duration| format!("{}:{:02}", duration.as_secs() / 60, duration.as_secs() % 60);
    let metadata = Column::new(children![
        Text::new(title)
            .size(18.0)
            .weight(Weight::SemiBold)
            .color(crate::theme::palette().foreground)
            .elide()
            .max_lines(2),
        Text::new(artist)
            .size(14.0)
            .weight(Weight::Medium)
            .color(crate::theme::palette().secondary)
            .elide(),
    ])
    .width(224.0)
    .gap(8.0);
    let header = Row::new(children![artwork, metadata])
        .gap(16.0)
        .align(Center);
    let timeline = Column::new(children![
        Rectangle::new()
            .width(312.0)
            .height(8.0)
            .radius(Full)
            .fill(crate::theme::palette().surface)
            .child(
                Rectangle::new()
                    .width((312.0 * progress.clamp(0.0, 1.0)).max(1.0))
                    .height(8.0)
                    .radius(Full)
                    .fill(crate::theme::palette().blue)
            ),
        Row::new(children![
            cell(
                156.0,
                Start,
                Text::new(time(media.position()))
                    .size(13.0)
                    .color(crate::theme::palette().secondary)
            ),
            cell(
                156.0,
                End,
                Text::new(time(media.length()))
                    .size(13.0)
                    .color(crate::theme::palette().secondary)
            )
        ]),
    ])
    .gap(8.0);
    let controls = Rectangle::new()
        .width(312.0)
        .height(36.0)
        .align_child(Center, Center)
        .child(
            Row::new(children![
                media_button("\u{e045}", Media::previous, false),
                media_button(
                    if media.playing() {
                        "\u{e034}"
                    } else {
                        "\u{e037}"
                    },
                    Media::play_pause,
                    true
                ),
                media_button("\u{e044}", Media::next, false),
            ])
            .gap(8.0)
            .align(Center),
        );
    Column::new(children![header, timeline, controls])
        .width(312.0)
        .gap(10.0)
}

fn vertical(control: Osd, animated: f32) -> Column {
    let audio = Audio::read();
    let percent = match control {
        Osd::Volume => {
            if audio.muted() {
                0
            } else {
                audio.volume()
            }
        }
        Osd::Brightness => crate::ddc::percent().unwrap_or(0),
    };
    let color = control_color(control);
    let kind = if control == Osd::Volume {
        Popup::Volume
    } else {
        Popup::Brightness
    };
    let track = Rectangle::new()
        .width(48.0)
        .height(154.0)
        .radius(22.0)
        .fill(crate::theme::palette().surface)
        .clip()
        .align_child(Center, End)
        .on_drag(move |point| {
            hover(kind, true);
            let percent = vertical_percent(point.y);
            match control {
                Osd::Volume => Audio::set_volume(percent),
                Osd::Brightness => {
                    crate::ddc::brightness(&[percent.to_string()]);
                }
            }
        })
        .on_scroll(move |scroll| {
            if scroll.y != 0.0 {
                adjust(control, if scroll.y < 0.0 { "up" } else { "down" });
            }
        })
        .on_click(|_| {})
        .child(
            Rectangle::new()
                .width(48.0)
                .height((154.0 * animated / 100.0).max(0.0))
                .fill(Gradient::linear(
                    0.0,
                    [(0.0, color), (1.0, crate::theme::palette().foreground)],
                )),
        );
    Column::new(children![
        track,
        icon(
            if control == Osd::Volume {
                volume_glyph(Audio::read().volume(), Audio::read().muted())
            } else {
                "\u{e1ac}"
            },
            color,
            22.0
        ),
        Text::new(format!("{percent}%"))
            .size(14.0)
            .weight(Weight::SemiBold)
            .color(crate::theme::palette().foreground),
    ])
    .width(48.0)
    .gap(8.0)
    .align(Center)
}

fn vertical_percent(y: f32) -> u8 {
    ((1.0 - y / 154.0).clamp(0.0, 1.0) * 100.0).round() as u8
}

// Content budgets of the HSV picker: saturation/value square, hue strip and
// the copy row, all sharing one width inside the popup's padding.
const PICKER_WIDTH: f32 = 248.0;
const PICKER_SQUARE_H: f32 = 140.0;
const PICKER_HUE_H: f32 = 18.0;
const PICKER_ROW_H: f32 = 40.0;
const PICKER_GAP: f32 = 10.0;

fn color_picker(state: &Hover) -> Column {
    let (hue, saturation, value) = (state.hue, state.saturation, state.value);
    let picked = hsv_to_rgb(hue, saturation, value);
    // Hue base, white left-to-right, black bottom-to-top: the classic SV plane.
    let square = Rectangle::new()
        .width(PICKER_WIDTH)
        .height(PICKER_SQUARE_H)
        .radius(12.0)
        .clip()
        .fill(hsv_to_rgb(hue, 1.0, 1.0))
        .cursor(Pointer)
        .on_drag(|point| set_sv(point.x, point.y))
        .on_click(|_| {})
        .child(
            Rectangle::new()
                .width(PICKER_WIDTH)
                .height(PICKER_SQUARE_H)
                .fill(Gradient::linear(
                    90.0,
                    [(0.0, Color::WHITE), (1.0, Color::TRANSPARENT)],
                ))
                .child(
                    Rectangle::new()
                        .width(PICKER_WIDTH)
                        .height(PICKER_SQUARE_H)
                        .fill(Gradient::linear(
                            0.0,
                            [(0.0, Color::BLACK), (1.0, Color::TRANSPARENT)],
                        )),
                ),
        );
    let indicator = Rectangle::new()
        .width(14.0)
        .height(14.0)
        .radius(Full)
        .border(2.0, Color::WHITE)
        .translate(
            saturation * PICKER_WIDTH - 7.0,
            (1.0 - value) * PICKER_SQUARE_H - 7.0,
        );
    let square = amane::Stack::new(vec![
        Box::new(square) as Box<dyn amane::Widget>,
        Box::new(indicator),
    ])
    .width(PICKER_WIDTH)
    .height(PICKER_SQUARE_H);
    let hue_stops: Vec<(f32, Color)> = (0..=6)
        .map(|step| (step as f32 / 6.0, hsv_to_rgb(step as f32 * 60.0, 1.0, 1.0)))
        .collect();
    let strip = Rectangle::new()
        .width(PICKER_WIDTH)
        .height(PICKER_HUE_H)
        .radius(Full)
        .clip()
        .fill(Gradient::linear(90.0, hue_stops))
        .cursor(Pointer)
        .on_drag(|point| set_hue(point.x))
        .on_click(|_| {});
    let knob = Rectangle::new()
        .width(10.0)
        .height(10.0)
        .radius(Full)
        .fill(picked)
        .border(2.0, Color::WHITE)
        .translate(hue / 360.0 * PICKER_WIDTH - 5.0, (PICKER_HUE_H - 10.0) / 2.0);
    let strip = amane::Stack::new(vec![
        Box::new(strip) as Box<dyn amane::Widget>,
        Box::new(knob),
    ])
    .width(PICKER_WIDTH)
    .height(PICKER_HUE_H);
    let hex = picked.hex();
    let rgb = format!(
        "rgb({},{},{})",
        picked.red(),
        picked.green(),
        picked.blue()
    );
    let (hue_deg, sat, light) = rgb_to_hsl(picked);
    let hsl = format!("hsl({hue_deg}, {sat}%, {light}%)");
    let copied_format = state.copied.map(|(format, _)| format);
    let copy_row = |text: String, key: &'static str, format: ColorFormat| {
        // Only the row whose format was actually copied shows the badge.
        let copy_label = if copied_format == Some(format) {
            "Copied"
        } else {
            "Click to copy"
        };
        Rectangle::new()
            .width(PICKER_WIDTH)
            .height(PICKER_ROW_H)
            .radius(12.0)
            .scale(crate::feedback::scale(key))
            .align_child(Start, Center)
            .cursor(Pointer)
            .on_click(move |button| {
                if matches!(button, Button::Left) {
                    crate::feedback::click(key);
                    copy_picked(format);
                }
            })
            .child(
                Row::new(children![
                    Rectangle::new()
                        .width(32.0)
                        .height(32.0)
                        .radius(10.0)
                        .fill(picked),
                    Column::new(children![
                        Text::new(&text)
                            .size(14.0)
                            .weight(Weight::SemiBold)
                            .color(crate::theme::palette().foreground),
                        Text::new(copy_label)
                            .size(12.0)
                            .color(crate::theme::palette().secondary),
                    ])
                    .width(148.0)
                    .gap(2.0)
                    .align(Start),
                    icon("\u{e14d}", crate::theme::palette().secondary, 20.0),
                ])
                .gap(12.0)
                .align(Center),
            )
    };
    let hex_row = copy_row(hex.clone(), "picker:copy-hex", ColorFormat::Hex);
    let rgb_row = copy_row(rgb.clone(), "picker:copy-rgb", ColorFormat::Rgb);
    let hsl_row = copy_row(hsl.clone(), "picker:copy-hsl", ColorFormat::Hsl);
    Column::new(children![square, strip, hex_row, rgb_row, hsl_row])
        .width(PICKER_WIDTH)
        .gap(PICKER_GAP)
}

fn set_hue(x: f32) {
    hover(Popup::Color, true);
    {
        let mut state = Hover::write();
        state.hue = hue_at(x);
        state.copied = None;
    }
    amane::request_frame();
}

fn set_sv(x: f32, y: f32) {
    hover(Popup::Color, true);
    {
        let mut state = Hover::write();
        let (saturation, value) = sv_at(x, y);
        state.saturation = saturation;
        state.value = value;
        state.copied = None;
    }
    amane::request_frame();
}

fn hue_at(x: f32) -> f32 {
    (x / PICKER_WIDTH * 360.0).clamp(0.0, 360.0) % 360.0
}

fn sv_at(x: f32, y: f32) -> (f32, f32) {
    (
        (x / PICKER_WIDTH).clamp(0.0, 1.0),
        (1.0 - y / PICKER_SQUARE_H).clamp(0.0, 1.0),
    )
}

fn copy_picked(format: ColorFormat) {
    let text = {
        let state = Hover::read();
        let color = hsv_to_rgb(state.hue, state.saturation, state.value);
        match format {
            ColorFormat::Hex => color.hex(),
            ColorFormat::Rgb => {
                format!("rgb({},{},{})", color.red(), color.green(), color.blue())
            }
            ColorFormat::Hsl => {
                let (hue, saturation, light) = rgb_to_hsl(color);
                format!("hsl({hue}, {saturation}%, {light}%)")
            }
        }
    };
    // wl-copy forks a clipboard server and must not inherit captured pipes.
    amane::spawn(&format!("wl-copy --type text/plain '{text}'"));
    let dir = super::settings::state_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("picked-color"), format!("{text}\n"));
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    let hook = config.join("amane/hooks/color");
    if hook.is_file() {
        let path = hook.display().to_string().replace('\'', "'\\''");
        amane::spawn(&format!("'{path}' '{text}'"));
    }
    Hover::write().copied = Some((format, Instant::now()));
    amane::request_frame();
}

/// The textual forms the colour popup can copy.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorFormat {
    Hex,
    Rgb,
    Hsl,
}

/// Converts a colour to CSS `hsl`: hue in degrees, saturation/lightness as
/// rounded percentages.
fn rgb_to_hsl(color: Color) -> (u32, u32, u32) {
    let r = f32::from(color.red()) / 255.0;
    let g = f32::from(color.green()) / 255.0;
    let b = f32::from(color.blue()) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let light = (max + min) / 2.0;
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let hue = hue.rem_euclid(360.0);
    let saturation = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * light - 1.0).abs())
    };
    (
        hue.round() as u32,
        (saturation * 100.0).round() as u32,
        (light * 100.0).round() as u32,
    )
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> Color {
    let hue = hue.rem_euclid(360.0);
    let saturation = saturation.clamp(0.0, 1.0);
    let value = value.clamp(0.0, 1.0);
    let chroma = value * saturation;
    let section = hue / 60.0;
    let secondary = chroma * (1.0 - (section.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match section as u32 {
        0 => (chroma, secondary, 0.0),
        1 => (secondary, chroma, 0.0),
        2 => (0.0, chroma, secondary),
        3 => (0.0, secondary, chroma),
        4 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };
    let match_value = value - chroma;
    let channel = |component: f32| ((component + match_value) * 255.0).round() as u8;
    Color::rgb(channel(r), channel(g), channel(b))
}

fn rgb_to_hsv(color: Color) -> (f32, f32, f32) {
    let r = f32::from(color.red()) / 255.0;
    let g = f32::from(color.green()) / 255.0;
    let b = f32::from(color.blue()) / 255.0;
    let maximum = r.max(g).max(b);
    let minimum = r.min(g).min(b);
    let delta = maximum - minimum;
    let hue = if delta == 0.0 {
        0.0
    } else if maximum == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if maximum == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let saturation = if maximum == 0.0 { 0.0 } else { delta / maximum };
    (hue, saturation, maximum)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn departing_popup_rounds_toward_a_capsule() {
        assert_eq!(popup_radius(200.0, 100.0, 0.0), 50.0);
        assert_eq!(popup_radius(200.0, 100.0, 1.0), 28.0);
        for p in [0.55, 0.65, 0.8, 0.95] {
            assert_eq!(
                popup_radius(200.0, 100.0, p),
                28.0,
                "content still fading at {p}"
            );
        }
        assert!(popup_radius(200.0, 100.0, 0.3) > popup_radius(200.0, 100.0, 0.8));
    }
    #[test]
    fn month_navigation_crosses_years_and_leap_days() {
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2024, 3, -1), (2024, 2));
    }
    #[test]
    fn popup_rectangles_start_inside_controls_and_bounce() {
        for kind in [
            Popup::Calendar,
            Popup::Media,
            Popup::Volume,
            Popup::Brightness,
            Popup::Color,
        ] {
            let (x, y, w, h) = origin(kind);
            assert!(x >= 0.0 && x + w <= PANEL.0);
            assert!(y >= 0.0 && y + h <= PANEL.1);
            assert_eq!(x + w / 2.0, anchor(kind));
        }
        assert_eq!(blob_growth(0.0), 0.0);
        assert!(
            (1..100)
                .map(|i| blob_growth(i as f32 / 100.0))
                .any(|g| g > 1.04)
        );
        assert_eq!(blob_growth(1.0), 1.0);
    }

    #[test]
    fn row_budgets_and_popup_anchors_stay_balanced() {
        assert_eq!(
            ROW_PADDING * 2.0
                + CONTROL_WIDTH * 3.0
                + CONTROL_GAP * 2.0
                + MEDIA_WIDTH
                + CLOCK_WIDTH
                + TRANSPORT_WIDTH
                + ROW_GAP * 5.0
                + visualizer::WIDTH
                + modules::Module::Settings.width(),
            PANEL.0
        );
        assert!(anchor(Popup::Volume) < anchor(Popup::Calendar));
        assert!(anchor(Popup::Brightness) < anchor(Popup::Color));
        for kind in [
            Popup::Calendar,
            Popup::Media,
            Popup::Volume,
            Popup::Brightness,
            Popup::Color,
        ] {
            let (w, _, x) = geometry(kind, 6);
            assert!(x >= ROW_PADDING);
            assert!(x + w <= PANEL.0 - ROW_PADDING);
            assert!(anchor(kind) >= x && anchor(kind) <= x + w);
        }
        assert_eq!(
            anchor(Popup::Brightness) - anchor(Popup::Volume),
            CONTROL_WIDTH + CONTROL_GAP
        );
        assert_eq!(blob_growth(0.0), 0.0);
        assert_eq!(blob_growth(1.0), 1.0);
    }

    #[test]
    fn calendar_monday_first_and_leap_years() {
        let october = calendar_rows(2026, 10);
        assert_eq!(october[0], "          1  2  3  4 ");
        assert_eq!(october.len(), 5);
        assert!(calendar_rows(2024, 2).join("").contains("29"));
        assert!(!calendar_rows(2100, 2).join("").contains("29"));
        assert_eq!(calendar_rows(2021, 8).len(), 6);
    }
    #[test]
    fn vertical_track_inverts_and_clamps() {
        assert_eq!(vertical_percent(-10.0), 100);
        assert_eq!(vertical_percent(0.0), 100);
        assert_eq!(vertical_percent(77.0), 50);
        assert_eq!(vertical_percent(154.0), 0);
        assert_eq!(vertical_percent(200.0), 0);
    }

    #[test]
    fn hsv_corners_match_primary_colors() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), Color::RED);
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), Color::GREEN);
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), Color::BLUE);
        assert_eq!(hsv_to_rgb(0.0, 0.0, 1.0), Color::WHITE);
        assert_eq!(hsv_to_rgb(0.0, 1.0, 0.0), Color::BLACK);
        assert_eq!(hsv_to_rgb(360.0, 1.0, 1.0), Color::RED);
    }

    #[test]
    fn hsv_roundtrips_theme_channels() {
        for color in [
            Color::rgb(129, 161, 193),
            Color::rgb(1, 2, 3),
            Color::rgb(250, 244, 237),
            Color::rgb(191, 97, 106),
        ] {
            let (hue, saturation, value) = rgb_to_hsv(color);
            let back = hsv_to_rgb(hue, saturation, value);
            let near = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs() <= 1;
            assert!(near(back.red(), color.red()));
            assert!(near(back.green(), color.green()));
            assert!(near(back.blue(), color.blue()));
        }
    }

    #[test]
    fn picker_coordinates_clamp_to_hue_and_sv_ranges() {
        assert_eq!(hue_at(-20.0), 0.0);
        assert_eq!(hue_at(0.0), 0.0);
        assert_eq!(hue_at(PICKER_WIDTH), 0.0); // wraps back to zero
        assert!((hue_at(PICKER_WIDTH / 2.0) - 180.0).abs() < f32::EPSILON);
        assert_eq!(sv_at(0.0, 0.0), (0.0, 1.0));
        assert_eq!(sv_at(PICKER_WIDTH, PICKER_SQUARE_H), (1.0, 0.0));
        assert_eq!(sv_at(-5.0, -5.0), (0.0, 1.0));
        assert_eq!(sv_at(1e9, 1e9), (1.0, 0.0));
    }

    #[test]
    fn hsl_matches_known_colors() {
        assert_eq!(rgb_to_hsl(Color::rgb(255, 0, 0)), (0, 100, 50));
        assert_eq!(rgb_to_hsl(Color::rgb(0, 255, 0)), (120, 100, 50));
        assert_eq!(rgb_to_hsl(Color::rgb(0, 0, 255)), (240, 100, 50));
        assert_eq!(rgb_to_hsl(Color::WHITE), (0, 0, 100));
        assert_eq!(rgb_to_hsl(Color::BLACK), (0, 0, 0));
        assert_eq!(rgb_to_hsl(Color::rgb(128, 128, 128)), (0, 0, 50));
    }
}

/// Also useful for keyboard access and deterministic previewing.
pub fn command(arguments: &[String]) -> String {
    match arguments.first().map(String::as_str) {
        Some("workspace") => return "Workspace popups have been removed.".into(),
        Some("calendar") => show(Popup::Calendar),
        Some("media") => show(Popup::Media),
        Some("volume") => show(Popup::Volume),
        Some("brightness") => show(Popup::Brightness),
        Some("color") => show(Popup::Color),
        Some("hide") => {
            dismiss();
            pointer_presence(false);
        }
        _ => return String::from("usage: popup calendar|media|volume|brightness|color|hide"),
    }
    String::from("ok")
}

fn change_month(delta: i32) {
    let offset = Hover::read()
        .month_offset
        .saturating_add(delta)
        .clamp(-1200, 1200);
    let (day, month, today, calendar) = calendar_at(offset);
    let mut state = Hover::write();
    state.month_offset = offset;
    state.calendar_day = day;
    state.month = month;
    state.today = today;
    state.calendar = calendar;
    state.deadline = None;
}

fn local_calendar() -> (String, String, u8, Vec<String>) {
    calendar_at(0)
}

fn calendar_at(offset: i32) -> (String, String, u8, Vec<String>) {
    let date = amane::output("date '+%Y %m %d'");
    let mut parts = date
        .split_whitespace()
        .map(|s| s.parse::<u32>().unwrap_or(1));
    let year = parts.next().unwrap_or(2026);
    let month = parts.next().unwrap_or(1).clamp(1, 12);
    let today = parts.next().unwrap_or(1) as u8;
    let (view_year, view_month) = shift_month(year, month, offset);
    let label = amane::output(&format!(
        "date -d '{view_year:04}-{view_month:02}-01' '+%B %Y'"
    ));
    (
        date.trim().to_string(),
        label.trim().to_string(),
        if offset == 0 { today } else { 0 },
        calendar_rows(view_year, view_month),
    )
}

fn shift_month(year: u32, month: u32, offset: i32) -> (u32, u32) {
    let index = (year as i32 * 12 + month as i32 - 1 + offset).max(12);
    ((index / 12) as u32, (index % 12 + 1) as u32)
}

fn calendar_rows(year: u32, month: u32) -> Vec<String> {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let count = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    // Sakamoto: Sunday=0, convert to Monday=0.
    let offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = year - u32::from(month < 3);
    let weekday = (y + y / 4 - y / 100 + y / 400 + offsets[(month - 1) as usize] + 1) % 7;
    let monday = (weekday + 6) % 7;
    let mut rows = Vec::new();
    let mut row = String::new();
    for slot in 0..((monday + count).div_ceil(7) * 7) {
        if slot < monday || slot >= monday + count {
            row.push_str("   ");
        } else {
            row.push_str(&format!("{:2} ", slot - monday + 1));
        }
        if slot % 7 == 6 {
            rows.push(std::mem::take(&mut row));
        }
    }
    rows
}

pub(super) fn visible_height() -> f32 {
    let state = Hover::read();
    if state.open || state.closed.elapsed() < Duration::from_millis(BLOB_MS + 20) {
        state.box_h.precise_value() * state.motion.value().clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub(super) fn popup_radius(w: f32, h: f32, p: f32) -> f32 {
    let morph = ((0.55 - p) / 0.55).clamp(0.0, 1.0);
    let morph = morph * morph * (3.0 - 2.0 * morph);
    (28.0 + (w.min(h) / 2.0 - 28.0) * morph)
        .max(0.0)
        .min(w.min(h) / 2.0)
}
