//! Small analytic click springs shared by all button factories.
use amane::Service;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
struct Clicks {
    times: BTreeMap<String, Instant>,
}
impl Service for Clicks {
    fn new() -> Self {
        Self {
            times: BTreeMap::new(),
        }
    }
    fn listen() {}
}
pub fn click(key: &str) {
    let mut s = Clicks::write();
    s.times.retain(|_, t| t.elapsed() < Duration::from_secs(1));
    s.times.insert(key.into(), Instant::now());
}
pub fn scale(key: &str) -> f32 {
    let s = Clicks::read();
    let Some(start) = s.times.get(key) else {
        return 1.0;
    };
    let t = start.elapsed().as_secs_f32();
    if t >= 0.16 {
        return 1.0;
    }
    amane::request_frame();
    press_scale(t)
}
fn press_scale(t: f32) -> f32 {
    // One clear 40ms press followed by a 120ms ease-out. No oscillating
    // subpixel tail, and exactly neutral once complete.
    if t < 0.04 {
        1.0 - 0.10 * (t / 0.04).clamp(0.0, 1.0)
    } else {
        let u = ((t - 0.04) / 0.12).clamp(0.0, 1.0);
        0.90 + 0.10 * (1.0 - (1.0 - u).powi(3))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn click_has_no_lingering_tail() {
        assert_eq!(press_scale(0.0), 1.0);
        assert!((press_scale(0.04) - 0.9).abs() < 0.001);
        assert_eq!(press_scale(0.16), 1.0);
        assert_eq!(press_scale(1.0), 1.0);
    }
}
