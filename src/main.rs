use relm4::{adw, adw::prelude::*, prelude::*};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Once;

mod components;
use components::EguiArea;

fn setup_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let path = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc";
    let name = "Noto Sans CJK";

    match std::fs::read(path) {
        Ok(data) => {
            fonts.font_data.insert(
                name.to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push(name.to_owned());
            }
            ctx.set_fonts(fonts);
        }
        Err(err) => eprintln!("Failed to load CJK font from {path}: {err}"),
    }
}

fn load_document(path: &std::path::Path) -> Result<wordcraft_engine::Session, String> {
    let data = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&data).into_owned();
    let mut session = wordcraft_engine::Session::new(wordcraft_doc::Document::new());
    session
        .run("text.insert", &serde_json::json!({ "text": text }))
        .map_err(|e| format!("text.insert: {e:?}"))?;
    Ok(session)
}

fn paint_layout(ui: &mut egui::Ui, session: &Rc<RefCell<wordcraft_engine::Session>>) {
    let mut session = session.borrow_mut();
    let layout = session.layout();

    const ZOOM: f32 = 1.0;
    const PAGE_GAP: f32 = 20.0;
    const MARGIN_X: f32 = 50.0;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Make the scroll content at least as wide as the view
            ui.set_min_width(ui.available_width());

            for page in &layout.pages {
                let page_w = page.w as f32 * ZOOM;
                let page_h = page.h as f32 * ZOOM;

                // Horizontal inset: full row width so layout is stable
                let row_w = (page_w + MARGIN_X * 2.0).max(ui.available_width());
                let row_h = page_h + PAGE_GAP;

                // ★ This is what makes scrolling work
                let (response, painter) =
                    ui.allocate_painter(egui::vec2(row_w, row_h), egui::Sense::hover());

                let page_rect = egui::Rect::from_min_size(
                    egui::pos2(response.rect.min.x + MARGIN_X, response.rect.min.y),
                    egui::vec2(page_w, page_h),
                );

                // Page background
                painter.rect_filled(page_rect, 0.0, egui::Color32::WHITE);
                painter.rect_stroke(
                    page_rect,
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::from_gray(160)),
                    egui::StrokeKind::Outside,
                );

                // Content
                for placed in &page.items {
                    paint_placed(&painter, page_rect, placed, ZOOM);
                }
            }
        });
}

/// Draw one layout item. Adjust arms to your real `Placed` enum.
fn paint_placed(
    painter: &egui::Painter,
    page_rect: egui::Rect,
    placed: &wordcraft_layout::Placed, // use your actual type
    zoom: f32,
) {
    // Inspect with: eprintln!("{placed:?}");
    match placed {
        // --- examples: rename fields to match crates/layout ---
        //
        // wordcraft_layout::Placed::TextRun { rect, text, size, .. } => {
        //     let pos = egui::pos2(
        //         page_rect.min.x + rect.x as f32 * zoom,
        //         page_rect.min.y + rect.y as f32 * zoom,
        //     );
        //     painter.text(
        //         pos,
        //         egui::Align2::LEFT_TOP,
        //         text,
        //         egui::FontId::proportional((*size as f32 * zoom).max(8.0)),
        //         egui::Color32::BLACK,
        //     );
        // }
        // wordcraft_layout::Placed::GlyphRun { rect, glyphs, .. } => { ... }
        // wordcraft_layout::Placed::Rect { rect, .. } => {
        //     let r = egui::Rect::from_min_size(
        //         egui::pos2(
        //             page_rect.min.x + rect.x as f32 * zoom,
        //             page_rect.min.y + rect.y as f32 * zoom,
        //         ),
        //         egui::vec2(rect.w as f32 * zoom, rect.h as f32 * zoom),
        //     );
        //     painter.rect_filled(r, 0.0, egui::Color32::LIGHT_BLUE);
        // }
        _ => {
            // Temporary: show *something* so you know items exist
            // If you see blue boxes, layout is fine and only the match is wrong.
        }
    }
}

#[derive(Debug)]
enum AppMsg {
    RequestOpen,
    OpenPath(PathBuf),
    OpenFailed(String),
}

