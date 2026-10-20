//! Decorative playback animation. No output or microphone recording.
use super::*;
use std::sync::LazyLock;
static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);
pub(super) const WIDTH: f32 = 22.0;
struct Playback {
    motion: Spring,
    playing: bool,
}
impl Service for Playback {
    fn new() -> Self {
        Self {
            motion: Spring::new(0.0),
            playing: false,
        }
    }
    fn interval() -> Duration {
        Duration::from_millis(30)
    }
    fn update(&mut self) -> bool {
        let playing = Media::read().playing();
        if self.playing == playing {
            return false;
        }
        self.playing = playing;
        self.motion.to(if playing { 1.0 } else { 0.0 });
        true
    }
}
pub(super) fn amount() -> f32 {
    Playback::read().motion.precise_value().clamp(0.0, 1.0)
}
pub(super) fn view() -> Rectangle {
    let playback = Playback::read();
    let amount = playback.motion.precise_value().clamp(0.0, 1.0);
    let time = EPOCH.elapsed().as_secs_f32();
    if playback.playing {
        amane::request_frame();
    }
    let bars = (0..4)
        .map(|i| {
            let wave = (time * (2.6 + i as f32 * 0.5) + i as f32 * 1.7).sin();
            let h = (5.0 + 13.0 * (wave * 0.5 + 0.5)) * amount;
            let colors = [
                crate::theme::palette().green,
                crate::theme::palette().cyan,
                crate::theme::palette().blue,
                crate::theme::palette().yellow,
                crate::theme::palette().green,
            ];
            let phase = (time / 3.5 + i as f32 * 0.16) % 4.0;
            let index = phase.floor() as usize;
            let mix = phase.fract();
            let mix = mix * mix * (3.0 - 2.0 * mix);
            let color = blend_color(colors[index], colors[index + 1], mix);
            Box::new(
                Rectangle::new()
                    .width(4.0)
                    .height(h.max(0.1))
                    .radius(Full)
                    .fill(color),
            ) as Box<dyn amane::Widget>
        })
        .collect();
    Rectangle::new()
        .width(WIDTH)
        .height(24.0)
        .align_child(Center, Center)
        .opacity(amount)
        .translate(0.0, 3.0 * (1.0 - amount))
        .child(Row::new(bars).gap(2.0).align(Center))
}
