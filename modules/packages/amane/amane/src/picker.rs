//! Full-screen colour picker: freeze the screen, sample the pixel under the
//! pointer, and copy it. The capture is taken once when the picker opens, so
//! sampling is local and instant; the overlay itself is never in the capture.
use amane::{
    Button, Center, Color, Column, Full, Horizontal, InputArea, Key, Keyboard, LayerWindow,
    Rectangle, Row, Service, Stack, Text, Vertical, Weight, children,
};
use std::{io::Read, time::Duration};

/// Side of the magnifier, in screen pixels sampled. Odd so the centre is exact.
const SAMPLE: i32 = 15;
/// Rendered size of one magnified screen pixel.
const CELL: f32 = 11.0;
/// The grid the magnifier draws, matching SAMPLE.
const GRID: i32 = SAMPLE;

struct Frame {
    width: u32,
    height: u32,
    /// Raw RGB triples, row-major, from a P6 netpbm image.
    pixels: Vec<u8>,
}

impl Frame {
    fn sample(&self, x: i32, y: i32) -> Color {
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        let at = (y * self.width as usize + x) * 3;
        match self.pixels.get(at..at + 3) {
            Some([r, g, b]) => Color::rgb(*r, *g, *b),
            _ => Color::BLACK,
        }
    }
}

/// Reads a plain P6 netpbm image (grim's ppm output), returning its pixels.
fn parse_ppm(bytes: &[u8]) -> Option<Frame> {
    // Header: magic, width, height, maxval, each whitespace separated, then a
    // single whitespace byte before the binary raster. Comments are skipped.
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'#' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if start == i {
            return None;
        }
        fields.push(std::str::from_utf8(&bytes[start..i]).ok()?);
    }
    if fields[0] != "P6" {
        return None;
    }
    let width: u32 = fields[1].parse().ok()?;
    let height: u32 = fields[2].parse().ok()?;
    let maxval: u32 = fields[3].parse().ok()?;
    if maxval != 255 || width == 0 || height == 0 {
        return None;
    }
    // Exactly one whitespace byte ends the header.
    if i >= bytes.len() {
        return None;
    }
    i += 1;
    let needed = width as usize * height as usize * 3;
    let pixels = bytes.get(i..i + needed)?.to_vec();
    Some(Frame {
        width,
        height,
        pixels,
    })
}

pub struct Picker {
    active: bool,
    ready: bool,
    frame: Option<Frame>,
    /// PNG of the same capture, drawn full-screen as the frozen backdrop.
    display: Option<std::path::PathBuf>,
    /// Latest pointer position in surface coordinates.
    cursor: (f32, f32),
    motion: crate::island::motion::Spring,
    error: Option<String>,
}

impl Service for Picker {
    fn new() -> Self {
        Self {
            active: false,
            ready: false,
            frame: None,
            display: None,
            cursor: (-1000.0, -1000.0),
            motion: crate::island::motion::Spring::new(0.0),
            error: None,
        }
    }
}

/// Where the picker keeps its frozen frame and backdrop.
fn frame_path() -> std::path::PathBuf {
    crate::island::settings::state_dir().join("picker-frame.png")
}

/// Runs grim twice at the same instant: a PNG drawn as the frozen backdrop
/// (the shell cannot read netpbm as an image) and a PPM parsed for sampling.
fn capture() -> Result<(Frame, std::path::PathBuf), String> {
    let display = frame_path();
    if let Some(parent) = display.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let png = std::process::Command::new("grim")
        .args(["-t", "png"])
        .arg(&display)
        .output()
        .map_err(|_| "grim is not installed".to_string())?;
    if !png.status.success() {
        return Err("grim could not capture the screen".into());
    }
    let mut child = std::process::Command::new("grim")
        .args(["-t", "ppm", "-"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| "grim is not installed".to_string())?;
    let mut bytes = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout
            .read_to_end(&mut bytes)
            .map_err(|_| "could not read grim output".to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|_| "grim did not finish".to_string())?;
    if !output.status.success() {
        return Err("grim could not capture the screen".into());
    }
    let frame = parse_ppm(&bytes).ok_or_else(|| String::from("grim returned an unreadable image"))?;
    Ok((frame, display))
}

pub fn toggle() {
    let start = { !Picker::read().active };
    if start {
        // Clear the shell off the screen and grab the frozen frame *before* the
        // overlay is made visible, so the capture never includes our own surface.
        let _ = crate::auxiliary::command(&["hide".to_string()]);
        crate::island::hide_for_picker();
        // Let the compositor drop the hidden surfaces before grabbing.
        std::thread::sleep(Duration::from_millis(160));
        let outcome = capture();
        let mut state = Picker::write();
        match outcome {
            Ok((frame, display)) => {
                state.frame = Some(frame);
                state.display = Some(display);
                state.ready = true;
                state.error = None;
            }
            Err(message) => {
                state.frame = None;
                state.display = None;
                state.ready = false;
                state.error = Some(message);
            }
        }
        state.active = true;
        state.cursor = (-1000.0, -1000.0);
        state.motion.to(1.0);
    } else {
        close();
    }
}

pub fn close() {
    let mut state = Picker::write();
    state.active = false;
    state.ready = false;
    state.frame = None;
    state.display = None;
    state.motion.to(0.0);
    drop(state);
    let _ = std::fs::remove_file(frame_path());
}

fn sample_under_cursor() -> Option<Color> {
    let state = Picker::read();
    let frame = state.frame.as_ref()?;
    let (x, y) = state.cursor;
    if x < 0.0 || y < 0.0 {
        return None;
    }
    Some(frame.sample(x as i32, y as i32))
}

/// Left click: copy the sampled colour straight away as hex, then exit.
fn pick() {
    let Some(color) = sample_under_cursor() else {
        return;
    };
    let hex = color.hex();
    // wl-copy forks a clipboard server and must not inherit captured pipes.
    amane::spawn(&format!("wl-copy --type text/plain '{hex}'"));
    let dir = crate::island::settings::state_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("picked-color"), format!("{hex}\n"));
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    let hook = config.join("amane/hooks/color");
    if hook.is_file() {
        let path = hook.display().to_string().replace('\'', "'\\''");
        amane::spawn(&format!("'{path}' '{hex}'"));
    }
    close();
}

