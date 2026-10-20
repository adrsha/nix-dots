//! Settings-only scroll physics, composed from ordinary clipped rectangles.
use super::*;
use std::collections::BTreeMap;
const STEP: f32 = 56.0;
const RUBBER: f32 = 54.0;
struct Position {
    target: f32,
    maximum: f32,
    motion: Spring,
    release: Option<Instant>,
}
struct Scrolling {
    pages: BTreeMap<&'static str, Position>,
}
impl Service for Scrolling {
    fn new() -> Self {
        Self {
            pages: BTreeMap::new(),
        }
    }
    fn interval() -> Duration {
        Duration::from_millis(16)
    }
    fn update(&mut self) -> bool {
        let mut changed = false;
        for p in self.pages.values_mut() {
            if p.release.is_some_and(|t| Instant::now() >= t) {
                p.release = None;
                p.target = p.target.clamp(0.0, p.maximum);
                p.motion.to(p.target);
                changed = true;
            }
        }
        changed
    }
}
fn resisted(value: f32, max: f32) -> f32 {
    if value < 0.0 {
        -RUBBER * (1.0 - (-value / RUBBER).exp())
    } else if value > max {
        max + RUBBER * (1.0 - (-(value - max) / RUBBER).exp())
    } else {
        value
    }
}
fn wheel(id: &'static str, delta: f32, max: f32) {
    if delta == 0.0 || !delta.is_finite() {
        return;
    }
    let mut s = Scrolling::write();
    let p = s.pages.entry(id).or_insert_with(|| Position {
        target: 0.0,
        maximum: max,
        motion: Spring::slow(0.0, 0.7),
        release: None,
    });
    if p.maximum != max {
        p.target = p.target.clamp(0.0, max);
    }
    p.maximum = max;
    p.target = resisted(p.target + delta * STEP, max);
    p.motion.to(p.target);
    p.release = Some(Instant::now() + Duration::from_millis(110));
}
fn track_value(y: f32, max: f32, height: f32, thumb: f32) -> f32 {
    ((y - thumb / 2.0) / (height - thumb).max(1.0)).clamp(0.0, 1.0) * max
}
fn seek(id: &'static str, y: f32, max: f32, height: f32, thumb: f32) {
    let value = track_value(y, max, height, thumb);
    let mut s = Scrolling::write();
    let p = s.pages.entry(id).or_insert_with(|| Position {
        target: 0.0,
        maximum: max,
        motion: Spring::new(0.0),
        release: None,
    });
    p.maximum = max;
    p.target = value;
    p.motion = Spring::new(value);
    p.release = None;
}
/// Current scroll offset of a page, for callers that only build what is visible.
pub(super) fn offset(id: &'static str) -> f32 {
    Scrolling::read()
        .pages
        .get(id)
        .map(|p| p.motion.precise_value().max(0.0))
        .unwrap_or(0.0)
}
pub(super) fn view(
    id: &'static str,
    content: impl amane::Widget + 'static,
    width: f32,
    height: f32,
) -> Rectangle {
    let content_h = match content.height() {
        amane::Size::Fixed(h) => h,
        _ => height,
    };
    let max = (content_h - height).max(0.0);
    let state = Scrolling::read();
    let offset = state
        .pages
        .get(id)
        .map(|p| {
            let sampled = p.motion.precise_value();
            if p.maximum != max {
                sampled.clamp(0.0, max)
            } else {
                sampled
            }
        })
        .unwrap_or(0.0);
    drop(state);
    let thumb = if max > 0.0 {
        (height * height / content_h).max(32.0).min(height)
    } else {
        height
    };
    let ty = if max > 0.0 {
        offset.clamp(0.0, max) / max * (height - thumb)
    } else {
        0.0
    };
    let overshoot = if offset < 0.0 {
        -offset
    } else {
        (offset - max).max(0.0)
    };
    let visible_thumb = (thumb - overshoot * 0.4).max(18.0).min(height);
    let ty = if offset > max {
        height - visible_thumb
    } else {
        ty
    };
    let mut children: Vec<Box<dyn amane::Widget>> = children![
        Rectangle::new()
            .width(width - 18.0)
            .height(content_h)
            .translate(0.0, -offset)
            .child(content)
    ];
    // Full-height hit track makes click-to-position and drag continuous, even
    // when the thumb itself moves between input frames.
    children.push(Box::new(
        Rectangle::new()
            .width(14.0)
            .height(height)
            .translate(width - 14.0, 0.0)
            .cursor(if max > 0.0 {
                amane::Grab
            } else {
                amane::Default
            })
            .on_drag(move |point| {
                if max > 0.0 {
                    seek(id, point.y, max, height, thumb)
                }
            })
            .child(
                // Plain solid fills, and nothing at all when there is nothing to
                // scroll: a gradient forces a window-sized vector pass per frame,
                // and opacity 0 forces a window-sized offscreen canvas.
                amane::Stack::new(if max > 0.0 {
                    children![
                        Rectangle::new()
                            .width(2.0)
                            .height(height)
                            .translate(6.0, 0.0)
                            .radius(Full)
                            .fill(blend_color(
                                crate::theme::palette().blue,
                                Color::BLACK,
                                0.82
                            )),
                        Rectangle::new()
                            .width(6.0)
                            .height(visible_thumb)
                            .translate(4.0, ty)
                            .radius(Full)
                            .fill(crate::theme::palette().cyan)
                    ]
                } else {
                    children![]
                })
                .width(14.0)
                .height(height),
            ),
    ));
    Rectangle::new()
        .width(width)
        .height(height)
        .clip()
        .on_scroll(move |scroll| wheel(id, scroll.y, max))
        .child(amane::Stack::new(children).width(width).height(height))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rubber_band_is_bounded_and_bidirectional() {
        assert!(resisted(-1000.0, 100.0) >= -RUBBER);
        assert!(resisted(1000.0, 100.0) <= 100.0 + RUBBER);
        assert_eq!(resisted(50.0, 100.0), 50.0);
        assert!(STEP > 15.0 * 3.0);
    }
    #[test]
    fn scrollbar_maps_track_to_content() {
        assert_eq!(track_value(-10.0, 1000.0, 400.0, 80.0), 0.0);
        assert_eq!(track_value(200.0, 1000.0, 400.0, 80.0), 500.0);
        assert_eq!(track_value(800.0, 1000.0, 400.0, 80.0), 1000.0);
    }
    #[test]
    fn overscroll_returns_to_endpoint() {
        let mut s = Scrolling::new();
        s.pages.insert(
            "test",
            Position {
                target: -40.0,
                maximum: 500.0,
                motion: Spring::new(-40.0),
                release: Some(Instant::now() - Duration::from_millis(1)),
            },
        );
        assert!(s.update());
        assert_eq!(s.pages["test"].target, 0.0);
        assert!(s.pages["test"].release.is_none());
    }
    #[test]
    fn upper_lower_bounds_are_continuous() {
        assert_eq!(resisted(0.0, 100.0), 0.0);
        assert_eq!(resisted(100.0, 100.0), 100.0);
    }
}
