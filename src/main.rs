mod app;
mod game_object;

fn main() {
    let app = app::FkApp::new();
    app.main_loop();
}
