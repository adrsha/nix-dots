//! IPC-operated lower-edge islands. Work runs in owned worker subprocesses.
use amane::{
    Apps, Button, Center, Column, Full, Horizontal, InputArea, Key, Keyboard, LayerWindow, Pointer,
    Rectangle, Row, Service, Start, Text, Vertical, Widget, children,
};
use std::{
    io::{BufRead, BufReader, Write},
    process::{ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Screenshot,
    Record,
    Session,
    Search,
}
struct Auxiliary {
    kind: Kind,
    open: bool,
    motion: crate::island::motion::Spring,
    body_height: amane::Animation,
    target_height: f32,
    deadline: Option<std::time::Instant>,
    results: amane::Animation,
    query: String,
    caret: usize,
    caret_x: f32,
    edited: std::time::Instant,
    closing_search: Option<std::time::Instant>,
    close_wait: Duration,
    deferred_action: Option<(String, Vec<String>)>,
    status: String,
    confirm: Option<String>,
    file: Option<String>,
    record_control: Option<Arc<Mutex<ChildStdin>>>,
    busy: bool,
    selected: usize,
    tooltip: Option<String>,
    pending: Option<Kind>,
    switching_since: Option<std::time::Instant>,
    tip_motion: crate::island::motion::Spring,
    tip_y: crate::island::motion::Spring,
    tip_width: crate::island::motion::Spring,
    tip_deadline: Option<std::time::Instant>,
    /// True from the moment a region/window capture or recording starts until
    /// the pick is made or cancelled. The screen border carries the cue.
    selecting: bool,
    /// Border tint (0 = normal black frame, 1 = full capture colour).
    tint: amane::Animation,
    tint_color: amane::Color,
    /// The running action worker, so a cancel can reach it.
    worker_pid: Option<u32>,
    /// Pixel size of the captured image, for the preview overlay.
    file_size: Option<(f32, f32)>,
}
impl Service for Auxiliary {
    fn new() -> Self {
        Self {
            kind: Kind::Screenshot,
            open: false,
            motion: crate::island::motion::Spring::slow(0.0, 1.5),
            body_height: amane::Animation::new(76.0).duration(Duration::from_millis(340)),
            target_height: 76.0,
            deadline: None,
            results: amane::Animation::new(0.0)
                .duration(Duration::from_millis(420))
                .easing(amane::Easing::InOut),
            query: String::new(),
            caret: 0,
            caret_x: 0.0,
            edited: std::time::Instant::now(),
            closing_search: None,
            close_wait: Duration::from_millis(460),
            deferred_action: None,
            status: String::new(),
            confirm: None,
            file: None,
            record_control: None,
            busy: false,
            selected: 0,
            tooltip: None,
            pending: None,
            switching_since: None,
            tip_motion: crate::island::motion::Spring::new(0.0),
            tip_y: crate::island::motion::Spring::new(0.0),
            tip_width: crate::island::motion::Spring::new(140.0),
            tip_deadline: None,
            selecting: false,
            tint: amane::Animation::new(0.0).duration(Duration::from_millis(220)),
            tint_color: amane::Color::BLACK,
            worker_pid: None,
            file_size: None,
        }
    }
    fn interval() -> Duration {
        Duration::from_millis(30)
    }
    fn update(&mut self) -> bool {
        let mut changed = false;
        if let Some(start) = self.closing_search {
            if start.elapsed() >= self.close_wait {
                self.motion.to(0.0);
            }
            if start.elapsed() >= self.close_wait + Duration::from_millis(960) {
                self.closing_search = None;
                self.open = false;
                if let Some((action, args)) = self.deferred_action.take() {
                    std::thread::spawn(move || worker(&action, args));
                }
                changed = true;
            }
        }
        if self
            .tip_deadline
            .is_some_and(|d| std::time::Instant::now() >= d)
        {
            self.tip_motion.to(0.0);
            self.tip_deadline = None;
            changed = true;
        }
        if self.switching_since.is_some_and(|t| {
            t.elapsed()
                >= if self.kind == Kind::Search {
                    self.close_wait + Duration::from_millis(960)
                } else {
                    Duration::from_millis(940)
                }
        }) {
            if let Some(kind) = self.pending.take() {
                self.kind = kind;
                self.open = true;
                self.motion.to(1.0);
                self.confirm = None;
                self.tooltip = None;
                self.tip_motion.to(0.0);
                self.deadline = None;
            }
            self.switching_since = None;
            changed = true;
        }
        if self.file.is_some() {
            // A capture waiting for Save/Annotate is only dismissed explicitly.
            self.deadline = None;
        }
        if self
            .deadline
            .is_some_and(|d| std::time::Instant::now() >= d)
        {
            begin_close(self);
            self.deadline = None;
            return true;
        }
        self.results.to(
            if !self.open || self.query.is_empty() || self.closing_search.is_some() {
                0.0
            } else {
                1.0
            },
        );
        let content = if self.kind == Kind::Search {
            search_results(self, 560.0)
        } else {
            vertical_panel(self, 46.0)
        };
        let target = match Widget::height(&content) {
            amane::Size::Fixed(h) => h + 32.0,
            _ => 180.0,
        };
        changed |= target != self.target_height;
        self.target_height = target;
        if !(self.kind == Kind::Search && (self.query.is_empty() || self.closing_search.is_some()))
        {
            self.body_height.to(target);
        }
        changed
    }
}
pub fn command(args: &[String]) -> String {
    let mut state = Auxiliary::write();
    let requested = args.first().map(String::as_str);
    // Any island command while a pick is pending (hide, or the same/another
    // island) cancels the pick instead of stacking a second flow on top of it.
    if state.selecting && matches!(requested, Some("screenshot" | "record" | "hide")) {
        cancel_selection(&state);
        return "ok".into();
    }
    let kind = match requested {
        Some("screenshot") => Kind::Screenshot,
        Some("record") => Kind::Record,
        Some("sessions" | "session") => Kind::Session,
        Some("search") => Kind::Search,
        Some("hide") => {
            state.pending = None;
            state.switching_since = None;
            begin_close(&mut state);
            return "ok".into();
        }
        Some("status") => return state.status.clone(),
        _ => return "usage: auxiliary screenshot|record|session|search [query]|hide|status".into(),
    };
    if (state.kind == kind && state.open) || state.pending == Some(kind) {
        state.pending = None;
        state.switching_since = None;
        begin_close(&mut state);
        return "ok".into();
    }
    request_kind(&mut state, kind);
    state.selected = 0;
    if kind == Kind::Search {
        state.query = args.get(1..).unwrap_or_default().join(" ");
        state.caret = state.query.len();
        update_caret(&mut state);
    }
    "ok".into()
}
fn begin_close(s: &mut Auxiliary) {
    s.tip_motion.to(0.0);
    s.deadline = None;
    if s.kind == Kind::Search && (s.open || s.motion.precise_value() > 0.01) {
        if s.closing_search.is_none() {
            s.close_wait = if s.results.value() > 0.001 {
                Duration::from_millis(460)
            } else {
                Duration::ZERO
            };
            s.closing_search = Some(std::time::Instant::now());
            if s.close_wait.is_zero() {
                s.motion.to(0.0);
            }
        }
        s.results.to(0.0);
    } else {
        s.open = false;
        s.motion.to(0.0);
        // Nothing from a finished flow may survive the island closing.
        if !s.busy {
            discard_capture(s);
            s.status.clear();
            s.confirm = None;
        }
    }
}
/// Cancelling anywhere throws the unsaved capture away.
fn discard_capture(s: &mut Auxiliary) {
    if let Some(file) = s.file.take() {
        let path = std::path::Path::new(&file);
        if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("amane-shot-"))
        {
            let _ = std::fs::remove_file(path);
        }
    }
    s.file_size = None;
}
fn png_size(path: &str) -> Option<(f32, f32)> {
    use std::io::Read;
    let mut bytes = [0u8; 24];
    std::fs::File::open(path).ok()?.read_exact(&mut bytes).ok()?;
    if &bytes[1..4] != b"PNG" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width as f32, height as f32))
}
fn cancel_selection(s: &Auxiliary) {
    if let Some(pid) = s.worker_pid {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}
/// Colour the screen border takes while a pick is pending: one for screenshots,
/// another shared by recording / screen sharing.
fn capture_color(action: &str) -> amane::Color {
    let palette = crate::theme::palette();
    if action == "record" {
        palette.red
    } else {
        palette.blue
    }
}
fn hex(color: amane::Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.red(), color.green(), color.blue())
}
/// (colour, strength 0..1) for the main island's border frame, if tinted.
pub fn tint() -> Option<(amane::Color, f32)> {
    let s = Auxiliary::read();
    let amount = s.tint.value();
    (amount > 0.002).then_some((s.tint_color, amount.clamp(0.0, 1.0)))
}
fn update_caret(s: &mut Auxiliary) {
    let prefix = &s.query[..s.caret];
    let width = match Widget::width(&text(prefix).size(17.0)) {
        amane::Size::Fixed(w) => w,
        _ => 0.0,
    };
    s.caret_x = width;
    s.edited = std::time::Instant::now();
}
fn request_kind(s: &mut Auxiliary, kind: Kind) {
    s.deadline = None;
    if s.kind != kind && (s.open || s.motion.precise_value() > 0.01) {
        s.pending = Some(kind);
        if s.switching_since.is_none() {
            s.switching_since = Some(std::time::Instant::now());
        }
        begin_close(s);
        s.tip_deadline = None;
    } else {
        s.pending = None;
        s.switching_since = None;
        s.closing_search = None;
        s.deferred_action = None;
        if s.kind != kind {
            s.confirm = None;
            s.tooltip = None;
            s.tip_motion.to(0.0);
        }
        s.kind = kind;
        s.open = true;
        s.motion.to(1.0);
    }
}
fn hide() {
    let mut s = Auxiliary::write();
    s.pending = None;
    s.switching_since = None;
    begin_close(&mut s);
}

