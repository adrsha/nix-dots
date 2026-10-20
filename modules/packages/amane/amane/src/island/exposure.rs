//! Occupied-workspace edge reveal, independent of island layout locks.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
// Published by Island::sync without taking its lock from this service (OSD
// events can hold that lock while requesting exposure).
static COLLAPSED: AtomicBool = AtomicBool::new(true);
pub(super) fn collapsed(ready: bool) {
    COLLAPSED.store(ready, Ordering::Relaxed);
}
fn can_hide() -> bool {
    COLLAPSED.load(Ordering::Relaxed)
}
struct Exposure {
    revealed: bool,
    forced_hidden: bool,
    deadline: Option<Instant>,
    slide: Spring,
    target: bool,
}
impl Service for Exposure {
    fn new() -> Self {
        Self {
            revealed: false,
            forced_hidden: false,
            deadline: None,
            slide: Spring::slow(0.0, 2.2),
            target: false,
        }
    }
    fn interval() -> Duration {
        Duration::from_millis(30)
    }
    fn update(&mut self) -> bool {
        let occupied = occupied();
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            self.revealed = false;
            self.deadline = None;
        }
        let target = visibility::should_tuck(
            self.forced_hidden,
            occupied,
            self.revealed,
            can_hide(),
            visibility::auto_hide(),
        );
        let changed = target != self.target;
        self.target = target;
        self.slide.to(if target { 1.0 } else { 0.0 });
        // Sampling requests frames until the slide has settled.
        let _ = self.slide.precise_value();
        changed
    }
}
pub(super) fn occupied() -> bool {
    amane::Workspaces::read()
        .list()
        .iter()
        .any(|w| w.focused() && w.windows() > 0)
}
pub(super) fn tucked() -> bool {
    let s = Exposure::read();
    visibility::should_tuck(
        s.forced_hidden,
        occupied(),
        s.revealed,
        can_hide(),
        visibility::auto_hide(),
    )
}
pub(super) fn amount() -> f32 {
    Exposure::read().slide.precise_value().clamp(0.0, 1.0)
}
pub(super) fn reveal() {
    let mut s = Exposure::write();
    s.forced_hidden = false;
    s.revealed = true;
    s.deadline = Some(Instant::now() + Duration::from_millis(3500));
}
pub(super) fn pointer(inside: bool) {
    let mut s = Exposure::write();
    if inside {
        s.forced_hidden = false;
        s.revealed = true;
        s.deadline = None;
    } else {
        s.deadline = Some(Instant::now() + Duration::from_millis(650));
    }
}

pub(super) fn hide() {
    let mut s = Exposure::write();
    s.forced_hidden = true;
    s.revealed = false;
    s.deadline = None;
}
