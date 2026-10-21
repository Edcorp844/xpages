use relm4::{adw, adw::prelude::*, prelude::*};
mod components;

use components::EguiArea;
use std::cell::RefCell;
use std::rc::Rc;

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
    session: Rc<RefCell<wordcraft_engine::Session>>,
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

        // Create the session with an empty document.
        let mut session = wordcraft_engine::Session::new(wordcraft_doc::Document::new());

        // Try to insert some text so we have something to render.
        // The exact command name/JSON shape may differ — check crates/engine/src/commands.rs.
        match session.run(
            "text.insert",
            &serde_json::json!({ "text" : "Hello, world! This is the first line." }),
        ) {
            Ok(result) => eprintln!("[init] text.insert succeeded: {:?}", result),
            Err(e) => eprintln!("[init] text.insert failed: {:?}", e),
        }

        let session = Rc::new(RefCell::new(session));
        let session_for_ui = session.clone();

        let egui_area = EguiArea::default();
        egui_area.set_vexpand(true);
        egui_area.set_hexpand(true);

        egui_area.set_ui(move |ctx| {
            use std::sync::Once;
            static INIT: Once = Once::new();
            INIT.call_once(|| setup_cjk_fonts(ctx));

            egui::CentralPanel::default().show(ctx, |ui| {
                let mut session = session_for_ui.borrow_mut();
                let layout = session.layout();

                // --- DIAGNOSTICS ---
                eprintln!(
                    "[ui] pages: {}, placed per page: {:?}",
                    layout.pages.len(),
                    layout
                        .pages
                        .iter()
                        .map(|p| p.items.len())
                        .collect::<Vec<_>>()
                );
                for (i, page) in layout.pages.iter().enumerate() {
                    eprintln!("[ui] page {} size: {} x {}", i, page.w, page.h);
                }
                // -------------------

                let zoom: f32 = 1.0;
                let page_gap: f32 = 20.0;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut y_offset = 0.0f32;

                    for page in &layout.pages {
                        let page_w = page.w as f32 * zoom;
                        let page_h = page.h as f32 * zoom;

                        let page_rect = egui::Rect::from_min_size(
                            egui::pos2(ui.cursor().min.x + 50.0, y_offset),
                            egui::vec2(page_w, page_h),
                        );

                        // Page background
                        ui.painter()
                            .rect_filled(page_rect, 0.0, egui::Color32::WHITE);

                        // Draw each placed element.
                        // TODO: replace this placeholder with the real `Placed` variants
                        // from crates/layout/src/lib.rs.
                        for placed in &page.items {
                            // Example (adjust to your actual enum):
                            //
                            // match placed {
                            //     wordcraft_layout::Placed::TextRun { rect, text, size, .. } => {
                            //         let pos = egui::pos2(
                            //             page_rect.min.x + rect.x as f32 * zoom,
                            //             page_rect.min.y + rect.y as f32 * zoom,
                            //         );
                            //         ui.painter().text(
                            //             pos,
                            //             egui::Align2::LEFT_TOP,
                            //             text,
                            //             egui::FontId::proportional(*size as f32 * zoom),
                            //             egui::Color32::BLACK,
                            //         );
                            //     }
                            //     wordcraft_layout::Placed::Rect { rect, .. } => {
                            //         let r = egui::Rect::from_min_size(
                            //             egui::pos2(
                            //                 page_rect.min.x + rect.x as f32 * zoom,
                            //                 page_rect.min.y + rect.y as f32 * zoom,
                            //             ),
                            //             egui::vec2(
                            //                 rect.width as f32 * zoom,
                            //                 rect.height as f32 * zoom,
                            //             ),
                            //         );
                            //         ui.painter().rect_filled(r, 0.0, egui::Color32::LIGHT_GRAY);
                            //     }
                            //     _ => {}
                            // }
                            let _ = placed;
                        }

                        y_offset += page_h + page_gap;
                    }

                    ui.allocate_space(egui::vec2(ui.available_width(), y_offset));
                });
            });
        });

        container.append(&egui_area);

        let model = AppModel {
            _egui_area: egui_area,
            session,
        };
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {}
}

fn main() {
    let app = RelmApp::new("com.example.XPagesTest");
    app.run::<AppModel>(());
}