fn text(t: impl Into<String>) -> Text {
    Text::new(t)
        .size(15.0)
        .color(crate::theme::palette().foreground)
}
fn button(glyph: &str, label: &str, width: f32, handler: impl Fn() + 'static) -> Rectangle {
    let key = format!("aux:{glyph}:{label}");
    let click_key = key.clone();
    let palette = crate::theme::palette();
    let accent = match label {
        "Record region" | "Record screen" | "Stop recording" => palette.red,
        "Shutdown" => palette.yellow,
        "Sleep" | "Annotate" => palette.cyan,
        "Save" | "Confirm" => palette.green,
        _ => palette.blue,
    };
    Rectangle::new()
        .width(width)
        .height(38.0)
        .scale(crate::feedback::scale(&key))
        .radius(18.0)
        .fill(amane::Color::TRANSPARENT)
        .cursor(Pointer)
        .align_child(Center, Center)
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&click_key);
                handler()
            }
        })
        .on_hover({
            let label = label.to_owned();
            move |inside| {
                let mut s = Auxiliary::write();
                if inside {
                    let index = button_index(&s, &label);
                    let target_y = 8.0 + index as f32 * 42.0;
                    if s.tip_motion.precise_value() < 0.01 {
                        s.tip_y = crate::island::motion::Spring::new(target_y);
                    } else {
                        s.tip_y.to(target_y);
                    }
                    let width = match Widget::width(&text(&label).size(13.0)) {
                        amane::Size::Fixed(w) => (w + 32.0).clamp(90.0, 300.0),
                        _ => 160.0,
                    };
                    s.tip_width.to(width);
                    s.tooltip = Some(label.clone());
                    s.tip_deadline = None;
                    s.tip_motion.to(1.0);
                    s.deadline = None;
                } else {
                    s.tip_deadline = Some(std::time::Instant::now() + Duration::from_millis(300));
                }
            }
        })
        .child(
            Text::new(glyph)
                .font("Material Icons Round")
                .size(27.0)
                .tight()
                .color(accent),
        )
}
fn report(action: &str, line: &str) {
    let (kind, message) = line.split_once('\t').unwrap_or(("ERROR", line));
    let mut s = Auxiliary::write();
    match kind {
        // The pick is made: the border goes back to normal before the capture.
        "SELECTED" => {
            s.selecting = false;
            s.tint.to(0.0);
        }
        // Cancelled at any point: leave the whole flow, show nothing.
        "CANCEL" => finish(&mut s),
        "FILE" => {
            s.file = Some(message.into());
            s.file_size = png_size(message);
            s.busy = false;
            s.selecting = false;
            s.tint.to(0.0);
            request_kind(&mut s, Kind::Screenshot);
        }
        "RECORDING" => {
            s.busy = false;
            s.selecting = false;
            s.tint.to(0.0);
            request_kind(&mut s, Kind::Record);
            // Show the stop control briefly, then get out of the way.
            s.deadline = Some(std::time::Instant::now() + Duration::from_secs(3));
        }
        "ERROR" => {
            finish(&mut s);
            s.status = message.into();
            if action == "record" {
                s.record_control = None;
            }
            s.open = true;
            s.motion.to(1.0)
        }
        // DONE: the action is over, nothing stays on screen.
        _ => {
            if action == "record" {
                s.record_control = None;
            }
            finish(&mut s);
        }
    }
}
/// Ends every phase: no tint, no pending pick, no leftover file or status.
fn finish(s: &mut Auxiliary) {
    s.busy = false;
    s.selecting = false;
    s.tint.to(0.0);
    s.worker_pid = None;
    s.file = None;
    s.file_size = None;
    s.status.clear();
    s.confirm = None;
    s.pending = None;
    s.switching_since = None;
}
fn worker(action: &str, mut args: Vec<String>) {
    let action = action.to_owned();
    {
        let mut s = Auxiliary::write();
        if s.busy || s.record_control.is_some() {
            s.status = "Finish the current capture/recording first.".into();
            return;
        }
        if s.kind == Kind::Search
            && s.closing_search.is_none()
            && s.open
            && (action == "launch" || action == "browser")
        {
            s.deferred_action = Some((action.clone(), args));
            begin_close(&mut s);
            return;
        }
        s.busy = true;
        s.pending = None;
        s.switching_since = None;
        s.tip_motion.to(0.0);
        s.status = "Working…".into();
        s.open = false;
        s.motion.to(0.0);
        if action == "screenshot" || action == "record" {
            let color = capture_color(&action);
            if matches!(args.first().map(String::as_str), Some("region" | "window")) {
                s.selecting = true;
                s.tint_color = color;
                s.tint.to(1.0);
            }
            args.push(hex(color));
        }
    }
    std::thread::spawn(move || {
        let result = Command::new("python3")
            .args(["-u", "-c", include_str!("auxiliary/actions.py"), &action])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        match result {
            Ok(mut child) => {
                Auxiliary::write().worker_pid = Some(child.id());
                if action == "record" {
                    Auxiliary::write().record_control =
                        child.stdin.take().map(|stdin| Arc::new(Mutex::new(stdin)));
                }
                if let Some(output) = child.stdout.take() {
                    for line in BufReader::new(output).lines().map_while(Result::ok) {
                        report(&action, &line);
                    }
                }
                let result = child.wait();
                let mut s = Auxiliary::write();
                s.busy = false;
                s.selecting = false;
                s.tint.to(0.0);
                s.worker_pid = None;
                if action == "record" {
                    s.record_control = None;
                }
                let silent_failure = s.status == "Working…";
                if silent_failure {
                    s.status.clear();
                }
                if silent_failure && result.is_ok_and(|v| !v.success()) {
                    s.status = "Action failed. Check capture tools / permissions.".into();
                    s.open = true;
                    s.motion.to(1.0);
                }
            }
            Err(e) => report(&action, &format!("ERROR\t{e}")),
        }
    });
}
fn stop_record() {
    let control = Auxiliary::read().record_control.clone();
    if let Some(control) = control {
        std::thread::spawn(move || {
            if let Ok(mut stdin) = control.lock() {
                let _ = stdin.write_all(b"stop\n");
                let _ = stdin.flush();
            }
        });
    }
}

