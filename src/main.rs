//! XPages — GTK shell + WordCraft document canvas (egui).
//!
//! - Chrome / sidebar thumbnails: GTK (Relm4 + Adwaita)
//! - Page view: Session.layout() → wordcraft_render::render_page → egui textures
//! - Scroll background follows Adwaita light/dark
//! - Pages are centered; sidebar thumbnails jump the scroll to that page
//!
//! EguiArea::set_ui must receive `&mut egui::Ui` (egui 0.37 `run_ui`).
//! Do not call `Context::root_ui` from that callback.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Once;

use relm4::{adw, adw::prelude::*, prelude::*};
use serde_json::json;

mod components;
use components::EguiArea;

const PX_PER_PT: f32 = 96.0 / 72.0;
const DEFAULT_ZOOM: f32 = 1.0;
const PAGE_GAP: f32 = 24.0;
const THUMB_SCALE: f32 = 0.12;

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

fn adwaita_is_dark() -> bool {
    adw::StyleManager::default().is_dark()
}

/// Adwaita window_bg-ish color for the egui scroll area (not the page paper).
fn adwaita_window_bg(is_dark: bool) -> egui::Color32 {
    if is_dark {
        egui::Color32::from_rgb(0x24, 0x24, 0x24)
    } else {
        egui::Color32::from_rgb(0xfa, 0xfa, 0xfa)
    }
}

// ---------------------------------------------------------------------------
// Fonts
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
// Document open
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
// Thumbnail → GDK texture (sidebar)
// ---------------------------------------------------------------------------

fn page_to_texture(
    session: &mut wordcraft_engine::Session,
    page_index: usize,
    scale: f32,
) -> Option<gtk::gdk::MemoryTexture> {
    let layout = session.layout();
    let page = layout.pages.get(page_index)?;

    let img = wordcraft_render::render_page(
        &session.doc,
        page,
        scale,
        &wordcraft_render::RenderOptions::default(),
    );

    if img.width == 0 || img.height == 0 {
        return None;
    }

    let bytes = gtk::glib::Bytes::from(img.pixels.as_slice());
    Some(gtk::gdk::MemoryTexture::new(
        img.width as i32,
        img.height as i32,
        gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied,
        &bytes,
        (img.width as usize) * 4,
    ))
}

