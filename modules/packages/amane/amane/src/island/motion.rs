//! Analytic, underdamped springs: frame-rate independent, with continuous
//! position and velocity when a transition is interrupted.
use std::time::{Duration, Instant};

// About 4% overshoot, then a gentle return. Shared by every island mode.
const FREQUENCY: f32 = 21.0;
const DAMPING: f32 = 0.72;
const SETTLE: Duration = Duration::from_millis(620);

pub(crate) struct Spring {
    from: f32,
    velocity: f32,
    target: f32,
    started: Instant,
    time_scale: f32,
}

impl Spring {
    pub(crate) fn new(value: f32) -> Self {
        Self {
            from: value,
            velocity: 0.0,
            target: value,
            started: Instant::now(),
            time_scale: 1.0,
        }
    }

    pub(crate) fn slow(value: f32, time_scale: f32) -> Self {
        Self {
            time_scale,
            ..Self::new(value)
        }
    }

    pub(crate) fn to(&mut self, target: f32) {
        if self.target == target {
            return;
        }
        let now = Instant::now();
        let (position, velocity) = self.sample(now.duration_since(self.started));
        self.from = position;
        self.velocity = velocity;
        self.target = target;
        self.started = now;
    }

    pub(crate) fn value(&self) -> f32 {
        // Only layer geometry is pixel-snapped. Tiny pill width/height and
        // tint animations need an unclamped, subpixel spring sample.
        (self.precise_value().max(2.0) / 2.0).round() * 2.0
    }

    pub(crate) fn precise_value(&self) -> f32 {
        let elapsed = self.started.elapsed();
        if elapsed < SETTLE.mul_f32(self.time_scale)
            && (self.from != self.target || self.velocity != 0.0)
        {
            amane::request_frame();
        }
        self.sample(elapsed).0
    }

    fn sample(&self, elapsed: Duration) -> (f32, f32) {
        let elapsed = elapsed.div_f32(self.time_scale);
        if elapsed.is_zero() {
            return (self.from, self.velocity);
        }
        if elapsed >= SETTLE {
            return (self.target, 0.0);
        }
        let t = elapsed.as_secs_f32();
        let decay = DAMPING * FREQUENCY;
        let frequency = FREQUENCY * (1.0 - DAMPING * DAMPING).sqrt();
        let displacement = self.from - self.target;
        let sine_weight = (self.velocity + decay * displacement) / frequency;
        let (sin, cos) = (frequency * t).sin_cos();
        let wave = displacement * cos + sine_weight * sin;
        let envelope = (-decay * t).exp();
        let position = self.target + envelope * wave;
        let velocity =
            envelope * (frequency * (-displacement * sin + sine_weight * cos) - decay * wave);
        (position, velocity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transition(from: f32, target: f32) -> Spring {
        Spring {
            target,
            ..Spring::new(from)
        }
    }

    #[test]
    fn slow_hide_spring_preserves_shape_but_extends_duration() {
        let normal = transition(0.0, 1.0);
        let slow = Spring {
            time_scale: 2.2,
            ..transition(0.0, 1.0)
        };
        let normal_pos = normal.sample(Duration::from_millis(200)).0;
        let slow_pos = slow.sample(Duration::from_millis(440)).0;
        assert!((normal_pos - slow_pos).abs() < 0.0001);
        assert_ne!(slow.sample(Duration::from_millis(620)).0, 1.0);
        assert_eq!(slow.sample(Duration::from_millis(1365)).0, 1.0);
    }
    #[test]
    fn normalized_spring_is_not_clamped_or_pixel_snapped() {
        assert_eq!(Spring::new(0.0).precise_value(), 0.0);
        assert_eq!(Spring::new(1.0).precise_value(), 1.0);
        assert_eq!(Spring::new(0.25).precise_value(), 0.25);
    }

    #[test]
    fn no_residual_motion_after_short_settle() {
        let spring = transition(104.0, 672.0);
        for ms in [620, 700, 800, 1600] {
            assert_eq!(spring.sample(Duration::from_millis(ms)), (672.0, 0.0));
        }
    }

    #[test]
    fn starts_at_rest_and_finishes_exactly() {
        let spring = transition(96.0, 400.0);
        assert_eq!(spring.sample(Duration::ZERO), (96.0, 0.0));
        assert_eq!(spring.sample(SETTLE), (400.0, 0.0));
    }

    #[test]
    fn expansion_and_shrink_both_overshoot() {
        let expansion = transition(96.0, 400.0);
        let shrink = transition(400.0, 96.0);
        let bounce = Duration::from_millis(225);
        let expanded = expansion.sample(bounce).0;
        let shrunk = shrink.sample(bounce).0;
        assert!(expanded > 400.0 && expanded < 416.0);
        assert!(shrunk < 96.0 && shrunk > 80.0);
    }

    #[test]
    fn retarget_preserves_position_and_velocity() {
        let mut spring = transition(96.0, 400.0);
        spring.started = Instant::now() - Duration::from_millis(80);
        spring.to(96.0);
        let initial = spring.sample(Duration::ZERO);
        assert!((initial.0 - spring.from).abs() < 0.001);
        assert!((initial.1 - spring.velocity).abs() < 0.001);
        assert!(initial.0 > 96.0 && initial.0 < 400.0);
        assert!(initial.1 > 0.0);
        assert_eq!(spring.sample(SETTLE), (96.0, 0.0));
    }
}