struct AppModel {
    /// Filled in `init` *after* `view_output!()`, then kept for the component lifetime.
    egui_area: Option<EguiArea>,
    session: Rc<RefCell<wordcraft_engine::Session>>,
    has_document: Rc<RefCell<bool>>,
    open_path: Option<PathBuf>,
    subtitle: String,
    status: String,
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        #[name = "window"]
        adw::ApplicationWindow {
            set_title: Some("XPages"),
            set_default_size: (960, 700),

            #[wrap(Some)]
            set_content = &adw::OverlaySplitView {
                set_show_sidebar: false,

                #[wrap(Some)]
                set_sidebar = &adw::ToolbarView {
                    add_top_bar = &adw::HeaderBar {},
                },

                #[wrap(Some)]
                set_content = &adw::NavigationPage {
                    set_title: "XPages",

                    #[wrap(Some)]
                    set_child = &adw::ToolbarView {
                        add_top_bar = &adw::HeaderBar {
                            pack_start = &gtk::Button {
                                set_label: "Open",
                                set_tooltip_text: Some("Open a document"),
                                connect_clicked => AppMsg::RequestOpen,
                            },
                            #[wrap(Some)]
                            set_title_widget = &adw::WindowTitle {
                                set_title: "XPages",
                                #[watch]
                                set_subtitle: &model.subtitle,
                            },
                        },

                        #[wrap(Some)]
                        #[name = "content_box"]
                        set_content = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_vexpand: true,
                            set_hexpand: true,
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // ------------------------------------------------------------------
        // 1) Model first (so `view_output!` can see `model`)
        // ------------------------------------------------------------------
        let session = Rc::new(RefCell::new(wordcraft_engine::Session::new(
            wordcraft_doc::Document::new(),
        )));
        let has_document = Rc::new(RefCell::new(false));

        let mut model = AppModel {
            egui_area: None,
            session: Rc::clone(&session),
            has_document: Rc::clone(&has_document),
            open_path: None,
            subtitle: "No document".into(),
            status: String::new(),
        };

        // ------------------------------------------------------------------
        // 2) Widgets (uses `model` for #[watch])
        // ------------------------------------------------------------------
        let widgets = view_output!();

        // ------------------------------------------------------------------
        // 3) Egui area after model + widgets, then append
        // ------------------------------------------------------------------
        let session_for_ui = Rc::clone(&session);
        let has_document_for_ui = Rc::clone(&has_document);
        let sender_for_ui = sender.clone();

        let egui_area = EguiArea::default();
        egui_area.set_vexpand(true);
        egui_area.set_hexpand(true);

        egui_area.set_ui(move |ctx| {
            static FONTS: Once = Once::new();
            FONTS.call_once(|| setup_cjk_fonts(ctx));

            egui::CentralPanel::default().show(ctx, |ui| {
                if !*has_document_for_ui.borrow() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.35);
                        ui.heading("No document open");
                        ui.label("Open a file to render it on the canvas.");
                        ui.add_space(12.0);
                        if ui.button("Open document…").clicked() {
                            sender_for_ui.input(AppMsg::RequestOpen);
                        }
                    });
                    return;
                }
                paint_layout(ui, &session_for_ui);
            });
        });

        widgets.content_box.append(&egui_area);
        model.egui_area = Some(egui_area);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::RequestOpen => {
                let dialog = gtk::FileDialog::builder()
                    .title("Open document")
                    .modal(true)
                    .build();

                let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
                let all = gtk::FileFilter::new();
                all.set_name(Some("All files"));
                all.add_pattern("*");
                filters.append(&all);
                let text = gtk::FileFilter::new();
                text.set_name(Some("Text / JSON"));
                text.add_pattern("*.txt");
                text.add_pattern("*.json");
                filters.append(&text);
                dialog.set_filters(Some(&filters));

                let sender = sender.clone();
                dialog.open(
                    None::<&gtk::Window>,
                    None::<&gtk::gio::Cancellable>,
                    move |result| match result {
                        Ok(file) => {
                            if let Some(path) = file.path() {
                                sender.input(AppMsg::OpenPath(path));
                            } else {
                                sender.input(AppMsg::OpenFailed(
                                    "Could not get path from file".into(),
                                ));
                            }
                        }
                        Err(err) => {
                            if err.kind() != Some(gtk::gio::IOErrorEnum::Cancelled) {
                                sender.input(AppMsg::OpenFailed(err.to_string()));
                            }
                        }
                    },
                );
            }

            AppMsg::OpenPath(path) => match load_document(&path) {
                Ok(new_session) => {
                    *self.session.borrow_mut() = new_session;
                    self.subtitle = path.display().to_string();
                    self.open_path = Some(path);
                    *self.has_document.borrow_mut() = true;
                    self.status.clear();
                    // Ask the GL area to redraw after load
                    if let Some(area) = self.egui_area.as_ref() {
                        area.queue_draw();
                    }
                }
                Err(err) => {
                    eprintln!("[open] {err}");
                    sender.input(AppMsg::OpenFailed(err));
                }
            },

            AppMsg::OpenFailed(err) => {
                self.status = err;
            }
        }
    }
}

fn main() {
    let app = RelmApp::new("com.example.XPages");
    app.run::<AppModel>(());
}
