use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Once;

use relm4::{adw, adw::prelude::*, prelude::*};
use serde_json::json;

use crate::components::sidebar::{rebuild_thumb_list, rebuild_toc_list};
use crate::components::{EguiArea, MainMenuButton};
use crate::editor::input::handle_editing;
use crate::state::DocumentCanvas;
use crate::utils::fonts::setup_cjk_fonts;
use crate::utils::theme::{adwaita_is_dark, transparent_visuals};

#[derive(Debug)]
pub enum AppMsg {
    RequestOpen,
    OpenPath(PathBuf),
    OpenFailed(String),
    GoToPage(usize),
    ThemeChanged,
}

pub struct XPagesApp {
    egui_area: Option<EguiArea>,
    session: Rc<RefCell<wordcraft_engine::Session>>,
    has_document: Rc<RefCell<bool>>,
    canvas: Rc<RefCell<DocumentCanvas>>,
    scroll_to_page: Rc<RefCell<Option<usize>>>,
    is_dark: Rc<RefCell<bool>>,
    subtitle: String,
    status: String,
    page_count: usize,
    thumb_list: Option<gtk::ListBox>,
    toc_list: Option<gtk::ListBox>,
    main_menu_button: Controller<MainMenuButton>,
}

#[relm4::component(pub)]
impl SimpleComponent for XPagesApp {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        #[name = "window"]
        adw::ApplicationWindow {
            set_title: Some("XPages"),
            set_default_size: (1200, 900),

            #[wrap(Some)]
            set_content = &adw::OverlaySplitView {
                set_sidebar_position: gtk::PackType::Start,
                set_show_sidebar: true,
                set_min_sidebar_width: 160.0,
                set_max_sidebar_width: 260.0,

                // -------------------- Sidebar --------------------
                #[wrap(Some)]
                set_sidebar = &adw::ToolbarView {
                    add_top_bar = &adw::HeaderBar {
                        #[wrap(Some)]
                        set_title_widget = &gtk::Label {
                            set_label: "Navigate",
                            add_css_class: "title",
                        },
                        pack_end = model.main_menu_button.widget() {}
                    },

                    #[wrap(Some)]
                    #[name = "side_stack"]
                    set_content = &adw::ViewStack {
                        set_vexpand: true,

                        // Pages — thumbnails
                        add_titled[Some("pages"), "Pages"] = &gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                            #[wrap(Some)]
                            #[name = "thumb_list"]
                            set_child = &gtk::ListBox {
                                set_selection_mode: gtk::SelectionMode::Single,
                                add_css_class: "navigation-sidebar",
                                set_valign: gtk::Align::Start,
                                set_vexpand: false,
                            },
                        },

                        // Contents — hierarchical TOC
                        add_titled[Some("contents"), "Contents"] = &gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                            #[wrap(Some)]
                            #[name = "toc_list"]
                            set_child = &gtk::ListBox {
                                set_selection_mode: gtk::SelectionMode::Single,
                                add_css_class: "navigation-sidebar",
                                set_valign: gtk::Align::Start,
                                set_vexpand: false,
                            },
                        },
                    },

                    // Document-viewer style switcher under the stack
                    #[name = "side_switcher"]
                    add_bottom_bar = &adw::ViewSwitcherBar {
                        set_reveal: true,
                    },
                },

                // -------------------- Main content --------------------
                #[wrap(Some)]
                set_content = &adw::ToolbarView {
                    add_top_bar = &adw::HeaderBar {
                        #[wrap(Some)]
                        set_title_widget = &adw::WindowTitle {
                            #[watch]
                            set_title: &model.subtitle,
                        },
                    },

                    add_top_bar = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_horizontal: 20,
                        set_margin_vertical: 6,
                        set_spacing: 12,
                        add_css_class: "toolbar",

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_halign: gtk::Align::Start,
                            add_css_class: "linked",

                            gtk::Button {
                                set_icon_name: "edit-undo-symbolic",
                                set_tooltip_text: Some("Undo"),
                            },
                            gtk::Button {
                                set_icon_name: "edit-redo-symbolic",
                                set_tooltip_text: Some("Redo"),
                            },
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_halign: gtk::Align::Center,
                            set_hexpand: true,
                            set_spacing: 12,

                            gtk::Button {
                                set_icon_name: "add-page-symbolic",
                                set_tooltip_text: Some("Insert Page"),
                            },
                            gtk::Button {
                                set_icon_name: "shape-text-data-symbolic",
                                set_tooltip_text: Some("Insert Text"),
                            },
                            gtk::Button {
                                set_icon_name: "am-pie-symbolic",
                                set_tooltip_text: Some("Insert Graph"),
                            },
                            gtk::Button {
                                set_icon_name: "pen-shaper-symbolic",
                                set_tooltip_text: Some("Insert Shape"),
                            },
                            gtk::Button {
                                set_icon_name: "multimedia-photo-viewer-symbolic",
                                set_tooltip_text: Some("Insert Media"),
                            },
                            gtk::Button {
                                set_icon_name: "link-symbolic",
                                set_tooltip_text: Some("Insert Link"),
                            },
                            gtk::Button {
                                set_icon_name: "mxml-comment-symbolic",
                                set_tooltip_text: Some("Insert Comment"),
                            },
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_halign: gtk::Align::End,

                            gtk::Button {
                                set_icon_name: "share-symbolic",
                                set_tooltip_text: Some("Share"),
                            },
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
        let canvas = Rc::new(RefCell::new(DocumentCanvas::default()));
        let scroll_to_page = Rc::new(RefCell::new(None));
        let is_dark = Rc::new(RefCell::new(adwaita_is_dark()));

        let main_menu_button = MainMenuButton::builder().launch(()).detach();
        let mut model = XPagesApp {
            egui_area: None,
            session: Rc::clone(&session),
            has_document: Rc::clone(&has_document),
            canvas: Rc::clone(&canvas),
            scroll_to_page: Rc::clone(&scroll_to_page),
            is_dark: Rc::clone(&is_dark),
            subtitle: "No document".into(),
            status: String::new(),
            page_count: 0,
            thumb_list: None,
            toc_list: None,
            main_menu_button,
        };

        let widgets = view_output!();

        // ViewSwitcherBar ↔ ViewStack (Document Viewer pattern)
        widgets.side_switcher.set_stack(Some(&widgets.side_stack));

        // Optional icons on stack pages
        if let Some(child) = widgets.side_stack.child_by_name("pages") {
            widgets
                .side_stack
                .page(&child)
                .set_icon_name(Some("view-paged-symbolic"));
        }
        if let Some(child) = widgets.side_stack.child_by_name("contents") {
            widgets
                .side_stack
                .page(&child)
                .set_icon_name(Some("view-list-symbolic"));
        }

        // Thumbnails → page index
        {
            let sender = sender.clone();
            widgets.thumb_list.connect_row_activated(move |_, row| {
                if row.index() >= 0 {
                    sender.input(AppMsg::GoToPage(row.index() as usize));
                }
            });
        }

        // TOC → page stored in widget_name
        {
            let sender = sender.clone();
            widgets.toc_list.connect_row_activated(move |_, row| {
                if let Ok(page) = row.widget_name().parse::<usize>() {
                    sender.input(AppMsg::GoToPage(page));
                }
            });
        }

        model.thumb_list = Some(widgets.thumb_list.clone());
        model.toc_list = Some(widgets.toc_list.clone());

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
            ui.ctx().set_visuals(transparent_visuals(dark));

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());

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