fn session(action: &str) {
    Auxiliary::write().confirm = Some(action.into());
}
fn list(query: &str) -> Vec<amane::DesktopApp> {
    let needle = query.to_lowercase();
    let apps = Apps::read();
    let mut list = apps
        .list()
        .iter()
        .filter(|a| {
            a.name().to_lowercase().contains(&needle)
                || a.description()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&needle)
        })
        .cloned()
        .collect::<Vec<_>>();
    list.sort_by_key(|a| {
        (
            !a.name().to_lowercase().starts_with(&needle),
            a.name().to_lowercase(),
        )
    });
    list.truncate(6);
    list
}
fn launch(query: String, selected: usize) {
    if let Some(search) = query.strip_prefix("s ") {
        worker("browser", vec![search.into()]);
        return;
    }
    if let Some(app) = list(&query).get(selected).cloned() {
        worker("launch", vec![app.exec().to_owned()]);
    }
}
fn word_range(text: &str, caret: usize, forward: bool) -> std::ops::Range<usize> {
    if forward {
        let mut end = caret;
        let mut in_word = false;
        for (i, c) in text[caret..].char_indices() {
            if !c.is_whitespace() {
                in_word = true;
            } else if in_word {
                break;
            }
            end = caret + i + c.len_utf8();
        }
        caret..end
    } else {
        let mut start = caret;
        let mut in_word = false;
        for (i, c) in text[..caret].char_indices().rev() {
            if !c.is_whitespace() {
                in_word = true;
            } else if in_word {
                break;
            }
            start = i;
        }
        start..caret
    }
}
fn delete_word(s: &mut Auxiliary, forward: bool) {
    let range = word_range(&s.query, s.caret, forward);
    s.caret = range.start;
    s.query.replace_range(range, "");
    s.selected = 0;
    update_caret(s);
}
fn key(key: Key) {
    let mut s = Auxiliary::write();
    if key == Key::Escape {
        s.pending = None;
        s.switching_since = None;
        begin_close(&mut s);
        return;
    }
    if s.kind != Kind::Search || s.closing_search.is_some() {
        return;
    }
    s.deadline = None;
    match key {
        Key::Character('\u{a}') => {
            s.selected = (s.selected + 1).min(list(&s.query).len().saturating_sub(1))
        }
        Key::Character('\u{b}') => s.selected = s.selected.saturating_sub(1),
        Key::Character('\u{17}') => {
            delete_word(&mut s, false);
        }
        Key::Character('\u{4}') => {
            delete_word(&mut s, true);
        }
        Key::Character(c) if !c.is_control() => {
            let pos = s.caret;
            s.query.insert(pos, c);
            s.caret += c.len_utf8();
            s.selected = 0;
            update_caret(&mut s);
        }
        Key::Space => {
            let pos = s.caret;
            s.query.insert(pos, ' ');
            s.caret += 1;
            s.selected = 0;
            update_caret(&mut s);
        }
        Key::Backspace => {
            if let Some((pos, _)) = s.query[..s.caret].char_indices().next_back() {
                let end = s.caret;
                s.query.replace_range(pos..end, "");
                s.caret = pos;
                s.selected = 0;
                update_caret(&mut s);
            }
        }
        Key::Left => {
            s.caret = s.query[..s.caret]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            update_caret(&mut s);
        }
        Key::Right => {
            if let Some(c) = s.query[s.caret..].chars().next() {
                s.caret += c.len_utf8();
                update_caret(&mut s);
            }
        }
        Key::Home => {
            s.caret = 0;
            update_caret(&mut s);
        }
        Key::End => {
            s.caret = s.query.len();
            update_caret(&mut s);
        }
        Key::Down => s.selected = (s.selected + 1).min(list(&s.query).len().saturating_sub(1)),
        Key::Up => s.selected = s.selected.saturating_sub(1),
        Key::Enter => {
            let q = s.query.clone();
            let selected = s.selected;
            drop(s);
            launch(q, selected);
        }
        _ => {}
    }
}

