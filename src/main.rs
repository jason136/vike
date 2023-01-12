mod app;
mod renderer;
mod render_systems;
mod game_object;
mod camera;
mod movement;

fn main() {
    let app = app::VkApp::new();
    app.main_loop();
}
