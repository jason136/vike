mod app;
mod renderer;
mod simple_render_system;
mod game_object;

fn main() {
    let app = app::FkApp::new();
    app.main_loop();
}