fn pointer(inside: bool) {
    let mut s = Auxiliary::write();
    s.deadline = if inside {
        None
    } else {
        Some(std::time::Instant::now() + Duration::from_millis(3000))
    };
}
fn edge(kind: Kind, inside: bool) {
    if !crate::edge_activation::enabled(edge_name(kind)) {
        return;
    }
    let mut s = Auxiliary::write();
    if s.busy {
        return;
    }
    if !inside {
        if s.kind == kind && s.open {
            s.deadline = Some(std::time::Instant::now() + Duration::from_secs(3));
        }
        return;
    }
    if kind == Kind::Search && s.kind != kind {
        s.query.clear();
        s.caret = 0;
        update_caret(&mut s);
        s.selected = 0;
    }
    request_kind(&mut s, kind);
}
fn button_index(s: &Auxiliary, label: &str) -> usize {
    let labels: Vec<&str> = match s.kind {
        Kind::Screenshot => {
            if s.file.is_some() {
                vec!["Save", "Annotate"]
            } else {
                vec!["Window", "Region", "Full screen"]
            }
        }
        Kind::Record => {
            if s.record_control.is_some() {
                vec!["Stop recording"]
            } else {
                vec!["Record region", "Record screen"]
            }
        }
        Kind::Session => {
            if s.confirm.is_some() {
                vec!["Confirm", "Cancel"]
            } else {
                vec!["Logout", "Restart", "Shutdown", "Sleep", "Lock"]
            }
        }
        _ => vec![],
    };
    labels.iter().position(|v| *v == label).unwrap_or_else(|| {
        labels.len()
            + if label == "Close" && !s.status.is_empty() {
                1
            } else {
                0
            }
    })
}
fn tooltip_hover(inside: bool) {
    let mut s = Auxiliary::write();
    if inside {
        s.tip_deadline = None;
        s.deadline = None;
        s.tip_motion.to(1.0);
    } else {
        s.tip_deadline = Some(std::time::Instant::now() + Duration::from_millis(300));
    }
}
fn vertical_panel(s: &Auxiliary, width: f32) -> Column {
    let mut rows: Vec<Box<dyn Widget>> = vec![];
    let bw = width - 8.0;
    match s.kind {
        Kind::Screenshot => {
            if let Some(file) = &s.file {
                let file = file.clone();
                let saved = file.clone();
                rows.push(Box::new(button("\u{e161}", "Save", bw, move || {
                    worker("save", vec![saved.clone()])
                })));
                rows.push(Box::new(button("\u{e3c9}", "Annotate", bw, move || {
                    worker("edit", vec![file.clone()])
                })));
            } else {
                rows.extend(children![
                    button("\u{e1b0}", "Window", bw, || worker(
                        "screenshot",
                        vec!["window".into()]
                    )),
                    button("\u{e3be}", "Region", bw, || worker(
                        "screenshot",
                        vec!["region".into()]
                    )),
                    button("\u{e30a}", "Full screen", bw, || worker(
                        "screenshot",
                        vec!["full".into()]
                    ))
                ]);
            }
        }
        Kind::Record => {
            if s.record_control.is_some() {
                rows.push(Box::new(button(
                    "\u{e047}",
                    "Stop recording",
                    bw,
                    stop_record,
                )));
            } else {
                rows.extend(children![
                    button("\u{e04b}", "Record region", bw, || worker(
                        "record",
                        vec!["region".into()]
                    )),
                    button("\u{e30a}", "Record screen", bw, || worker(
                        "record",
                        vec!["full".into()]
                    ))
                ]);
            }
        }
        Kind::Session => {
            if let Some(action) = &s.confirm {
                let action = action.clone();

                rows.extend(children![
                    button("\u{e5ca}", "Confirm", bw, move || worker(
                        "session",
                        vec![action.clone()]
                    )),
                    button("\u{e5cd}", "Cancel", bw, || Auxiliary::write().confirm =
                        None)
                ]);
            } else {
                rows.extend(children![
                    button("\u{e9ba}", "Logout", bw, || session("logout")),
                    button("\u{e042}", "Restart", bw, || session("restart")),
                    button("\u{e8ac}", "Shutdown", bw, || session("shutdown")),
                    button("\u{e3a8}", "Sleep", bw, || session("sleep")),
                    button("\u{e897}", "Lock", bw, || session("lock"))
                ]);
            }
        }
        _ => {}
    }
    if !s.status.is_empty() {
        rows.push(Box::new(button("\u{e88e}", &s.status, bw, || {})));
    }
    rows.push(Box::new(button("\u{e5cd}", "Close", bw, hide)));
    Column::new(rows).width(bw).gap(4.0).align(Center)
}
fn search_results(s: &Auxiliary, width: f32) -> Column {
    let mut rows: Vec<Box<dyn Widget>> = vec![];
    if s.query.starts_with("s ") {
        rows.push(Box::new(
            text("↵ Open search or URL in your browser").color(crate::theme::palette().blue),
        ));
    } else if !s.query.is_empty() {
        let apps = list(&s.query);
        if apps.is_empty() {
            rows.push(Box::new(text("No matching applications")));
        }
        // Best match sits nearest the search pill; subsequent results stack upward.
        for (i, app) in apps.iter().enumerate().rev() {
            let app2 = app.clone();
            let key = format!("app:{}", app.name());
            let ck = key.clone();
            let icon = Rectangle::new().width(32.0).height(32.0).radius(10.0);
            let icon = if let Some(path) = app.icon_path() {
                icon.fill(amane::Image::contain(path))
            } else {
                icon.fill(crate::theme::palette().blue)
            };
            rows.push(Box::new(
                Rectangle::new()
                    .width(width - 40.0)
                    .height(52.0)
                    .padding(10.0)
                    .radius(16.0)
                    .fill(amane::Color::TRANSPARENT)
                    .scale(crate::feedback::scale(&key))
                    .cursor(Pointer)
                    .on_click(move |b| {
                        if b == Button::Left {
                            crate::feedback::click(&ck);
                            worker("launch", vec![app2.exec().into()]);
                        }
                    })
                    .child(
                        Row::new(children![
                            icon,
                            Rectangle::new()
                                .width(width - 114.0)
                                .height(32.0)
                                .align_child(Start, Center)
                                .child(
                                    text(app.name())
                                        .color(if i == s.selected {
                                            crate::theme::palette().blue
                                        } else {
                                            crate::theme::palette().foreground
                                        })
                                        .elide()
                                )
                        ])
                        .gap(12.0)
                        .align(Center),
                    ),
            ));
        }
    }
    Column::new(rows).width(width - 40.0).gap(8.0)
}
fn shader() -> std::path::PathBuf {
    std::path::Path::new(file!())
        .parent()
        .unwrap()
        .join("island/goo.wgsl")
}
fn area(x: f32, y: f32, w: f32, h: f32) -> InputArea {
    InputArea {
        x: x.round() as i32,
        y: y.round() as i32,
        width: w.round() as i32,
        height: h.round() as i32,
    }
}
fn slide_content_alpha(p: f32) -> f32 {
    ((p - 0.30) / 0.45).clamp(0.0, 1.0)
}
fn slide_radius(p: f32) -> f32 {
    18.0 + 5.0 * ((0.30 - p) / 0.30).clamp(0.0, 1.0)
}
fn edge_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Screenshot => "screenshot",
        Kind::Record => "record",
        Kind::Session => "session",
        Kind::Search => "search",
    }
}
fn sensors(sw: f32, sh: f32) -> [(Kind, f32, f32, f32, f32); 4] {
    [
        (Kind::Record, 0.0, sh * 0.06, 4.0, sh * 0.26),
        (Kind::Session, 0.0, sh * 0.55, 4.0, sh * 0.36),
        (Kind::Screenshot, sw - 4.0, sh * 0.55, 4.0, sh * 0.36),
        (Kind::Search, sw * 0.25, sh - 4.0, sw * 0.5, 4.0),
    ]
}
fn results_rect(y: f32, pill_h: f32, results_h: f32, gap: f32, p: f32) -> (f32, f32) {
    (y - (gap + results_h) * p, pill_h + (results_h - pill_h) * p)
}
fn modal_active(s: &Auxiliary, p: f32) -> bool {
    !s.busy
        && s.record_control.is_none()
        && (s.open || p > 0.001 || s.pending.is_some() || s.closing_search.is_some())
}