// ---------------------------------------------------------------------------
// Canvas
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
        scroll_to_page: &mut Option<usize>,
    ) {
        let layout = session.layout();
        let scale_px = (zoom * PX_PER_PT).max(0.25);
        let base_key = (layout.pages.len() as u64) ^ ((scale_px.to_bits() as u64) << 16);

        let target = scroll_to_page.take();

        egui::ScrollArea::both()
            .id_salt("doc_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                for (i, page) in layout.pages.iter().enumerate() {
                    let screen_w = page.w * scale_px;
                    let screen_h = page.h * scale_px;
                    let row_w = ui.available_width().max(screen_w + 32.0);
                    let row_h = screen_h + PAGE_GAP;

                    let (response, painter) =
                        ui.allocate_painter(egui::vec2(row_w, row_h), egui::Sense::click());

                    if target == Some(i) {
                        response.scroll_to_me(Some(egui::Align::TOP));
                    }

                    // Centered page
                    let page_rect = egui::Rect::from_center_size(
                        egui::pos2(
                            response.rect.center().x,
                            response.rect.min.y + screen_h * 0.5,
                        ),
                        egui::vec2(screen_w, screen_h),
                    );

                    painter.rect_filled(
                        page_rect.translate(egui::vec2(3.0, 3.0)),
                        2.0,
                        egui::Color32::from_black_alpha(40),
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

                    painter.rect_stroke(
                        page_rect,
                        0.0,
                        egui::Stroke::new(1.0, egui::Color32::from_black_alpha(50)),
                        egui::StrokeKind::Outside,
                    );
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
    GoToPage(usize),
    ThemeChanged,
}

struct AppModel {
    egui_area: Option<EguiArea>,
    session: Rc<RefCell<wordcraft_engine::Session>>,
    has_document: Rc<RefCell<bool>>,
    canvas: Rc<RefCell<DocCanvas>>,
    scroll_to_page: Rc<RefCell<Option<usize>>>,
    is_dark: Rc<RefCell<bool>>,
    open_path: Option<PathBuf>,
    subtitle: String,
    status: String,
    page_count: usize,
    /// Stored so `update` can rebuild sidebar without `update_with_view`.
    thumb_list: Option<gtk::ListBox>,
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
            set_default_size: (1100, 720),

            #[wrap(Some)]
            set_content = &adw::OverlaySplitView {
                set_sidebar_position: gtk::PackType::Start,
                set_show_sidebar: true,
                set_min_sidebar_width: 140.0,
                set_max_sidebar_width: 220.0,

                #[wrap(Some)]
                set_sidebar = &adw::ToolbarView {
                    add_top_bar = &adw::HeaderBar {
                        #[wrap(Some)]
                        set_title_widget = &gtk::Label {
                            set_label: "Pages",
                            add_css_class: "title",
                        },
                    },

                    #[wrap(Some)]
                    #[name = "thumb_scroll"]
                    set_content = &gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                        #[wrap(Some)]
                        #[name = "thumb_list"]
                        set_child = &gtk::ListBox {
                            set_selection_mode: gtk::SelectionMode::Single,
                            add_css_class: "navigation-sidebar",
                        },
                    },
                },

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
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let session = Rc::new(RefCell::new(wordcraft_engine::Session::new(
            wordcraft_doc::Document::new(),
        )));
        let has_document = Rc::new(RefCell::new(false));
        let canvas = Rc::new(RefCell::new(DocCanvas::default()));
        let scroll_to_page = Rc::new(RefCell::new(None));
        let is_dark = Rc::new(RefCell::new(adwaita_is_dark()));

        let mut model = AppModel {
            egui_area: None,
            session: Rc::clone(&session),
            has_document: Rc::clone(&has_document),
            canvas: Rc::clone(&canvas),
            scroll_to_page: Rc::clone(&scroll_to_page),
            is_dark: Rc::clone(&is_dark),
            open_path: None,
            subtitle: "No document".into(),
            status: String::new(),
            page_count: 0,
            thumb_list: None,
        };

        let widgets = view_output!();

        // Sidebar click → jump to page (connect once)
        {
            let sender = sender.clone();
            widgets.thumb_list.connect_row_activated(move |_, row| {
                let index = row.index();
                if index >= 0 {
                    sender.input(AppMsg::GoToPage(index as usize));
                }
            });
        }
        model.thumb_list = Some(widgets.thumb_list.clone());

        // Adwaita theme changes
        {
            let is_dark = Rc::clone(&is_dark);
            let sender = sender.clone();
            adw::StyleManager::default().connect_dark_notify(move |sm| {
                *is_dark.borrow_mut() = sm.is_dark();
                sender.input(AppMsg::ThemeChanged);
            });
        }

        let session_for_ui = Rc::clone(&session);
        let has_document_for_ui = Rc::clone(&has_document);
        let canvas_for_ui = Rc::clone(&canvas);
        let scroll_for_ui = Rc::clone(&scroll_to_page);
        let is_dark_for_ui = Rc::clone(&is_dark);
        let sender_for_ui = sender.clone();

        let egui_area = EguiArea::default();
        egui_area.set_vexpand(true);
        egui_area.set_hexpand(true);

        egui_area.set_ui(move |ui: &mut egui::Ui| {
            static FONTS: Once = Once::new();
            FONTS.call_once(|| setup_cjk_fonts(ui.ctx()));

            let dark = *is_dark_for_ui.borrow();
            let bg = adwaita_window_bg(dark);

            ui.ctx().set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(bg))
                .show(ui, |ui| {
                    if !*has_document_for_ui.borrow() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(ui.available_height() * 0.35);
                            ui.heading("No document open");
                            ui.label("Open a file to view pages. Use the sidebar to jump.");
                            ui.add_space(12.0);
                            if ui.button("Open document…").clicked() {
                                sender_for_ui.input(AppMsg::RequestOpen);
                            }
                        });
                        return;
                    }

                    let mut session = session_for_ui.borrow_mut();
                    let mut canvas = canvas_for_ui.borrow_mut();
                    let mut target = scroll_for_ui.borrow_mut();
                    canvas.show(ui, &mut session, DEFAULT_ZOOM, &mut target);
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
                Ok(mut new_session) => {
                    let page_count = new_session.layout().pages.len();

                    *self.session.borrow_mut() = new_session;
                    self.canvas.borrow_mut().clear();
                    *self.scroll_to_page.borrow_mut() = Some(0);
                    self.subtitle = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    self.open_path = Some(path);
                    *self.has_document.borrow_mut() = true;
                    self.status.clear();
                    self.page_count = page_count;

                    if let Some(list) = self.thumb_list.as_ref() {
                        rebuild_thumb_list(list, &self.session, page_count);
                    }

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

            AppMsg::GoToPage(index) => {
                *self.scroll_to_page.borrow_mut() = Some(index);
                if let Some(area) = self.egui_area.as_ref() {
                    area.queue_draw();
                }
            }

            AppMsg::ThemeChanged => {
                if let Some(area) = self.egui_area.as_ref() {
                    area.queue_draw();
                }
            }
        }
    }
}

/// Fill the sidebar with page thumbnails. Row activation is connected once in init.
fn rebuild_thumb_list(
    list: &gtk::ListBox,
    session: &Rc<RefCell<wordcraft_engine::Session>>,
    page_count: usize,
) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }

    for i in 0..page_count {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(true);

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 4);
        vbox.set_margin_top(8);
        vbox.set_margin_bottom(8);
        vbox.set_margin_start(8);
        vbox.set_margin_end(8);
        vbox.set_halign(gtk::Align::Center);

        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_can_shrink(true);
        picture.set_size_request(100, 130);

        {
            let mut session = session.borrow_mut();
            if let Some(tex) = page_to_texture(&mut session, i, THUMB_SCALE) {
                picture.set_paintable(Some(&tex));
            }
        }

        let label = gtk::Label::new(Some(&format!("Page {}", i + 1)));
        label.add_css_class("caption");

        vbox.append(&picture);
        vbox.append(&label);
        row.set_child(Some(&vbox));
        list.append(&row);
    }
}

fn main() {
    adw::init().expect("adw::init failed");

    let app = RelmApp::new("com.example.XPages");
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);
    app.run::<AppModel>(());
}