/// Right click: hand the sampled colour to the colour popup, which closes the
/// picker and lets the user click hex or rgb to copy.
fn choose_format() {
    let Some(color) = sample_under_cursor() else {
        return;
    };
    // Hand the pixel over before dismissing: `close` clears the frozen frame.
    crate::island::popups::color_at(color);
    close();
}

pub fn view() -> LayerWindow {
    let state = Picker::read();
    let p = state.motion.precise_value().clamp(0.0, 1.0);
    let (vw, vh) = amane::window_size();
    let sw = vw.max(800.0);
    let sh = vh.max(600.0);
    if !state.active && p <= 0.001 {
        return LayerWindow::new()
            .width(Full)
            .height(Full)
            .keyboard(Keyboard::None)
            .input_region(vec![])
            .anchor_horizontal(Horizontal::Middle)
            .anchor_vertical(Vertical::Top)
            .namespace("amane-picker")
            .child(Stack::new(vec![]).width(1.0).height(1.0));
    }
    let opacity = p;
    let mut layers: Vec<Box<dyn amane::Widget>> = vec![];
    // The frozen capture is drawn full-screen, so the overlay never depends on
    // the desktop showing through a translucent surface.
    if let Some(display) = &state.display {
        layers.push(Box::new(
            Rectangle::new()
                .width(sw)
                .height(sh)
                .fill(amane::Image::stretch(display.clone()))
                .opacity(opacity),
        ));
    } else {
        // Still grabbing the screen: an opaque backdrop reads as intentional
        // instead of letting the transparent surface composite over black.
        layers.push(Box::new(
            Rectangle::new()
                .width(sw)
                .height(sh)
                .fill(Color::rgba(8, 9, 15, 255))
                .opacity(opacity),
        ));
    }
    if state.ready {
        layers.extend(cursor_widgets(&state, sw, sh, opacity));
    }
    let overlay = Rectangle::new()
        .width(sw)
        .height(sh)
        .fill(Color::TRANSPARENT)
        .on_move(|point| move_to(point.x, point.y))
        .on_click(|button| match button {
            Button::Left => pick(),
            Button::Right => choose_format(),
            _ => {}
        })
        .on_scroll(|_| {})
        .child(
            Stack::new(layers)
                .width(sw)
                .height(sh),
        );
    LayerWindow::new()
        .width(Full)
        .height(Full)
        .anchor_horizontal(Horizontal::Middle)
        .anchor_vertical(Vertical::Top)
        .namespace("amane-picker")
        .input_region(vec![InputArea {
            x: 0,
            y: 0,
            width: sw as i32,
            height: sh as i32,
        }])
        .keyboard(Keyboard::Exclusive)
        .on_key(|key| {
            if key == Key::Escape {
                close()
            }
        })
        .child(overlay)
}