fn dormant(s: &Auxiliary, p: f32) -> bool {
    !s.open
        && !s.busy
        && !s.selecting
        && p <= 0.001
        && s.pending.is_none()
        && s.closing_search.is_none()
        && s.record_control.is_none()
}
pub fn view() -> LayerWindow {
    let s = Auxiliary::read();
    let p = s.motion.precise_value().clamp(0.0, 1.0);
    // Keep the surface mapped but invisible when dormant during fullscreen;
    // unmapping makes mango close the surface and shut the shell down.
    if crate::island::fullscreen::blocked() && dormant(&s, p) {
        return LayerWindow::new()
            .width(Full)
            .height(Full)
            .input_region(vec![])
            .anchor_horizontal(Horizontal::Middle)
            .anchor_vertical(Vertical::Top)
            .namespace("amane-auxiliary")
            .child(amane::Stack::new(vec![]).width(1.0).height(1.0));
    }
    let (sw, sh) = amane::window_size();
    let sw = sw.max(800.0);
    let sh = sh.max(600.0);
    let sensors = sensors(sw, sh);
    let mut input = vec![];
    let mut widgets: Vec<Box<dyn Widget>> = vec![];
    let modal = modal_active(&s, p);
    if modal {
        input.push(area(0.0, 0.0, sw, sh));
        // No dimming: the shield is invisible and only catches click-away.
        widgets.push(Box::new(
            Rectangle::new()
                .width(sw)
                .height(sh)
                .on_click(|_| hide())
                .on_scroll(|_| {}),
        ));
    }
    for (kind, x, y, w, h) in sensors {
        if !s.busy && crate::edge_activation::enabled(edge_name(kind)) {
            input.push(area(x, y, w, h));
            widgets.push(Box::new(
                Rectangle::new()
                    .width(w)
                    .height(h)
                    .translate(x, y)
                    .on_hover(move |inside| edge(kind, inside))
                    .cursor(Pointer),
            ));
        }
    }
    if s.open || p > 0.001 {
        if s.kind == Kind::Search {
            let width = 560.0;
            let x = (sw - width) / 2.0;
            let pill_h = 64.0;
            let y = sh - 28.0 - pill_h;
            let rp = s.results.value().clamp(0.0, 1.0);
            let rh = s.body_height.value().max(0.0);
            let gap = 12.0;
            let body_y = y + (pill_h + 32.0) * (1.0 - p);
            let (result_y, result_h) = results_rect(body_y, pill_h, rh, gap, rp);
            let top = (y - gap - rh - 36.0).max(0.0);
            let height = sh - top;
            // Full-size pill translating vertically through a solid bottom
            // half-plane. Never shrink either shape to reveal it.
            widgets.push(Box::new(
                Rectangle::new()
                    .width(width + 72.0)
                    .height(height)
                    .translate(x - 36.0, top)
                    .shader(shader())
                    .shader_values(crate::theme::shader_values(vec![
                        [-48.0, sh - top - 4.0, width + 168.0, 52.0],
                        [36.0, body_y - top, width, pill_h],
                        [
                            0.0,
                            32.0 + 12.0 * ((0.3 - p) / 0.3).clamp(0.0, 1.0),
                            12.0 * (p * std::f32::consts::PI).sin().max(0.0),
                            0.48,
                        ],
                    ])),
            ));
            if rp > 0.001 {
                // One continuous rounded field: at rp=0 both shapes match
                // the exact pill rectangle, so no scissors or square corners.
                widgets.push(Box::new(
                    Rectangle::new()
                        .width(width + 72.0)
                        .height(height)
                        .translate(x - 36.0, top)
                        .shader(shader())
                        .shader_values(crate::theme::shader_values(vec![
                            [36.0, body_y - top, width, pill_h],
                            [36.0, result_y - top, width, result_h],
                            [
                                32.0,
                                32.0 + (24.0 - 32.0) * rp,
                                12.0 * (rp * std::f32::consts::PI).sin().max(0.0),
                                0.48,
                            ],
                        ])),
                ));
                widgets.push(Box::new(
                    Rectangle::new()
                        .width(width)
                        .height((body_y - top).max(0.0))
                        .translate(x, top)
                        .clip()
                        .child(
                            Rectangle::new()
                                .width(width)
                                .height(rh)
                                .padding(20.0)
                                .translate(0.0, result_y - top)
                                .opacity(slide_content_alpha(rp) * slide_content_alpha(p))
                                .on_hover(pointer)
                                .child(search_results(&s, width)),
                        ),
                ));
                if slide_content_alpha(rp) > 0.05 {
                    let visible = (body_y - result_y).min(rh).max(0.0);
                    input.push(area(x, result_y, width, visible));
                }
            }
            let edit = s.edited.elapsed().as_secs_f32();
            // Text, caret and horizontal scrolling use the same edit position.
            // Do not spring the caret behind text that has already been inserted.
            // Edits reset the blink so the insertion point stays visible.
            let blink = if edit < 0.5 {
                1.0
            } else {
                0.55 + 0.45 * (edit * std::f32::consts::TAU).cos()
            };
            if s.open {
                amane::request_frame();
            }
            let caret = s.caret_x;
            let offset = (caret - 490.0).max(0.0);
            let mut text_widgets: Vec<Box<dyn Widget>> = vec![];
            if s.query.is_empty() {
                text_widgets.push(Box::new(
                    text("Search applications…")
                        .size(17.0)
                        .color(crate::theme::palette().secondary),
                ));
            } else {
                text_widgets.push(Box::new(
                    Rectangle::new()
                        .width(2400.0)
                        .height(24.0)
                        .translate(-offset, 0.0)
                        .child(text(&s.query).size(17.0)),
                ));
            }
            text_widgets.push(Box::new(
                Rectangle::new()
                    .width(2.0)
                    .height(21.0)
                    .radius(1.0)
                    .fill(crate::theme::palette().blue)
                    .translate(caret - offset, 1.0)
                    .opacity(blink),
            ));
            widgets.push(Box::new(
                Rectangle::new()
                    .width(width)
                    .height(pill_h)
                    .padding(20.0)
                    .translate(x, body_y)
                    .opacity(slide_content_alpha(p))
                    .on_hover(pointer)
                    .on_click(|_| {})
                    .child(
                        Rectangle::new()
                            .width(width - 40.0)
                            .height(24.0)
                            .clip()
                            .child(
                                amane::Stack::new(text_widgets)
                                    .width(width - 40.0)
                                    .height(24.0),
                            ),
                    ),
            ));
            if slide_content_alpha(p) > 0.05 {
                input.push(area(x, body_y, width, pill_h));
            }
        } else {
            let width = 46.0;
            let panel = vertical_panel(&s, width);
            let h = match Widget::height(&panel) {
                amane::Size::Fixed(h) => h + 16.0,
                _ => 180.0,
            };
            let x = if s.kind == Kind::Screenshot {
                sw - width - 12.0
            } else {
                12.0
            };
            let y = if s.kind == Kind::Record {
                sh * 0.07
            } else {
                (sh - h - 28.0).max(sh * 0.35)
            };
            let field_x = if s.kind == Kind::Screenshot {
                x - 36.0
            } else {
                0.0
            };
            let fw = width + 64.0;
            let field_y = y - 36.0;
            let fh = h + 72.0;
            // Translate a full-size body through its lateral border; neither
            // dimension depends on reveal progress. Content follows the same x.
            let right = s.kind == Kind::Screenshot;
            let body_x = if right {
                x + (width + 32.0) * (1.0 - p)
            } else {
                x - (width + 32.0) * (1.0 - p)
            };
            let alpha = slide_content_alpha(p);
            let radius = slide_radius(p);
            // The capture itself, shown over the screen while Save/Annotate is up.
            if s.kind == Kind::Screenshot && s.open && !s.busy {
                if let (Some(file), Some((iw, ih))) = (&s.file, s.file_size) {
                    let fit = (sw * 0.55 / iw).min(sh * 0.55 / ih).min(1.0);
                    let (pw, ph) = ((iw * fit).round(), (ih * fit).round());
                    widgets.push(Box::new(
                        Rectangle::new()
                            .width(pw)
                            .height(ph)
                            .translate(((sw - pw) / 2.0).round(), ((sh - ph) / 2.0).round())
                            .radius(16.0)
                            .clip()
                            .fill(amane::Image::contain(std::path::PathBuf::from(file)))
                            .border(3.0, crate::theme::palette().blue)
                            .on_click(|_| {}),
                    ));
                }
            }
            let source_x = if right { sw - 4.0 - field_x } else { 0.0 };
            widgets.push(Box::new(
                Rectangle::new()
                    .width(fw)
                    .height(fh)
                    .translate(field_x, field_y)
                    .shader(shader())
                    .shader_values(crate::theme::shader_values(vec![
                        // Integrate the solid exterior half-plane, not an isolated 4px
                        // strip (which disappears under the Gaussian threshold).
                        [if right { source_x } else { -48.0 }, -48.0, 52.0, fh + 96.0],
                        [body_x - field_x, 36.0, width, h],
                        [
                            0.0,
                            radius,
                            12.0 * (p * std::f32::consts::PI).sin().max(0.0),
                            0.48,
                        ],
                    ])),
            ));
            let tp = s.tip_motion.precise_value().clamp(0.0, 1.0) * p;
            if tp > 0.001 {
                if let Some(label) = &s.tooltip {
                    let tw = s.tip_width.precise_value();
                    let th = 38.0;
                    let sy = y + s.tip_y.precise_value();
                    let tx = if right {
                        x - tw - 12.0
                    } else {
                        x + width + 12.0
                    };
                    let tip_x =
                        body_x + width / 2.0 + ((tx + tw / 2.0) - (body_x + width / 2.0)) * tp
                            - tw / 2.0;
                    let tip_w = 38.0 + (tw - 38.0) * tp;
                    let tip_h = 38.0;
                    let fx = body_x.min(tx) - 36.0;
                    let fy = y.min(sy) - 36.0;
                    let fw = (body_x + width).max(tx + tw) - fx + 36.0;
                    let fh = (y + h).max(sy + th) - fy + 36.0;
                    widgets.push(Box::new(
                        Rectangle::new()
                            .width(fw)
                            .height(fh)
                            .translate(fx, fy)
                            .shader(shader())
                            .shader_values(crate::theme::shader_values(vec![
                                [body_x - fx, y - fy, width, h],
                                [tip_x + (tw - tip_w) / 2.0 - fx, sy - fy, tip_w, tip_h],
                                [
                                    radius,
                                    14.0 + 5.0 * ((0.3 - tp) / 0.3).clamp(0.0, 1.0),
                                    11.0 * (tp * std::f32::consts::PI).sin().max(0.0),
                                    0.48,
                                ],
                            ])),
                    ));
                    widgets.push(Box::new(
                        Rectangle::new()
                            .width(tw)
                            .height(th)
                            .translate(tx, sy)
                            .opacity(((tp - 0.4) / 0.45).clamp(0.0, 1.0))
                            .on_hover(tooltip_hover)
                            .align_child(Center, Center)
                            .child(text(label).size(13.0)),
                    ));
                    if tp > 0.5 {
                        input.push(area(tx, sy, tw, th));
                        input.push(area(if right { tx } else { x + width }, sy, tw + 12.0, th));
                    }
                }
            }
            widgets.push(Box::new(
                Rectangle::new()
                    .width(width)
                    .height(h)
                    .padding(amane::Padding {
                        top: 8.0,
                        bottom: 8.0,
                        left: 4.0,
                        right: 4.0,
                    })
                    .translate(body_x, y)
                    .opacity(alpha)
                    .on_hover(pointer)
                    .on_click(|_| {})
                    .clip()
                    .child(panel),
            ));
            if alpha > 0.05 {
                input.push(area(
                    body_x.max(0.0),
                    y,
                    (width + body_x.min(0.0)).min(sw - body_x.max(0.0)).max(0.0),
                    h,
                ));
            }
        }
    }
    LayerWindow::new()
        .width(Full)
        .height(Full)
        .anchor_horizontal(Horizontal::Middle)
        .anchor_vertical(Vertical::Top)
        .namespace("amane-auxiliary")
        .input_region(input)
        .keyboard(if modal && s.open {
            Keyboard::Exclusive
        } else {
            Keyboard::None
        })
        .on_key(key)
        .child(amane::Stack::new(widgets).width(sw).height(sh))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn results_merge_exactly_into_pill() {
        assert_eq!(results_rect(900.0, 64.0, 320.0, 12.0, 0.0), (900.0, 64.0));
        assert_eq!(results_rect(900.0, 64.0, 320.0, 12.0, 1.0), (568.0, 320.0));
    }
    #[test]
    fn search_closes_results_before_pill() {
        let mut s = Auxiliary::new();
        s.kind = Kind::Search;
        s.open = true;
        s.motion = crate::island::motion::Spring::new(1.0);
        s.query = "test".into();
        s.results = amane::Animation::new(1.0);
        s.caret = 4;
        begin_close(&mut s);
        assert!(s.open);
        assert_eq!(s.motion.precise_value(), 1.0);
        assert!(s.closing_search.is_some());
        s.closing_search = Some(std::time::Instant::now() - Duration::from_millis(1500));
        s.update();
        assert!(!s.open);
    }
    #[test]
    fn caret_uses_utf8_boundaries() {
        let mut s = Auxiliary::new();
        s.query = "日a".into();
        s.caret = 3;
        update_caret(&mut s);
        assert_eq!(&s.query[..s.caret], "日");
    }
    #[test]
    fn rounding_waits_for_content_to_disappear() {
        for p in [0.31, 0.5, 0.8, 1.0] {
            assert_eq!(slide_radius(p), 18.0);
        }
        assert_eq!(slide_content_alpha(0.30), 0.0);
        assert!(slide_radius(0.1) > 18.0);
    }
    #[test]
    fn lateral_sensor_geometry_is_disjoint() {
        let s = sensors(1920.0, 1080.0);
        assert!(s[0].2 + s[0].4 < s[1].2);
        assert_eq!(s[2].1 + s[2].3, 1920.0);
        assert_eq!(s[3].2 + s[3].4, 1080.0);
        assert_eq!(s[3].3, 960.0);
    }
    #[test]
    fn leaving_edge_schedules_hide_without_resetting_query() {
        let mut s = Auxiliary::new();
        s.query = "keep".into();
        s.open = true;
        s.deadline = Some(std::time::Instant::now() - Duration::from_secs(1));
        assert!(s.update());
        assert!(!s.open);
        assert_eq!(s.query, "keep");
    }
    #[test]
    fn changing_menu_preserves_old_until_exit() {
        let mut s = Auxiliary::new();
        s.open = true;
        s.motion = crate::island::motion::Spring::new(1.0);
        request_kind(&mut s, Kind::Record);
        assert!(s.kind == Kind::Screenshot);
        assert!(s.pending == Some(Kind::Record));
        assert!(!s.open);
        s.switching_since = Some(std::time::Instant::now() - Duration::from_secs(1));
        s.update();
        assert!(s.kind == Kind::Record);
        assert!(s.open);
        assert!(s.pending.is_none());
    }
    #[test]
    fn tooltip_matches_icon_positions() {
        let mut s = Auxiliary::new();
        assert_eq!(button_index(&s, "Region"), 1);
        s.kind = Kind::Session;
        assert_eq!(button_index(&s, "Lock"), 4);
    }
    #[test]
    fn capture_work_disables_modal_shield() {
        let mut s = Auxiliary::new();
        s.open = true;
        assert!(modal_active(&s, 1.0));
        s.busy = true;
        assert!(!modal_active(&s, 1.0));
        s.busy = false;
        s.open = false;
        assert!(!modal_active(&s, 0.0));
    }
    #[test]
    fn cancel_or_finish_leaves_nothing_behind() {
        let mut s = Auxiliary::new();
        s.selecting = true;
        s.busy = true;
        s.file = Some("/tmp/x.png".into());
        s.status = "Working…".into();
        s.tint.to(1.0);
        finish(&mut s);
        assert!(!s.selecting && !s.busy);
        assert!(s.file.is_none() && s.status.is_empty());
        let mut s = Auxiliary::new();
        s.kind = Kind::Screenshot;
        s.open = true;
        s.file = Some("/tmp/x.png".into());
        s.status = "Copied".into();
        begin_close(&mut s);
        assert!(!s.open && s.file.is_none() && s.status.is_empty());
    }
    #[test]
    fn cancelling_deletes_the_unsaved_capture() {
        let path = std::env::temp_dir().join("amane-shot-test.png");
        std::fs::write(&path, b"x").unwrap();
        let mut s = Auxiliary::new();
        s.open = true;
        s.file = Some(path.to_string_lossy().into_owned());
        begin_close(&mut s);
        assert!(!path.exists() && s.file.is_none());
        // Anything not named like our scratch captures is never touched.
        let other = std::env::temp_dir().join("amane-keep.png");
        std::fs::write(&other, b"x").unwrap();
        s.file = Some(other.to_string_lossy().into_owned());
        discard_capture(&mut s);
        assert!(other.exists());
        let _ = std::fs::remove_file(other);
    }
    #[test]
    fn capture_colours_differ_between_screenshot_and_recording() {
        assert_ne!(capture_color("screenshot"), capture_color("record"));
    }
    #[test]
    fn word_deletion_bounds() {
        assert_eq!(word_range("one two", 7, false), 4..7);
        assert_eq!(word_range("one two", 0, true), 0..3);
        assert_eq!(word_range("日 本", 3, false), 0..3);
    }
    #[test]
    fn empty_search_closes_without_merge_wait() {
        let mut s = Auxiliary::new();
        s.kind = Kind::Search;
        s.open = true;
        begin_close(&mut s);
        assert!(s.close_wait.is_zero());
    }
    #[test]
    fn query_prefix_is_explicit() {
        assert!("s query".starts_with("s "));
        assert!(!"spotify".starts_with("s "));
    }
}
