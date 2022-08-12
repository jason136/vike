mod app;
mod renderer;
mod simple_render_system;
mod game_object;
mod camera;
mod movement;

fn main() {
    let app = app::VkApp::new();
    app.main_loop();
}
