//! Stable full-output island surface. Only explicit island/popup areas take input.
use super::*;
pub(super) const THICKNESS: f32 = 4.0;
pub(super) fn frame(width: f32, height: f32) -> amane::Stack {
    let path = std::path::Path::new(file!())
        .parent()
        .unwrap()
        .join("frame.wgsl");
    // While a region/window pick is pending the frame takes the capture colour
    // (and thickens a little) so it is obvious the screen is waiting for input.
    let mut values = crate::theme::shader_values(vec![[THICKNESS, settings::radius(), 0.0, 0.0]]);
    if let Some((color, amount)) = crate::auxiliary::tint() {
        values[0][0] = THICKNESS + 3.0 * amount;
        let mix = |from: f32, to: u8| from + (f32::from(to) / 255.0 - from) * amount;
        values[1][0] = mix(values[1][0], color.red());
        values[1][1] = mix(values[1][1], color.green());
        values[1][2] = mix(values[1][2], color.blue());
    }
    amane::Stack::new(children![
        Rectangle::new()
            .width(width)
            .height(height)
            .shader(path)
            .shader_values(values)
    ])
    .width(width)
    .height(height)
}
/// Same alpha-field shader as popups; evaluated only over a narrow top strip.
pub(super) fn merge(
    canvas_width: f32,
    pill_width: f32,
    pill_height: f32,
    pill_y: f32,
    hide: f32,
) -> Rectangle {
    let strip_width = (pill_width + 80.0).min(canvas_width);
    let strip_height = pill_height + GAP_TOP + 40.0;
    let path = std::path::Path::new(file!())
        .parent()
        .unwrap()
        .join("goo.wgsl");
    Rectangle::new()
        .width(strip_width)
        .height(strip_height)
        .translate((canvas_width - strip_width) / 2.0, 0.0)
        .shader(path)
        .shader_values({
            let mut values = crate::theme::shader_values(vec![
                [-40.0, -40.0, strip_width + 80.0, 40.0 + THICKNESS],
                [
                    (strip_width - pill_width) / 2.0,
                    pill_y,
                    pill_width,
                    pill_height,
                ],
                [
                    0.0,
                    // Must match `pill()`: a taller panel (settings) keeps a 34px corner,
                    // not height/2, or this field balloons into a huge capsule.
                    (pill_height / 2.0).min(34.0),
                    9.0 * (hide * std::f32::consts::PI).sin().max(0.0),
                    0.48,
                ],
            ]);
            // While the screen border is tinted for a capture pick, the goo
            // strip blends toward the same colour so no black notch remains.
            if let Some((color, amount)) = crate::auxiliary::tint() {
                let mix = |from: f32, to: u8| from + (f32::from(to) / 255.0 - from) * amount;
                let n = values.len() - 1;
                values[n][0] = mix(values[n][0], color.red());
                values[n][1] = mix(values[n][1], color.green());
                values[n][2] = mix(values[n][2], color.blue());
            }
            values
        })
}