fn cursor_widgets(state: &Picker, sw: f32, sh: f32, opacity: f32) -> Vec<Box<dyn amane::Widget>> {
    let mut widgets: Vec<Box<dyn amane::Widget>> = vec![];
    let (cx, cy) = state.cursor;
    let frame = state.frame.as_ref();
    let color = frame
        .map(|f| f.sample(cx as i32, cy as i32))
        .unwrap_or(Color::BLACK);
    // Crosshair lines through the pointer.
    let hair = |w: f32, h: f32, x: f32, y: f32| {
        Box::new(
            Rectangle::new()
                .width(w)
                .height(h)
                .translate(x, y)
                .fill(Color::rgba(0, 0, 0, 200))
                .opacity(opacity),
        ) as Box<dyn amane::Widget>
    };
    widgets.push(hair(1.0, sh, cx, 0.0));
    widgets.push(hair(sw, 1.0, 0.0, cy));
    // Magnifier: a grid of screen pixels around the cursor, plus its centre cell.
    let half = SAMPLE / 2;
    let grid = GRID as f32 * CELL;
    let gx = (cx - grid / 2.0).clamp(0.0, (sw - grid).max(0.0));
    let gy = (cy - grid / 2.0).clamp(0.0, (sh - grid).max(0.0));
    widgets.push(Box::new(
        Rectangle::new()
            .width(grid + 8.0)
            .height(grid + 8.0)
            .translate(gx - 4.0, gy - 4.0)
            .fill(Color::rgba(20, 20, 24, 235))
            .radius(10.0)
            .opacity(opacity),
    ));
    if let Some(frame) = frame {
        for row in 0..GRID {
            for col in 0..GRID {
                let sx = cx as i32 + col - half;
                let sy = cy as i32 + row - half;
                let cell = frame.sample(sx, sy);
                let center = row == half && col == half;
                let w = if center { CELL } else { CELL - 0.0 };
                let mut rect = Rectangle::new()
                    .width(w)
                    .height(w)
                    .translate(gx + col as f32 * CELL, gy + row as f32 * CELL)
                    .fill(cell)
                    .opacity(opacity);
                if center {
                    rect = rect.border(3.0, Color::WHITE);
                }
                widgets.push(Box::new(rect));
            }
        }
    }
    // Readout: a swatch, the hex, and its rgb parts.
    let readout_w = 268.0;
    let readout_h = 96.0;
    let rx = (cx + 40.0).clamp(0.0, (sw - readout_w).max(0.0));
    let ry = (cy + 40.0).clamp(0.0, (sh - readout_h).max(0.0));
    let hex = color.hex();
    let rgb = format!(
        "r {}  g {}  b {}",
        color.red(),
        color.green(),
        color.blue()
    );
    widgets.push(Box::new(
        Rectangle::new()
            .width(readout_w)
            .height(readout_h)
            .translate(rx, ry)
            .radius(14.0)
            .fill(Color::rgb(20, 20, 24))
            .padding(16.0)
            .opacity(opacity)
            .align_child(Center, Center)
            .child(
                Row::new(children![
                    Rectangle::new().width(56.0).height(56.0).radius(12.0).fill(color),
                    Column::new(children![
                        Text::new(&hex)
                            .size(22.0)
                            .weight(Weight::Bold)
                            .color(Color::WHITE),
                        Text::new(rgb)
                            .size(14.0)
                            .color(Color::rgb(170, 176, 186)),
                    ])
                    .width(168.0)
                    .gap(6.0)
                    .align(Center),
                ])
                .gap(16.0)
                .align(Center),
            ),
    ));
    widgets
}

fn move_to(x: f32, y: f32) {
    let mut state = Picker::write();
    state.cursor = (x, y);
}

pub fn command(arguments: &[String]) -> String {
    match arguments.first().map(String::as_str) {
        Some("hide") => close(),
        Some("status") => {
            let state = Picker::read();
            let sampled = state
                .frame
                .as_ref()
                .map(|f| f.sample(state.cursor.0 as i32, state.cursor.1 as i32).hex())
                .unwrap_or_else(|| String::from("none"));
            return format!(
                "active={} ready={} cursor=({:.0},{:.0}) sample={}",
                state.active, state.ready, state.cursor.0, state.cursor.1, sampled
            );
        }
        None | Some("show" | "toggle") => toggle(),
        Some("copy") => choose_format(),
        Some(other) => return format!("unknown picker argument: {other}"),
    }
    String::from("ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ppm(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
        bytes.extend_from_slice(pixels);
        bytes
    }

    #[test]
    fn parses_plain_p6_and_samples_pixels() {
        let pixels = [255, 0, 0, 0, 255, 0, 0, 0, 255, 10, 20, 30];
        let frame = parse_ppm(&ppm(2, 2, &pixels)).unwrap();
        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 2);
        assert_eq!(frame.sample(0, 0), Color::rgb(255, 0, 0));
        assert_eq!(frame.sample(1, 0), Color::rgb(0, 255, 0));
        assert_eq!(frame.sample(0, 1), Color::rgb(0, 0, 255));
        assert_eq!(frame.sample(1, 1), Color::rgb(10, 20, 30));
    }

    #[test]
    fn parser_tolerates_comments_and_whitespace() {
        let mut bytes = b"P6\n# made by grim\n2 1\n255\n".to_vec();
        bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let frame = parse_ppm(&bytes).unwrap();
        assert_eq!(frame.sample(0, 0), Color::rgb(1, 2, 3));
        assert_eq!(frame.sample(1, 0), Color::rgb(4, 5, 6));
    }

    #[test]
    fn parser_rejects_truncated_and_foreign_images() {
        assert!(parse_ppm(b"P6\n2 2\n255\n\0\0").is_none());
        assert!(parse_ppm(b"P3\n1 1\n255\n0 0 0\n").is_none());
        assert!(parse_ppm(b"").is_none());
    }

    #[test]
    fn sampling_clamps_to_the_frame() {
        let frame = parse_ppm(&ppm(2, 2, &[0; 12])).unwrap();
        assert_eq!(frame.sample(-5, -5), Color::rgb(0, 0, 0));
        assert_eq!(frame.sample(99, 99), Color::rgb(0, 0, 0));
    }
}
