mod app;
mod app_menu;
mod components;
mod editor;
mod state;
mod utils;

use app_menu::AppMenu;
use relm4::RelmApp;

fn main() {
    adw::init().expect("adw::init failed");
    AppMenu::register();
    let app = RelmApp::new("com.example.XPages");
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);
    app.run::<app::XPagesApp>(());
}
