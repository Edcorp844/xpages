//! XPages — GTK shell + WordCraft document canvas (egui).
//!
//! Menus / window chrome: GTK (Relm4 + Adwaita).
//! Document pages: Session.layout() → wordcraft_render::render_page → egui textures.
//!
//! EguiArea::set_ui must receive `&mut egui::Ui` (egui 0.37 run_ui). Do not call
//! Context::root_ui from that callback.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Once;

use relm4::{adw, adw::prelude::*, prelude::*};
use serde_json::json;

mod components;
use components::EguiArea;

/// ~1 CSS pixel per typographic point (WordCraft canvas convention).
const PX_PER_PT: f32 = 96.0 / 72.0;
const DEFAULT_ZOOM: f32 = 1.0;
const PAGE_GAP: f32 = 20.0;
const MARGIN_X: f32 = 40.0;

// ---------------------------------------------------------------------------
// Fonts (optional CJK for empty-state UI labels)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Document open via WordCraft engine
// ---------------------------------------------------------------------------

fn load_document(path: &Path) -> Result<wordcraft_engine::Session, String> {
    let mut session =
        wordcraft_engine::Session::new(wordcraft_doc::Document::new());
    session
        .run(
            "file.open",
            &json!({ "path": path.to_string_lossy() }),
        )
        .map_err(|e| format!("file.open {}: {e}", path.display()))?;
    Ok(session)
}



// ---------------------------------------------------------------------------
// Canvas: full-page rasters as egui textures
// ---------------------------------------------------------------------------

struct DocCanvas {
    /// page index → (cache key, texture)
    textures: HashMap<usize, (u64, egui::TextureHandle)>,
}

impl Default for DocCanvas {
    fn default() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }
}

impl DocCanvas {
    fn clear(&mut self) {
        self.textures.clear();
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        session: &mut wordcraft_engine::Session,
        zoom: f32,
    ) {
        let layout = session.layout();
        let scale_px = (zoom * PX_PER_PT).max(0.25);

        // Cache key: page count + zoom bits (replace with session.rev() if public).
        let base_key = (layout.pages.len() as u64) ^ ((scale_px.to_bits() as u64) << 16);

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                for (i, page) in layout.pages.iter().enumerate() {
                    let screen_w = page.w * scale_px;
                    let screen_h = page.h * scale_px;
                    let row_w = (screen_w + MARGIN_X * 2.0).max(ui.available_width());
                    let row_h = screen_h + PAGE_GAP;

                    let (response, painter) =
                        ui.allocate_painter(egui::vec2(row_w, row_h), egui::Sense::hover());

                    let page_rect = egui::Rect::from_min_size(
                        response.rect.min + egui::vec2(MARGIN_X, 0.0),
                        egui::vec2(screen_w, screen_h),
                    );

                    // Soft shadow
                    painter.rect_filled(
                        page_rect.translate(egui::vec2(2.0, 2.0)),
                        2.0,
                        egui::Color32::from_black_alpha(28),
                    );

                    let key = base_key ^ ((i as u64) << 32);

                    let stale = self
                        .textures
                        .get(&i)
                        .map(|(k, _)| *k != key)
                        .unwrap_or(true);

                    if stale {
                        let img = wordcraft_render::render_page(
                            &session.doc,
                            page,
                            scale_px,
                            &wordcraft_render::RenderOptions::default(),
                        );
                        let color = egui::ColorImage::from_rgba_premultiplied(
                            [img.width as usize, img.height as usize],
                            &img.pixels,
                        );
                        match self.textures.get_mut(&i) {
                            Some((k, tex)) => {
                                tex.set(color, egui::TextureOptions::LINEAR);
                                *k = key;
                            }
                            None => {
                                let tex = ui.ctx().load_texture(
                                    format!("page{i}"),
                                    color,
                                    egui::TextureOptions::LINEAR,
                                );
                                self.textures.insert(i, (key, tex));
                            }
                        }
                    }

                    if let Some((_, tex)) = self.textures.get(&i) {
                        painter.image(
                            tex.id(),
                            page_rect,
                            egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            ),
                            egui::Color32::WHITE,
                        );
                    }
                }
            });

        let n = layout.pages.len();
        self.textures.retain(|i, _| *i < n);
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum AppMsg {
    RequestOpen,
    OpenPath(PathBuf),
    OpenFailed(String),
}

struct AppModel {
    egui_area: Option<EguiArea>,
    session: Rc<RefCell<wordcraft_engine::Session>>,
    has_document: Rc<RefCell<bool>>,
    canvas: Rc<RefCell<DocCanvas>>,
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
            set_content = &adw::ToolbarView {
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
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // 1) Model first so view_output! can see `model`
        let session = Rc::new(RefCell::new(wordcraft_engine::Session::new(
            wordcraft_doc::Document::new(),
        )));
        let has_document = Rc::new(RefCell::new(false));
        let canvas = Rc::new(RefCell::new(DocCanvas::default()));

        let mut model = AppModel {
            egui_area: None,
            session: Rc::clone(&session),
            has_document: Rc::clone(&has_document),
            canvas: Rc::clone(&canvas),
            open_path: None,
            subtitle: "No document".into(),
            status: String::new(),
        };

        // 2) Widgets
        let widgets = view_output!();

        // 3) Egui area after model + widgets, then append
        let session_for_ui = Rc::clone(&session);
        let has_document_for_ui = Rc::clone(&has_document);
        let canvas_for_ui = Rc::clone(&canvas);
        let sender_for_ui = sender.clone();

        let egui_area = EguiArea::default();
        egui_area.set_vexpand(true);
        egui_area.set_hexpand(true);

        // EguiArea must call this from inside ctx.run_ui(|ui| …).
        // Callback signature: FnMut(&mut egui::Ui)  — NOT &Context.
        egui_area.set_ui(move |ui: &mut egui::Ui| {
            static FONTS: Once = Once::new();
            FONTS.call_once(|| setup_cjk_fonts(ui.ctx()));

            // Already the root Ui — do not call ctx.root_ui().
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::from_gray(220)))
                .show(ui, |ui| {
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

                    let mut session = session_for_ui.borrow_mut();
                    let mut canvas = canvas_for_ui.borrow_mut();
                    canvas.show(ui, &mut session, DEFAULT_ZOOM);
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

                let office = gtk::FileFilter::new();
                office.set_name(Some("Word / OpenDocument"));
                for pat in ["*.docx", "*.docm", "*.dotx", "*.odt", "*.rtf"] {
                    office.add_pattern(pat);
                }
                filters.append(&office);

                let text = gtk::FileFilter::new();
                text.set_name(Some("Text / Markdown"));
                for pat in ["*.txt", "*.md", "*.markdown", "*.html", "*.htm"] {
                    text.add_pattern(pat);
                }
                filters.append(&text);

                let all = gtk::FileFilter::new();
                all.set_name(Some("All files"));
                all.add_pattern("*");
                filters.append(&all);

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
                    self.canvas.borrow_mut().clear();
                    self.subtitle = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    self.open_path = Some(path);
                    *self.has_document.borrow_mut() = true;
                    self.status.clear();
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