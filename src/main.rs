use relm4::{adw, adw::prelude::*, prelude::*};
mod components;

use components::EguiArea;

fn setup_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let path = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc";
    let name = "Noto Sans CJK";

    if let Ok(data) = std::fs::read(path) {
        fonts.font_data.insert(
            name.to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(data)),
        );

        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push(name.to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push(name.to_owned());
    } else {
        eprintln!("Failed to load CJK font from {}", path);
    }

    ctx.set_fonts(fonts);
}

struct AppModel {
    _egui_area: EguiArea,
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = ();
    type Output = ();

    view! {
        #[name = "adw_application_window"]
        adw::ApplicationWindow {
            set_title: Some("XPages Test"),
            set_default_size: (800, 600),

            #[wrap(Some)]
            #[name = "content_box"]
            set_content = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_vexpand: true,
                set_hexpand: true,
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        let container = &widgets.content_box;

        let egui_area = EguiArea::default();
        egui_area.set_vexpand(true);
        egui_area.set_hexpand(true);

        egui_area.set_ui(|ctx| {
            use std::sync::Once;
            static INIT: Once = Once::new();
            INIT.call_once(|| setup_cjk_fonts(ctx));

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("EguiArea is working!");
                ui.separator();
                ui.label("Type some Chinese text below to test CJK rendering:");
                ui.add_space(10.0);

                let mut text = String::new();
                ui.text_edit_singleline(&mut text);

                ui.add_space(20.0);
                ui.label("示例中文：你好，世界！这是一个测试。");
                ui.label("日本語のテスト：こんにちは世界");
                ui.label("한국어 테스트: 안녕하세요 세계");

                ui.add_space(20.0);
                let mut counter = 0;
                ui.add(egui::Slider::new(&mut counter, 0..=10).text("Test slider"));
                if ui.button("Click me").clicked() {
                    println!("Button was clicked!");
                }
            });
        });

        container.append(&egui_area);

        let model = AppModel {
            _egui_area: egui_area,
        };
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {}
}

fn main() {
    let app = RelmApp::new("com.example.XPagesTest");
    app.run::<AppModel>(());
}
