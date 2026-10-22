mod app;
mod state;
mod utils;
mod components;
mod editor;

use relm4::RelmApp;

fn main() {
    adw::init().expect("adw::init failed");

    let app = RelmApp::new("com.example.XPages");
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);
    app.run::<app::XPagesApp>(());
}