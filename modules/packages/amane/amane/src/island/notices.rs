//! Independent notification stack; never replaces the clock or expanded row.
use super::*;
pub(super) const WIDTH: f32 = 400.0;
const PADDING: f32 = 24.0;
const GAP: f32 = 6.0;
const MAX_VISIBLE: usize = 5;
struct Toast {
    id: u32,
    title: String,
    body: String,
    app: String,
    icon: String,
    image: String,
    actions: Vec<(String, String)>,
    received: SystemTime,
    opened: Instant,
    expires: Instant,
    visible: bool,
    offset: Spring,
    closing: Option<Instant>,
    held: bool,
    critical: bool,
    stored_progress: Option<u8>,
    history_key: Option<u64>,
    actionable: bool,
}
struct Notices {
    list: Vec<Toast>,
    since: SystemTime,
}
impl Service for Notices {
    fn new() -> Self {
        let _ = notice_hints::value(0);
        Self {
            list: vec![],
            since: SystemTime::now(),
        }
    }
    fn interval() -> Duration {
        Duration::from_millis(40)
    }
    fn update(&mut self) -> bool {
        let mut changed = false;
        let now = Instant::now();
        let notifications = Notifications::read();
        for n in notifications
            .list()
            .iter()
            .filter(|n| n.received() > self.since)
        {
            history::record(n);
            if let Some(old) = self.list.iter_mut().find(|t| t.id == n.id()) {
                if n.received() != old.received {
                    old.title = n.summary().into();
                    old.body = n.body().into();
                    old.icon = n.icon().into();
                    old.image = n.image().into();
                    old.actions = n
                        .actions()
                        .iter()
                        .map(|a| (a.key().into(), a.label().into()))
                        .collect();
                    old.received = n.received();
                    old.expires = now + Duration::from_secs(7);
                    old.closing = None;
                    changed = true;
                }
            } else {
                self.list.push(Toast {
                    id: n.id(),
                    title: n.summary().into(),
                    body: n.body().into(),
                    app: n.app_name().into(),
                    icon: n.icon().into(),
                    image: n.image().into(),
                    actions: n
                        .actions()
                        .iter()
                        .map(|a| (a.key().into(), a.label().into()))
                        .collect(),
                    received: n.received(),
                    opened: now,
                    expires: now + Duration::from_secs(7),
                    visible: false,
                    offset: Spring::new(0.0),
                    closing: None,
                    held: false,
                    critical: n.urgency() == amane::Urgency::Critical,
                    stored_progress: None,
                    history_key: None,
                    actionable: true,
                });
                changed = true;
            }
        }
        if let Some(newest) = notifications.list().iter().map(|n| n.received()).max() {
            self.since = self.since.max(newest);
        }
        for (i, t) in self.list.iter_mut().enumerate() {
            history::update_progress(t.id, notice_hints::value(t.id));
            if i < MAX_VISIBLE && !t.visible {
                t.visible = true;
                t.opened = now;
                t.expires = now + Duration::from_secs(7);
                changed = true;
            }
            if t.closing.is_none()
                && (!notifications.list().iter().any(|n| n.id() == t.id)
                    || (i < MAX_VISIBLE && !t.held && !t.critical && now >= t.expires))
            {
                t.closing = Some(now);
                changed = true;
            }
        }
        drop(notifications);
        let old_len = self.list.len();
        let mut dismissed = vec![];
        self.list.retain(|t| {
            let keep = t
                .closing
                .is_none_or(|closed| closed.elapsed() < Duration::from_millis(popups::BLOB_MS));
            if !keep {
                dismissed.push(t.id);
            }
            keep
        });
        for id in dismissed {
            Notifications::dismiss(id);
        }
        if old_len != self.list.len() {
            changed = true;
        }
        let mut offset = 0.0;
        for toast in self.list.iter_mut().take(MAX_VISIBLE) {
            if !toast.visible {
                continue;
            }
            toast.offset.to(offset);
            let amount = progress(toast.opened.elapsed(), toast.closing.map(|t| t.elapsed()));
            offset += (card_height(toast) + GAP) * amount;
        }
        changed
    }
}
fn hold(id: u32, inside: bool) {
    if let Some(t) = Notices::write().list.iter_mut().find(|t| t.id == id) {
        t.held = inside;
        if !inside {
            t.expires = Instant::now() + Duration::from_secs(7);
        }
    }
}
fn dismiss(id: u32) {
    if let Some(t) = Notices::write().list.iter_mut().find(|t| t.id == id) {
        t.closing = Some(Instant::now());
    }
}
pub(super) fn decorate(
    scene: popups::Scene,
    bar_width: f32,
    bar_height: f32,
    shift: f32,
) -> popups::Scene {
    let state = Notices::read();
    if state.list.is_empty() || fullscreen::blocked() {
        return scene;
    }
    let width = scene.width.max(WIDTH + 72.0);
    let dx = (width - scene.width) / 2.0;
    let dy = 36.0 - scene.top;
    let top = 36.0;
    let extent = popups::visible_height();
    let hover_bottom = top + bar_height + extent + 12.0 * (extent / 48.0).clamp(0.0, 1.0);
    // Parent scene still slides with the island. Compensate only notification
    // geometry, leaving the island hidden. Hidden notifications originate in
    // the screen frame instead of carrying the island back onscreen.
    let compensation = GAP_TOP - shift;
    let first_y = hover_bottom + GAP + compensation
        - (bar_height + GAP_TOP - border::THICKNESS) * exposure::amount();
    let height = (first_y
        + state
            .list
            .iter()
            .take(MAX_VISIBLE)
            .map(|t| card_height(t) + GAP)
            .sum::<f32>()
        + 36.0)
        .max(scene.height + dy);
    let wrap = |widget: Box<dyn amane::Widget>| {
        Rectangle::new()
            .width(scene.width)
            .height(scene.height)
            .translate(dx, dy)
            .child(
                amane::Stack::new(vec![widget])
                    .width(scene.width)
                    .height(scene.height),
            )
    };
    let mut backgrounds: Vec<Box<dyn amane::Widget>> = vec![Box::new(wrap(scene.background))];
    let mut contents: Vec<Box<dyn amane::Widget>> = vec![Box::new(wrap(scene.content))];
    let mut input = scene.input;
    for region in &mut input {
        region.x += dx as i32;
        region.y += dy as i32;
    }
    let hide = exposure::amount();
    let source_height = bar_height + (border::THICKNESS - bar_height) * hide;
    let mut previous = [
        (width - bar_width) / 2.0,
        top + compensation - (GAP_TOP * hide),
        bar_width,
        source_height,
    ];
    for (i, t) in state.list.iter().take(MAX_VISIBLE).enumerate() {
        let card_height = card_height(t);
        let y = first_y + t.offset.precise_value();
        let p = progress(t.opened.elapsed(), t.closing.map(|closed| closed.elapsed()));
        if (0.0..1.0).contains(&p) {
            amane::request_frame();
        }
        let growth = popups::blob_growth(p);
        let from_center = previous[1] + previous[3] / 2.0;
        let w = 80.0 + (WIDTH - 80.0) * growth;
        let h = 32.0 + (card_height - 32.0) * growth;
        let center = from_center + (y + card_height / 2.0 - from_center) * growth;
        let body = [(width - w) / 2.0, center - h / 2.0, w, h];
        let sigma = 11.0 * (1.0 - ((p - 0.55) / 0.25).clamp(0.0, 1.0));
        let path = std::path::Path::new(file!())
            .parent()
            .unwrap()
            .join("goo.wgsl");
        backgrounds.push(Box::new(
            Rectangle::new()
                .width(width)
                .height(height)
                .shader(path)
                .shader_values(crate::theme::shader_values(vec![
                    previous,
                    body,
                    [
                        if i == 0 { bar_height / 2.0 } else { 24.0 },
                        popups::popup_radius(w, h, p),
                        sigma,
                        0.48,
                    ],
                ])),
        ));
        let id = t.id;
        let card = render_card(t, WIDTH);
        contents.push(Box::new(
            Rectangle::new()
                .width(WIDTH)
                .height(card_height)
                .translate((width - WIDTH) / 2.0, y)
                .padding(PADDING)
                .opacity(((p - 0.55) / 0.3).clamp(0.0, 1.0))
                .cursor(Pointer)
                .on_hover(move |inside| {
                    hold(id, inside);
                })
                .on_click(move |b| {
                    if matches!(b, Button::Left) {
                        Notifications::click(id);
                    } else if matches!(b, Button::Right) {
                        dismiss(id);
                    }
                })
                .child(card),
        ));
        input.push(amane::InputArea {
            x: ((width - WIDTH) / 2.0) as i32,
            y: y as i32,
            width: WIDTH as i32,
            height: card_height as i32,
        });
        previous = [(width - WIDTH) / 2.0, y, WIDTH, card_height];
    }
    popups::Scene {
        width,
        height,
        top,
        background: Box::new(amane::Stack::new(backgrounds).width(width).height(height)),
        content: Box::new(amane::Stack::new(contents).width(width).height(height)),
        input,
    }
}

