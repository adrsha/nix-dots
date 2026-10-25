mod auxiliary;
mod ddc;
mod edge_activation;
mod feedback;
mod island;
mod picker;
mod theme;

use amane::App;

fn main() {
    ddc::start();

    App::new()
        .font("Inter")
        .ipc("brightness", ddc::brightness)
        .ipc("volume", island::volume)
        .ipc("media", island::media)
        .ipc("island", island::toggle)
        .ipc("popup", island::popups::command)
        .ipc("settings", island::settings::command)
        .ipc("auxiliary", auxiliary::command)
        .ipc("picker", picker::command)
        .ipc("edge", edge_activation::command)
        .window(island::view)
        .window(auxiliary::view)
        .window(picker::view)
        .run();
}
