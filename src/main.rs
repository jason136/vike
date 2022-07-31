mod app;

fn main() {
    let app = app::FkApp::new();
    app.main_loop();
}
