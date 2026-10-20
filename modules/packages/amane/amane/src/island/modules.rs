//! The two lists below are the layout configuration. Reorder/remove entries to
//! choose mode contents; anchors and shared-element motion follow automatically.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Module {
    Controls,
    Workspaces,
    Time,
    Date,
    Media,
    Visualizer,
    Settings,
}
use Module::*;
pub(super) const COLLAPSED: &[Module] = &[Time, Visualizer];
pub(super) const EXPANDED: &[Module] = &[
    Controls, Workspaces, Time, Date, Media, Visualizer, Settings,
];
impl Module {
    pub(super) const fn width(self) -> f32 {
        match self {
            Controls => CONTROL_WIDTH * 2.0 + CONTROL_GAP,
            Workspaces => MEDIA_WIDTH,
            Time => 44.0,
            Date => 52.0,
            Media => TRANSPORT_WIDTH,
            Visualizer => visualizer::WIDTH,
            Settings => 32.0,
        }
    }
}
const fn gap(previous: Module, next: Module, expanded: bool) -> f32 {
    if matches!(previous, Time) && matches!(next, Date) {
        0.0
    } else if expanded {
        ROW_GAP
    } else {
        12.0
    }
}
pub(super) const fn total(list: &[Module], expanded: bool) -> f32 {
    let mut width = ROW_PADDING * 2.0;
    let mut i = 0;
    while i < list.len() {
        if i > 0 {
            width += gap(list[i - 1], list[i], expanded);
        }
        width += list[i].width();
        i += 1;
    }
    width
}
pub(super) fn position(module: Module, expanded: bool) -> Option<f32> {
    let list = if expanded { EXPANDED } else { COLLAPSED };
    let mut x = ROW_PADDING;
    for (i, m) in list.iter().enumerate() {
        if i > 0 {
            let g = gap(list[i - 1], *m, expanded);
            x += if *m == Visualizer {
                g * visualizer::amount()
            } else {
                g
            };
        }
        if *m == module {
            return Some(x);
        }
        x += if *m == Visualizer {
            m.width() * visualizer::amount()
        } else {
            m.width()
        };
    }
    None
}
pub(super) fn unused_visualizer_width(expanded: bool) -> f32 {
    (visualizer::WIDTH + if expanded { ROW_GAP } else { 12.0 }) * (1.0 - visualizer::amount())
}
fn widget(module: Module, clock: &Clock) -> Rectangle {
    match module {
        Settings => row_cell(
            32.0,
            Center,
            Text::new("\u{e8b8}")
                .font("Material Icons Round")
                .size(25.0)
                .tight()
                .color(crate::theme::palette().blue),
        )
        .cursor(Pointer)
        .scale(crate::feedback::scale("settings-gear"))
        .on_click(|b| {
            if b == Button::Left {
                crate::feedback::click("settings-gear");
                settings::toggle()
            }
        }),
        Controls => {
            let audio = Audio::read();
            row_cell(
                module.width(),
                Center,
                Row::new(children![
                    control_icon(Osd::Volume, audio.muted(), false),
                    control_icon(Osd::Brightness, audio.muted(), false)
                ])
                .gap(CONTROL_GAP)
                .align(Center),
            )
        }
        Workspaces => row_cell(module.width(), Center, workspaces::view()),
        Time => row_cell(
            module.width(),
            Center,
            Text::new(&clock.time)
                .size(14.0)
                .weight(Weight::SemiBold)
                .color(crate::theme::palette().foreground),
        )
        .on_click(clicked)
        .on_hover(|inside| {
            let expanded = { Island::read().expanded };
            if expanded {
                popups::hover(popups::Popup::Calendar, inside);
            }
        }),
        Date => {
            let mut parts = clock.date.split_whitespace();
            let weekday: String = parts.next().unwrap_or("").chars().take(3).collect();
            let day = parts.next().unwrap_or("").trim_end_matches(',');
            row_cell(
                module.width(),
                Center,
                Text::new(format!("· {weekday} {day}"))
                    .size(14.0)
                    .weight(Weight::SemiBold)
                    .color(crate::theme::palette().foreground),
            )
            .on_hover(|inside| {
                let expanded = { Island::read().expanded };
                if expanded {
                    popups::hover(popups::Popup::Calendar, inside);
                }
            })
        }
        Media => row_transport(&amane::Media::read()),
        Visualizer => row_cell(module.width(), Center, visualizer::view()),
    }
}
pub(super) fn view(island: &Island, clock: &Clock, width: f32, height: f32) -> amane::Stack {
    let p = island.expansion.precise_value();
    let mut list = EXPANDED.to_vec();
    for m in COLLAPSED {
        if !list.contains(m) {
            list.push(*m);
        }
    }
    let mut widgets: Vec<Box<dyn amane::Widget>> = vec![];
    for (i, module) in list.into_iter().enumerate() {
        let from = position(module, false);
        let to = position(module, true);
        let source = from.unwrap_or_else(|| position(Time, false).unwrap_or(ROW_PADDING));
        let target = to.unwrap_or_else(|| position(Time, true).unwrap_or(ROW_PADDING));
        let center_from =
            source + module.width() / 2.0 - (IDLE.0 - unused_visualizer_width(false)) / 2.0;
        let center_to =
            target + module.width() / 2.0 - (PANEL.0 - unused_visualizer_width(true)) / 2.0;
        let x = width / 2.0 + center_from + (center_to - center_from) * p - module.width() / 2.0;
        let fade = match (from, to) {
            (Some(_), Some(_)) => 1.0,
            (None, Some(_)) => ((p - i as f32 * 0.025) / 0.8).clamp(0.0, 1.0),
            (Some(_), None) => (1.0 - p).clamp(0.0, 1.0),
            _ => 0.0,
        };
        // Always retain module slots/descendant target indices across frames.
        // A zero transform removes invisible modules from hit testing without
        // changing target identity in Amane's index-based pointer dispatcher.
        let visible = fade > 0.01 && (module != Visualizer || visualizer::amount() > 0.01);
        widgets.push(Box::new(
            widget(module, clock)
                .translate(
                    x,
                    (height - 36.0) / 2.0
                        + if from.is_none() {
                            10.0 * (1.0 - fade)
                        } else {
                            0.0
                        },
                )
                .opacity(fade)
                .scale(if visible { 1.0 } else { 0.0 }),
        ));
    }
    amane::Stack::new(widgets).width(width).height(height)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn module_registry_budgets_both_modes_and_has_no_spacers() {
        assert_eq!(total(COLLAPSED, false), IDLE.0);
        assert_eq!(total(EXPANDED, true), PANEL.0);
        assert_eq!(
            position(Time, true).unwrap() + Time.width(),
            position(Date, true).unwrap()
        );
        assert_eq!(
            position(Settings, true).unwrap() + Settings.width() + ROW_PADDING,
            PANEL.0 - unused_visualizer_width(true)
        );
    }
}