                    let focus_id = egui::Id::unique("doc_focus");
                    let mut session = session_for_ui.borrow_mut();
                    let mut canvas = canvas_for_ui.borrow_mut();
                    let mut target = scroll_for_ui.borrow_mut();

                    let edited = handle_editing(ui, &mut session, &mut canvas, focus_id);
                    if edited {
                        canvas.clear();
                        ui.ctx().request_repaint();
                    }

                    canvas.show(ui, &mut session, &mut target, focus_id);
                });
        });

        widgets.content_box.append(&egui_area);
        model.egui_area = Some(egui_area);

        let app = relm4::main_application();
        if let Some(open_action) = app
            .lookup_action("open")
            .and_then(|a| a.downcast::<gtk::gio::SimpleAction>().ok())
        {
            let sender = sender.clone();
            open_action.connect_activate(move |_, _| {
                sender.input(AppMsg::RequestOpen);
            });
        } else {
            eprintln!("WARNING: app.open action not found – was register() called?");
        }

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
                office.set_name(Some("Word documents"));
                office.add_pattern("*.docx");
                office.add_pattern("*.doc");
                office.add_pattern("*.odt");
                office.add_pattern("*.rtf");
                office.add_mime_type(
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                );
                office.add_mime_type("application/msword");
                office.add_mime_type("application/vnd.oasis.opendocument.text");
                filters.append(&office);

                let all = gtk::FileFilter::new();
                all.set_name(Some("All files"));
                all.add_pattern("*");
                filters.append(&all);

                dialog.set_filters(Some(&filters));
                dialog.set_default_filter(Some(&office));

                let sender = sender.clone();
                dialog.open(
                    None::<&gtk::Window>,
                    None::<&gtk::gio::Cancellable>,
                    move |result| match result {
                        Ok(file) => {
                            if let Some(path) = file.path() {
                                sender.input(AppMsg::OpenPath(path));
                            } else {
                                sender.input(AppMsg::OpenFailed("Could not get path".into()));
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

            AppMsg::OpenPath(path) => {
                let mut new_session =
                    wordcraft_engine::Session::new(wordcraft_doc::Document::new());
                match new_session.run("file.open", &json!({ "path": path.to_string_lossy() })) {
                    Ok(_) => {
                        let page_count = new_session.layout().pages.len();
                        *self.session.borrow_mut() = new_session;
                        self.canvas.borrow_mut().clear();
                        *self.scroll_to_page.borrow_mut() = Some(0);
                        self.subtitle = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string());
                        *self.has_document.borrow_mut() = true;
                        self.status.clear();
                        self.page_count = page_count;

                        if let Some(list) = self.thumb_list.as_ref() {
                            rebuild_thumb_list(list, &self.session, page_count);
                        }
                        if let Some(list) = self.toc_list.as_ref() {
                            rebuild_toc_list(list, &self.session);
                        }
                        if let Some(area) = self.egui_area.as_ref() {
                            area.grab_focus();
                            area.queue_draw();
                        }
                    }
                    Err(err) => {
                        sender.input(AppMsg::OpenFailed(err.to_string()));
                    }
                }
            }

            AppMsg::OpenFailed(err) => {
                self.status = err;
            }

            AppMsg::GoToPage(index) => {
                *self.scroll_to_page.borrow_mut() = Some(index);
                if let Some(area) = self.egui_area.as_ref() {
                    area.grab_focus();
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