fn progress(age: Duration, closing: Option<Duration>) -> f32 {
    closing
        .map_or(
            age.as_secs_f32() / (popups::BLOB_MS as f32 / 1000.0),
            |elapsed| 1.0 - elapsed.as_secs_f32() / (popups::BLOB_MS as f32 / 1000.0),
        )
        .clamp(0.0, 1.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_motion_opens_and_reverses_without_text_scaling() {
        assert_eq!(progress(Duration::ZERO, None), 0.0);
        assert_eq!(progress(Duration::from_millis(210), None), 0.5);
        assert_eq!(progress(Duration::from_secs(8), None), 1.0);
        assert_eq!(
            progress(Duration::ZERO, Some(Duration::from_millis(210))),
            0.5
        );
        assert_eq!(progress(Duration::ZERO, Some(Duration::from_secs(1))), 0.0);
    }
}

fn image_path(value: &str) -> Option<std::path::PathBuf> {
    let value = value.strip_prefix("file://").unwrap_or(value);
    let path = std::path::PathBuf::from(value);
    if path.is_absolute() && path.is_file() {
        Some(path)
    } else {
        None
    }
}
fn icon_path(t: &Toast) -> Option<std::path::PathBuf> {
    if let Some(path) = image_path(&t.icon) {
        return Some(path);
    }
    let apps = amane::Apps::read();
    apps.list()
        .iter()
        .find(|a| {
            a.icon().is_some_and(|i| i.eq_ignore_ascii_case(&t.icon))
                || a.name().eq_ignore_ascii_case(&t.app)
        })
        .and_then(|a| a.icon_path().map(|p| p.to_path_buf()))
}
fn card_height(t: &Toast) -> f32 {
    let content = render_card(t, WIDTH);
    let height = match amane::Widget::height(&content) {
        amane::Size::Fixed(height) => height,
        _ => 0.0,
    };
    height + 2.0 * PADDING
}
fn title_text(t: &Toast) -> Text {
    Text::new(&t.title)
        .size(16.0)
        .weight(Weight::SemiBold)
        .color(crate::theme::palette().foreground)
        .wrap()
        .max_lines(2)
        .elide()
}
fn body_text(t: &Toast) -> Text {
    Text::new(plain_body(&t.body))
        .size(14.0)
        .color(crate::theme::palette().secondary)
        .wrap()
        .max_lines(3)
        .elide()
}
fn measured_text(text: Text, width: f32) -> Rectangle {
    let height = text.height_in(width - 2.0 * PADDING);
    Rectangle::new()
        .width(width - 2.0 * PADDING)
        .height(height)
        .child(text)
}

fn render_card(t: &Toast, width: f32) -> Column {
    let id = t.id;
    let history_key = t.history_key;
    let actionable = t.actionable;
    let icon = Rectangle::new()
        .width(36.0)
        .height(36.0)
        .radius(9.0)
        .align_child(Center, Center);
    let icon = if let Some(path) = icon_path(t) {
        icon.fill(amane::Image::contain(path))
    } else {
        let letter = Text::new(
            t.app
                .chars()
                .next()
                .unwrap_or('!')
                .to_uppercase()
                .to_string(),
        )
        .size(20.0)
        .weight(Weight::Bold)
        .color(Color::BLACK);
        // Gradients force a window-sized vector pass per frame; the history
        // list can show many of them, so it gets a plain fill.
        if t.history_key.is_some() {
            icon.fill(crate::theme::palette().blue).child(letter)
        } else {
            icon.fill(icon_gradient(crate::theme::palette().blue))
                .child(letter)
        }
    };
    let close = Rectangle::new()
        .width(28.0)
        .height(28.0)
        .radius(Full)
        .fill(crate::theme::palette().surface)
        .cursor(Pointer)
        .align_child(Center, Center)
        .on_click(move |b| {
            if matches!(b, Button::Left) {
                if let Some(key) = history_key {
                    history::remove(key)
                } else {
                    dismiss(id)
                }
            }
        })
        .child(
            Text::new("×")
                .size(22.0)
                .color(crate::theme::palette().secondary),
        );
    let mut rows = children![
        Row::new(children![
            icon,
            Rectangle::new()
                .width(width - 2.0 * PADDING - 88.0)
                .height(36.0)
                .align_child(Start, Center)
                .child(
                    Text::new(&t.app)
                        .size(13.0)
                        .weight(Weight::SemiBold)
                        .color(crate::theme::palette().secondary)
                        .elide()
                ),
            close
        ])
        .gap(12.0)
        .align(Center),
        measured_text(title_text(t), width)
    ];
    if !t.body.is_empty() {
        rows.push(Box::new(measured_text(body_text(t), width)));
    }
    if let Some(path) = image_path(&t.image) {
        rows.push(Box::new(
            Rectangle::new()
                .width(width - 2.0 * PADDING)
                .height(136.0)
                .radius(14.0)
                .clip()
                .fill(amane::Image::contain(path)),
        ));
    }
    if let Some(value) = if t.history_key.is_some() {
        t.stored_progress
    } else {
        notice_hints::value(id)
    } {
        rows.push(Box::new(
            Row::new(children![
                Rectangle::new()
                    .width((width - 2.0 * PADDING - 52.0).max(20.0))
                    .height(8.0)
                    .radius(Full)
                    .fill(crate::theme::palette().surface)
                    .child(
                        Rectangle::new()
                            .width(
                                ((width - 2.0 * PADDING - 52.0).max(20.0) * value as f32 / 100.0)
                                    .max(1.0)
                            )
                            .height(8.0)
                            .radius(Full)
                            .fill(icon_gradient(crate::theme::palette().blue))
                    ),
                Rectangle::new()
                    .width(42.0)
                    .height(20.0)
                    .align_child(End, Center)
                    .child(
                        Text::new(format!("{value}%"))
                            .size(13.0)
                            .weight(Weight::SemiBold)
                            .color(crate::theme::palette().foreground)
                    )
            ])
            .gap(10.0)
            .align(Center),
        ));
    }
    for chunk in t.actions.chunks(2) {
        let buttons = chunk
            .iter()
            .map(|(key, label)| {
                let key = key.clone();
                let feedback_key = format!("notice:{id}:{key}");
                let click_key = feedback_key.clone();
                let label = Text::new(label)
                    .size(14.0)
                    .weight(Weight::SemiBold)
                    .color(crate::theme::palette().blue);
                let label_width = {
                    use amane::Widget;
                    match label.width() {
                        amane::Size::Fixed(width) => width,
                        _ => 0.0,
                    }
                }
                .min((width - 2.0 * PADDING - 8.0) / 2.0 - 24.0);
                Box::new(
                    Rectangle::new()
                        .width((width - 2.0 * PADDING - 8.0) / 2.0)
                        .height(36.0)
                        .scale(crate::feedback::scale(&feedback_key))
                        .radius(12.0)
                        .fill(Color::TRANSPARENT)
                        .opacity(if actionable { 1.0 } else { 0.45 })
                        .cursor(if actionable { Pointer } else { amane::Default })
                        .align_child(Center, Center)
                        .on_click(move |b| {
                            if actionable && matches!(b, Button::Left) {
                                crate::feedback::click(&click_key);
                                Notifications::invoke(id, &key);
                            }
                        })
                        .child(
                            Rectangle::new()
                                .width(label_width)
                                .height(20.0)
                                .align_child(Center, Center)
                                .child(label.elide()),
                        ),
                ) as Box<dyn amane::Widget>
            })
            .collect();
        rows.push(Box::new(Row::new(buttons).gap(8.0)));
    }
    Column::new(rows).width(width - 2.0 * PADDING).gap(12.0)
}
/// The very same card content used by live toasts, rendered inside a normal
/// settings container. Archive-only IDs are never dispatched to D-Bus.
type HeightKey = (u64, u32, usize, usize, bool);
static CARD_HEIGHTS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<HeightKey, f32>>> =
    std::sync::LazyLock::new(Default::default);
/// Height of a history card, measured once and remembered (text layout is the
/// expensive part), so long lists can be scrolled without rebuilding every card.
pub(super) fn history_card_height(entry: &history::Entry, width: f32) -> f32 {
    let key = (
        entry.key,
        width as u32,
        entry.title.len(),
        entry.body.len(),
        !entry.image.is_empty(),
    );
    if let Some(h) = CARD_HEIGHTS.lock().ok().and_then(|m| m.get(&key).copied()) {
        return h;
    }
    let h = match amane::Widget::height(&history_card(entry, width)) {
        amane::Size::Fixed(h) => h,
        _ => 180.0,
    };
    if let Ok(mut m) = CARD_HEIGHTS.lock() {
        if m.len() > 2000 {
            m.clear();
        }
        m.insert(key, h);
    }
    h
}
pub(super) fn history_card(entry: &history::Entry, width: f32) -> Rectangle {
    let live = entry.live_id.and_then(|id| {
        Notifications::read()
            .list()
            .iter()
            .find(|n| n.id() == id)
            .map(|_| id)
    });
    let now = Instant::now();
    let t = Toast {
        id: live.unwrap_or(0),
        title: entry.title.clone(),
        body: entry.body.clone(),
        app: entry.app.clone(),
        icon: entry.icon.clone(),
        image: entry.image.clone(),
        actions: entry.actions.clone(),
        received: SystemTime::UNIX_EPOCH + Duration::from_secs(entry.time),
        opened: now,
        expires: now,
        visible: true,
        offset: Spring::new(0.0),
        closing: None,
        held: false,
        critical: false,
        stored_progress: entry.progress,
        history_key: Some(entry.key),
        actionable: live.is_some(),
    };
    let card = render_card(&t, width);
    let h = match amane::Widget::height(&card) {
        amane::Size::Fixed(h) => h + PADDING * 2.0,
        _ => 180.0,
    };
    Rectangle::new()
        .width(width)
        .height(h)
        .radius(20.0)
        .fill(crate::theme::surface())
        .border(1.0, crate::theme::palette().raised)
        .padding(PADDING)
        .on_click(move |b| {
            if b == Button::Left {
                if let Some(id) = live {
                    Notifications::click(id)
                }
            }
        })
        .child(card)
}

pub(super) fn plain_body(text: &str) -> String {
    let mut inside = false;
    text.chars()
        .filter(|c| {
            if *c == '<' {
                inside = true;
                false
            } else if *c == '>' {
                inside = false;
                false
            } else {
                !inside
            }
        })
        .collect::<String>()
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}
