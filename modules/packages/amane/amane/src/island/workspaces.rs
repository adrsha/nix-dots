//! Mango's workspace service supplies the focused output's tags. Local springs
//! preserve position and velocity on rapid switches; no compositor polling.
use super::*;
use amane::Workspaces;
struct Pill {
    id: i64,
    active: bool,
    urgent: bool,
    occupied: bool,
    motion: Spring,
}
struct Pills {
    list: Vec<Pill>,
}
impl Service for Pills {
    fn new() -> Self {
        Self { list: Vec::new() }
    }
    fn interval() -> Duration {
        Duration::from_millis(30)
    }
    fn update(&mut self) -> bool {
        let workspaces = Workspaces::read();
        let list = workspaces.list();
        let output = list
            .iter()
            .find(|w| w.focused())
            .or_else(|| list.iter().find(|w| w.active()))
            .and_then(|w| w.output());
        let tags: Vec<_> = list.iter().filter(|w| w.output() == output).collect();
        let previous_count = self.list.len();
        self.list
            .retain(|pill| tags.iter().any(|w| w.id() == pill.id));
        let mut changed = previous_count != self.list.len();
        for w in tags {
            if let Some(pill) = self.list.iter_mut().find(|p| p.id == w.id()) {
                if pill.active != w.active()
                    || pill.urgent != w.urgent()
                    || pill.occupied != (w.windows() > 0)
                {
                    pill.active = w.active();
                    pill.urgent = w.urgent();
                    pill.occupied = w.windows() > 0;
                    pill.motion.to(if w.active() { 1.0 } else { 0.0 });
                    changed = true;
                }
            } else {
                self.list.push(Pill {
                    id: w.id(),
                    active: w.active(),
                    urgent: w.urgent(),
                    occupied: w.windows() > 0,
                    motion: Spring::new(if w.active() { 1.0 } else { 0.0 }),
                });
                changed = true;
            }
        }
        changed
    }
}
pub(super) fn view() -> Row {
    let state = Pills::read();
    let count = state.list.len().max(1) as f32;
    let gap = 7.0;
    let slim = ((MEDIA_WIDTH - gap * (count - 1.0) - 10.0) / count).clamp(4.0, 12.0);
    Row::new(
        state
            .list
            .iter()
            .map(|pill| {
                let t = pill.motion.precise_value();
                let color = if pill.urgent {
                    crate::theme::palette().yellow
                } else {
                    crate::theme::palette().blue
                };
                let tint = blend_color(
                    blend_color(color, Color::BLACK, 0.55),
                    color,
                    if pill.occupied { 1.0 } else { t },
                );
                let id = pill.id;
                let key = format!("workspace:{id}");
                let ck = key.clone();
                Box::new(
                    Rectangle::new()
                        .width(slim + 10.0 * t)
                        .height(23.0)
                        .scale(crate::feedback::scale(&key))
                        .radius(Full)
                        .fill(icon_gradient(tint))
                        .cursor(Pointer)
                        .on_click(move |button| {
                            if matches!(button, Button::Left) {
                                crate::feedback::click(&ck);
                                Workspaces::focus(id);
                            }
                        }),
                ) as Box<dyn amane::Widget>
            })
            .collect(),
    )
    .gap(gap)
    .align(Center)
}

pub(super) fn bounds(id: i64) -> (f32, f32) {
    let state = Pills::read();
    let count = state.list.len().max(1) as f32;
    let slim = ((MEDIA_WIDTH - 7.0 * (count - 1.0) - 10.0) / count).clamp(4.0, 12.0);
    let total: f32 = state
        .list
        .iter()
        .map(|p| slim + 10.0 * p.motion.precise_value())
        .sum::<f32>()
        + 7.0 * (count - 1.0);
    let start = modules::position(modules::Module::Workspaces, true).unwrap_or(ROW_PADDING);
    let mut x = start + (MEDIA_WIDTH - total) / 2.0;
    for p in &state.list {
        let w = slim + 10.0 * p.motion.precise_value();
        if p.id == id {
            return (x, w);
        }
        x += w + 7.0;
    }
    (start + MEDIA_WIDTH / 2.0, 12.0)
}

fn workspace_clients(id: i64) -> Vec<clients::Client> {
    let workspace = {
        let state = amane::Workspaces::read();
        state
            .list()
            .iter()
            .find(|w| w.id() == id)
            .map(|w| (w.output().unwrap_or("").to_string(), w.index()))
    };
    workspace.map_or_else(Vec::new, |(output, tag)| {
        clients::on_workspace(&output, tag)
    })
}
pub(super) fn grid_height(id: i64) -> f32 {
    40.0 + 28.0 + 12.0 + (workspace_clients(id).len().div_ceil(3).clamp(1, 3) as f32) * 96.0
}
pub(super) fn grid(id: i64) -> Column {
    let windows = workspace_clients(id);
    let apps = amane::Apps::read();
    let mut rows: Vec<Box<dyn amane::Widget>> = Vec::new();
    for chunk in windows.chunks(3) {
        let mut tiles: Vec<Box<dyn amane::Widget>> = vec![];
        for window in chunk {
            let app = apps.list().iter().find(|app| {
                app.icon()
                    .is_some_and(|s| s.eq_ignore_ascii_case(&window.app))
                    || app.name().eq_ignore_ascii_case(&window.app)
            });
            let image = Rectangle::new()
                .width(36.0)
                .height(36.0)
                .radius(8.0)
                .align_child(Center, Center);
            let image = if let Some(path) = app.and_then(|a| a.icon_path()) {
                image.fill(amane::Image::contain(path))
            } else {
                image
                    .fill(icon_gradient(crate::theme::palette().cyan))
                    .child(
                        Text::new(
                            window
                                .app
                                .chars()
                                .next()
                                .unwrap_or('?')
                                .to_uppercase()
                                .to_string(),
                        )
                        .size(22.0)
                        .weight(Weight::Bold)
                        .color(Color::BLACK),
                    )
            };

            let client = window.id;
            let key = format!("client:{client}");
            let ck = key.clone();
            tiles.push(Box::new(
                Rectangle::new()
                    .width(88.0)
                    .height(88.0)
                    .scale(crate::feedback::scale(&key))
                    .radius(14.0)
                    .fill(crate::theme::palette().surface)
                    .align_child(Center, Center)
                    .cursor(Pointer)
                    .on_click(move |b| {
                        if matches!(b, Button::Left) {
                            crate::feedback::click(&ck);
                            clients::focus(client);
                        }
                    })
                    .child(
                        Column::new(children![image,])
                            .width(76.0)
                            .gap(3.0)
                            .align(Center),
                    ),
            ));
        }
        rows.push(Box::new(Row::new(tiles).gap(8.0).align(Center)));
    }
    if windows.is_empty() {
        rows.push(Box::new(
            Text::new("No open windows")
                .size(14.0)
                .color(crate::theme::palette().secondary),
        ));
    }
    let index = id & 0xffffffff;
    let grid = Column::new(rows).gap(8.0);
    Column::new(children![
        Text::new(format!("Workspace {index} · {} windows", windows.len()))
            .size(18.0)
            .weight(Weight::SemiBold)
            .color(crate::theme::palette().foreground),
        amane::ScrollArea::new("workspace-app-grid", grid)
            .width(280.0)
            .height(grid_height(id) - 80.0),
    ])
    .width(288.0)
    .gap(12.0)
}
