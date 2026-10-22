//
//  egui_area.rs
//

use gtk::subclass::prelude::*;
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

glib::wrapper! {
    /// Thin wrapper around a GObject subclass implemented in the `imp` module.
    pub struct EguiArea(ObjectSubclass<imp::EguiArea>)
        @extends gtk::GLArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl EguiArea {
    pub fn new(ui: impl Fn(&mut egui::Ui) + 'static) -> Self {
        let area: Self = glib::Object::builder().build();
        area.set_ui(ui);
        area
    }

    pub fn with_max_fps(ui: impl Fn(&mut egui::Ui) + 'static, max_fps: u32) -> Self {
        let area = Self::new(ui);
        area.set_max_fps(max_fps);
        area
    }

    pub fn set_max_fps(&self, max_fps: u32) {
        self.imp().min_render_interval.set(Some(Duration::from_micros(
            ((1000.0 / max_fps as f64) * 1000.0) as u64,
        )));
    }

    pub fn set_ui(&self, ui: impl Fn(&mut egui::Ui) + 'static) {
        *self.imp().run_ui.borrow_mut() = Some(Box::new(ui));
    }

    pub fn egui_ctx(&self) -> &egui::Context {
        &self.imp().egui_ctx
    }
}

impl Default for EguiArea {
    fn default() -> Self {
        Self::new(|_ui| {})
    }
}

mod imp {
    use super::*;
    use egui;
    use egui_glow::glow;
    use gtk::{gdk, gio};

    type DynGuiFn = Box<dyn Fn(&mut egui::Ui)>;

    #[derive(Default)]
    pub struct EguiArea {
        painter: RefCell<Option<egui_glow::Painter>>,
        pub(super) egui_ctx: egui::Context,
        input_events: RefCell<Vec<egui::Event>>,
        pub(super) min_render_interval: Cell<Option<Duration>>,
        pub(super) run_ui: RefCell<Option<DynGuiFn>>,
        im_context: RefCell<Option<gtk::IMMulticontext>>,
        focused: Cell<bool>,
        modifiers: Rc<Cell<egui::Modifiers>>,
        tick_id: RefCell<Option<gtk::TickCallbackId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EguiArea {
        const NAME: &'static str = "EguiArea";
        type Type = super::EguiArea;
        type ParentType = gtk::GLArea;
    }

    impl ObjectImpl for EguiArea {
        fn constructed(&self) {
            self.parent_constructed();

            init_epoxy();

            let obj = self.obj().clone();
            obj.set_can_focus(true);
            obj.set_focusable(true);
            obj.set_focus_on_click(true);
            obj.set_hexpand(true);
            obj.set_vexpand(true);

            let im = gtk::IMMulticontext::new();
            im.set_use_preedit(true);
            im.set_client_widget(Some(&obj));
            *self.im_context.borrow_mut() = Some(im.clone());

            self.register_controllers();

            let im_for_focus = self.im_context.borrow().clone();
            obj.connect_notify_local(Some("has-focus"), move |widget, _pspec| {
                if let Some(im) = im_for_focus.as_ref() {
                    if widget.has_focus() {
                        im.focus_in();
                    } else {
                        im.focus_out();
                    }
                }
            });

            let last_render = Cell::new(Instant::now());
            let tick_id = obj.add_tick_callback(move |area, _clock| {
                let should_render = match area.imp().min_render_interval.get() {
                    Some(min_interval) => last_render.get().elapsed() > min_interval,
                    None => true,
                };
                if should_render {
                    area.queue_render();
                    last_render.set(Instant::now());
                }
                glib::ControlFlow::Continue
            });
            *self.tick_id.borrow_mut() = Some(tick_id);

            if let Some(im) = self.im_context.borrow().as_ref() {
                let obj = self.obj().downgrade();
                im.connect_commit(move |_im, text| {
                    if let Some(obj) = obj.upgrade() {
                        let mut events = obj.imp().input_events.borrow_mut();
                        events.push(egui::Event::Ime(egui::ImeEvent::Commit(text.to_string())));
                    }
                });

                let obj = self.obj().downgrade();
                im.connect_preedit_changed(move |im| {
                    let (preedit, _attrs, _pos) = im.preedit_string();
                    if let Some(obj) = obj.upgrade() {
                        let mut events = obj.imp().input_events.borrow_mut();
                        events.push(egui::Event::Ime(egui::ImeEvent::Preedit {
                            text: preedit.to_string(),
                            active_range_chars: None,
                        }));
                    }
                });
            }
        }

        fn dispose(&self) {
            if let Some(im) = self.im_context.borrow_mut().take() {
                im.set_client_widget(None::<&gtk::Widget>);
            }
        }
    }

    impl WidgetImpl for EguiArea {
        fn realize(&self) {
            self.parent_realize();
            self.obj().make_current();
            let gl = unsafe { glow::Context::from_loader_function(epoxy::get_proc_addr) };
            let gl = Arc::new(gl);
            *self.painter.borrow_mut() = Some(
                egui_glow::Painter::new(gl, "", None, true).expect("Failed to create painter"),
            );
        }

        fn unrealize(&self) {
            if let Some(id) = self.tick_id.borrow_mut().take() {
                id.remove();
            }
            if let Some(mut painter) = self.painter.borrow_mut().take() {
                painter.destroy();
            }
            self.parent_unrealize();
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            self.parent_compute_expand(hexpand, vexpand)
        }
        fn contains(&self, x: f64, y: f64) -> bool {
            self.parent_contains(x, y)
        }
        fn direction_changed(&self, previous_direction: gtk::TextDirection) {
            self.parent_direction_changed(previous_direction)
        }
        fn focus(&self, direction_type: gtk::DirectionType) -> bool {
            self.parent_focus(direction_type)
        }
        fn request_mode(&self) -> gtk::SizeRequestMode {
            self.parent_request_mode()
        }
        fn grab_focus(&self) -> bool {
            self.parent_grab_focus()
        }
        fn keynav_failed(&self, direction_type: gtk::DirectionType) -> bool {
            self.parent_keynav_failed(direction_type)
        }
        fn map(&self) {
            self.parent_map()
        }
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            self.parent_measure(orientation, for_size)
        }
        fn mnemonic_activate(&self, group_cycling: bool) -> bool {
            self.parent_mnemonic_activate(group_cycling)
        }
        fn move_focus(&self, direction_type: gtk::DirectionType) {
            self.parent_move_focus(direction_type)
        }
        fn query_tooltip(
            &self,
            x: i32,
            y: i32,
            keyboard_tooltip: bool,
            tooltip: &gtk::Tooltip,
        ) -> bool {
            self.parent_query_tooltip(x, y, keyboard_tooltip, tooltip)
        }
        fn root(&self) {
            self.parent_root()
        }
        fn set_focus_child(&self, child: Option<&gtk::Widget>) {
            self.parent_set_focus_child(child)
        }
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline)
        }
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.parent_snapshot(snapshot)
        }
        fn state_flags_changed(&self, state_flags: &gtk::StateFlags) {
            self.parent_state_flags_changed(state_flags)
        }
        fn system_setting_changed(&self, settings: &gtk::SystemSetting) {
            self.parent_system_setting_changed(settings)
        }
        fn unmap(&self) {
            self.parent_unmap()
        }
        fn unroot(&self) {
            self.parent_unroot()
        }
    }

    impl GLAreaImpl for EguiArea {
        fn render(&self, _context: &gdk::GLContext) -> glib::Propagation {
            let area = self.obj();

            // ---------------------------------------------------------------
            // Coordinate system (Linux / macOS / Windows)
            //
            // GTK reports width/height and pointer events in *logical points*.
            // The real OpenGL drawable is *physical pixels* =
            //   logical × surface.scale_factor()   (display scale only)
            //
            // Accessibility text scale (GNOME Large Text) is handled by:
            //   • shrinking the virtual screen  (logical / text)
            //   • raising pixels_per_point      (display × text)
            //   • converting pointer events     (pos / text)
            // so fonts stay sharp and the UI grows with the rest of the desktop.
            //
            // On macOS / Windows text_scale is always 1.0.
            // ---------------------------------------------------------------

            let display = self.scale_factor().max(1.0);
            let text = self.text_scale();
            let ppp = (display * text).max(1.0);

            let logical_w = area.width() as f32;
            let logical_h = area.height() as f32;

            if logical_w <= 0.0 || logical_h <= 0.0 {
                return glib::Propagation::Stop;
            }

            // Real framebuffer size – never includes text scale
            let physical = [
                (logical_w * display).round().max(1.0) as u32,
                (logical_h * display).round().max(1.0) as u32,
            ];

            // Virtual size shrinks when text > 1 → UI elements become larger
            let virtual_w = logical_w / text;
            let virtual_h = logical_h / text;

            self.egui_ctx.set_pixels_per_point(ppp);

            let bg_color = self.egui_ctx.global_style().visuals.window_fill();

            let focused_now = area.has_focus();
            if focused_now != self.focused.get() {
                self.focused.set(focused_now);
                if let Some(im) = self.im_context.borrow().as_ref() {
                    if focused_now {
                        im.focus_in();
                    } else {
                        im.focus_out();
                    }
                }
            }

            let mut painter_guard = self.painter.borrow_mut();
            let Some(painter) = painter_guard.as_mut() else {
                return glib::Propagation::Stop;
            };

            unsafe {
                use egui_glow::glow::HasContext;
                let gl = painter.gl();
                gl.viewport(0, 0, physical[0] as i32, physical[1] as i32);
            }

            painter.clear(physical, bg_color.to_normalized_gamma_f32());

            if let Some(run_ui) = self.run_ui.borrow().as_ref() {
                let input_events: Vec<egui::Event> =
                    std::mem::take(self.input_events.borrow_mut().as_mut());

                // Convert pointer events by the text-scale factor only.
                // (On macOS/Windows text == 1.0 → no change.)
                let converted_events: Vec<egui::Event> = input_events
                    .into_iter()
                    .map(|ev| match ev {
                        egui::Event::PointerMoved(p) => {
                            egui::Event::PointerMoved(p / text)
                        }
                        egui::Event::PointerButton {
                            pos,
                            button,
                            pressed,
                            modifiers,
                        } => egui::Event::PointerButton {
                            pos: pos / text,
                            button,
                            pressed,
                            modifiers,
                        },
                        other => other,
                    })
                    .collect();

                let input = egui::RawInput {
                    events: converted_events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(virtual_w, virtual_h),
                    )),
                    viewports: [(
                        egui::ViewportId::ROOT,
                        egui::ViewportInfo {
                            native_pixels_per_point: Some(ppp),
                            focused: Some(focused_now),
                            ..Default::default()
                        },
                    )]
                    .into_iter()
                    .collect(),
                    focused: focused_now,
                    ..egui::RawInput::default()
                };

                // egui 0.37 API
                let mut full_output = self.egui_ctx.run_ui(input, |ui| {
                    run_ui(ui);
                });

                self.apply_gtk_cursor(&full_output.platform_output.cursor_icon);
                self.handle_platform_output(full_output.platform_output);

                let clipped_primitives = self
                    .egui_ctx
                    .tessellate(full_output.shapes, full_output.pixels_per_point);

                painter.paint_and_update_textures(
                    physical,
                    ppp,
                    &clipped_primitives,
                    &mut full_output.textures_delta,
                );
            }

            glib::Propagation::Stop
        }
    }

    impl EguiArea {
        fn scale_factor(&self) -> f32 {
            if let Some(native) = self.obj().native() {
                if let Some(surface) = native.surface() {
                    return surface.scale_factor() as f32;
                }
            }
            1.0
        }

        /// Accessibility / GNOME text scale.
        /// Always 1.0 on macOS and Windows.
        fn text_scale(&self) -> f32 {
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                gtk::Settings::default()
                    .map(|s| {
                        let dpi = s.property::<i32>("gtk-xft-dpi") as f32 / 1024.0;
                        if dpi > 0.0 {
                            (dpi / 96.0).clamp(0.5, 4.0)
                        } else {
                            1.0
                        }
                    })
                    .unwrap_or(1.0)
            }
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            {
                1.0
            }
        }

        fn handle_platform_output(&self, output: egui::PlatformOutput) {
            for cmd in output.commands {
                match cmd {
                    egui::OutputCommand::CopyText(text) => {
                        if !text.is_empty() {
                            let clipboard = self.obj().clipboard();
                            clipboard.set_text(&text);
                        }
                    }
                    egui::OutputCommand::CopyImage(_image) => {
                        eprintln!("CopyImage not implemented");
                    }
                    egui::OutputCommand::OpenUrl(open) => {
                        let window = self
                            .obj()
                            .root()
                            .and_then(|r| r.downcast::<gtk::Window>().ok());
                        let launcher = gtk::UriLauncher::builder().uri(&open.url).build();
                        if let Some(win) = window.as_ref() {
                            launcher.launch(
                                Some(win),
                                None::<&gtk::gio::Cancellable>,
                                |result| {
                                    if let Err(e) = result {
                                        eprintln!("Failed to launch URI: {}", e);
                                    }
                                },
                            );
                        } else {
                            launcher.launch(
                                None::<&gtk::Window>,
                                None::<&gtk::gio::Cancellable>,
                                |result| {
                                    if let Err(e) = result {
                                        eprintln!("Failed to launch URI: {}", e);
                                    }
                                },
                            );
                        }
                    }
                    egui::OutputCommand::TextSelectionSettled(selection) => {
                        eprintln!("TextSelectionSettled: {:?}", selection);
                    }
                }
            }

            if let Some(ime) = output.ime {
                if let Some(im) = self.im_context.borrow().as_ref() {
                    let ppp = self.egui_ctx.pixels_per_point();
                    let cursor = ime.cursor_rect;
                    let x = (cursor.min.x * ppp).round() as i32;
                    let y = (cursor.min.y * ppp).round() as i32;
                    let w = ((cursor.max.x - cursor.min.x) * ppp).ceil() as i32;
                    let h = ((cursor.max.y - cursor.min.y) * ppp).ceil() as i32;
                    let rect = gdk::Rectangle::new(x, y, w, h);
                    im.set_cursor_location(&rect);
                }
            }
        }

        fn register_controllers(&self) {
            let obj = self.obj().clone();

            let current_modifiers = self.modifiers.clone();
            current_modifiers.set(egui::Modifiers::default());

            // ---- Pointer click -------------------------------------------------
            let gesture_click = gtk::GestureClick::new();
            gesture_click.set_button(0);
            {
                let obj = obj.clone();
                let current_modifiers = current_modifiers.clone();
                gesture_click.connect_pressed(move |gesture, _num, x, y| {
                    obj.grab_focus();
                    let mut events = obj.imp().input_events.borrow_mut();
                    let button = match gesture.current_button() {
                        1 => egui::PointerButton::Primary,
                        3 => egui::PointerButton::Secondary,
                        2 => egui::PointerButton::Middle,
                        _ => return,
                    };
                    events.push(egui::Event::PointerButton {
                        pos: egui::pos2(x as f32, y as f32),
                        button,
                        pressed: true,
                        modifiers: current_modifiers.get(),
                    });
                });
            }
            {
                let obj = obj.clone();
                let current_modifiers = current_modifiers.clone();
                gesture_click.connect_released(move |gesture, _num, x, y| {
                    let mut events = obj.imp().input_events.borrow_mut();
                    let button = match gesture.current_button() {
                        1 => egui::PointerButton::Primary,
                        3 => egui::PointerButton::Secondary,
                        2 => egui::PointerButton::Middle,
                        _ => return,
                    };
                    events.push(egui::Event::PointerButton {
                        pos: egui::pos2(x as f32, y as f32),
                        button,
                        pressed: false,
                        modifiers: current_modifiers.get(),
                    });
                });
            }

            // ---- Pointer motion ------------------------------------------------
            let motion = gtk::EventControllerMotion::new();
            {
                let obj = obj.clone();
                motion.connect_motion(move |_motion, x, y| {
                    let mut events = obj.imp().input_events.borrow_mut();
                    events.push(egui::Event::PointerMoved(egui::pos2(x as f32, y as f32)));
                });
            }
            {
                let obj = obj.clone();
                motion.connect_leave(move |_motion| {
                    let mut events = obj.imp().input_events.borrow_mut();
                    events.push(egui::Event::PointerGone);
                });
            }

            // ---- Scroll --------------------------------------------------------
            let scroll = gtk::EventControllerScroll::new(
                gtk::EventControllerScrollFlags::BOTH_AXES
                    | gtk::EventControllerScrollFlags::DISCRETE,
            );
            {
                let obj = obj.clone();
                let current_modifiers = current_modifiers.clone();
                scroll.connect_scroll(move |_scroll, x, y| {
                    let mut events = obj.imp().input_events.borrow_mut();
                    events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: egui::Vec2::new(-x as f32, -y as f32),
                        phase: egui::TouchPhase::Move,
                        modifiers: current_modifiers.get(),
                        source: egui::MouseWheelSource::Wheel,
                    });
                    glib::Propagation::Proceed
                });
            }

            // ---- Keyboard ------------------------------------------------------
            let key_controller = gtk::EventControllerKey::new();
            {
                let obj = obj.clone();
                key_controller.connect_key_pressed(move |_controller, key, _code, modifiers| {
                    let mut events = obj.imp().input_events.borrow_mut();
                    let emod = gdk_to_egui_modifiers(modifiers);

                    if key == gdk::Key::BackSpace {
                        events.push(egui::Event::Key {
                            key: egui::Key::Backspace,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: emod,
                        });
                        return glib::Propagation::Proceed;
                    }

                    if modifiers.is_empty() || modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
                        if let Some(ch) = key.to_unicode() {
                            events.push(egui::Event::Text(ch.into()));
                        }
                    }

                    if let Some(ekey) = gdk_to_egui_key(key) {
                        if is_cut_command(emod, ekey) {
                            events.push(egui::Event::Cut);
                        } else if is_copy_command(emod, ekey) {
                            events.push(egui::Event::Copy);
                        } else if is_paste_command(emod, ekey) {
                            let clipboard = obj.clipboard();
                            let obj = obj.clone();
                            clipboard.read_text_async(gio::Cancellable::NONE, move |result| {
                                if let Ok(Some(text)) = result {
                                    obj.imp()
                                        .input_events
                                        .borrow_mut()
                                        .push(egui::Event::Paste(text.into()));
                                }
                            });
                        }

                        events.push(egui::Event::Key {
                            key: ekey,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: emod,
                        });
                    }

                    glib::Propagation::Proceed
                });
            }
            {
                let obj = obj.clone();
                key_controller.connect_key_released(move |_controller, key, _code, modifiers| {
                    if let Some(ekey) = gdk_to_egui_key(key) {
                        let mut events = obj.imp().input_events.borrow_mut();
                        events.push(egui::Event::Key {
                            key: ekey,
                            physical_key: None,
                            pressed: false,
                            repeat: false,
                            modifiers: gdk_to_egui_modifiers(modifiers),
                        });
                    }
                });
            }
            {
                let current_modifiers = current_modifiers.clone();
                key_controller.connect_modifiers(move |_controller, new_modifiers| {
                    current_modifiers.set(gdk_to_egui_modifiers(new_modifiers));
                    glib::Propagation::Proceed
                });
            }

            obj.add_controller(gesture_click);
            obj.add_controller(motion);
            obj.add_controller(scroll);
            obj.add_controller(key_controller);
        }

        fn apply_gtk_cursor(&self, icon: &egui::CursorIcon) {
            use gtk::gdk::Cursor;
            use std::cell::RefCell;

            thread_local! {
                static CURRENT: RefCell<Option<egui::CursorIcon>> = const { RefCell::new(None) };
            }

            let changed = CURRENT.with(|c| {
                let mut c = c.borrow_mut();
                if c.as_ref() == Some(icon) {
                    false
                } else {
                    *c = Some(*icon);
                    true
                }
            });

            if !changed {
                return;
            }

            let name: Option<&str> = match icon {
                egui::CursorIcon::Text => Some("text"),
                egui::CursorIcon::PointingHand => Some("pointer"),
                egui::CursorIcon::Crosshair => Some("crosshair"),
                egui::CursorIcon::Grab => Some("grab"),
                egui::CursorIcon::Grabbing => Some("grabbing"),
                egui::CursorIcon::ResizeHorizontal => Some("ew-resize"),
                egui::CursorIcon::ResizeVertical => Some("ns-resize"),
                egui::CursorIcon::ResizeNwSe => Some("nwse-resize"),
                egui::CursorIcon::ResizeNeSw => Some("nesw-resize"),
                egui::CursorIcon::Move => Some("move"),
                egui::CursorIcon::NotAllowed => Some("not-allowed"),
                egui::CursorIcon::Wait => Some("wait"),
                egui::CursorIcon::Progress => Some("progress"),
                egui::CursorIcon::Help => Some("help"),
                egui::CursorIcon::ContextMenu => Some("context-menu"),
                egui::CursorIcon::Cell => Some("cell"),
                egui::CursorIcon::VerticalText => Some("vertical-text"),
                egui::CursorIcon::Alias => Some("alias"),
                egui::CursorIcon::Copy => Some("copy"),
                egui::CursorIcon::NoDrop => Some("no-drop"),
                egui::CursorIcon::AllScroll => Some("all-scroll"),
                egui::CursorIcon::ZoomIn => Some("zoom-in"),
                egui::CursorIcon::ZoomOut => Some("zoom-out"),
                egui::CursorIcon::Default => None,
                _ => None,
            };

            let cursor = match name {
                Some(n) => Cursor::from_name(n, None),
                None => Cursor::from_name("default", None),
            };

            self.obj().set_cursor(cursor.as_ref());
        }
    }

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn gdk_to_egui_key(key: gdk::Key) -> Option<egui::Key> {
        use egui::Key as EKey;
        use gdk::Key as GKey;

        let k = match key {
            GKey::BackSpace => EKey::Backspace,
            GKey::Down => EKey::ArrowDown,
            GKey::Up => EKey::ArrowUp,
            GKey::Left => EKey::ArrowLeft,
            GKey::Right => EKey::ArrowRight,
            GKey::KP_Enter | GKey::ISO_Enter => EKey::Enter,
            GKey::space | GKey::KP_Space => EKey::Space,
            GKey::Page_Up => EKey::PageUp,
            GKey::Page_Down => EKey::PageDown,
            GKey::colon => EKey::Colon,
            GKey::comma => EKey::Comma,
            GKey::backslash => EKey::Backslash,
            GKey::slash => EKey::Slash,
            GKey::vertbar => EKey::Pipe,
            GKey::question => EKey::Questionmark,
            GKey::bracketleft => EKey::OpenBracket,
            GKey::braceright => EKey::CloseBracket,
            GKey::grave => EKey::Backtick,
            GKey::minus => EKey::Minus,
            GKey::period => EKey::Period,
            GKey::plus => EKey::Plus,
            GKey::equal => EKey::Equals,
            GKey::semicolon => EKey::Semicolon,
            GKey::singlelowquotemark => EKey::Quote,
            GKey::_0 | GKey::KP_0 => EKey::Num0,
            GKey::_1 | GKey::KP_1 => EKey::Num1,
            GKey::_2 | GKey::KP_2 => EKey::Num2,
            GKey::_3 | GKey::KP_3 => EKey::Num3,
            GKey::_4 | GKey::KP_4 => EKey::Num4,
            GKey::_5 | GKey::KP_5 => EKey::Num5,
            GKey::_6 | GKey::KP_6 => EKey::Num6,
            GKey::_7 | GKey::KP_7 => EKey::Num7,
            GKey::_8 | GKey::KP_8 => EKey::Num8,
            GKey::_9 | GKey::KP_9 => EKey::Num9,
            other => return other.name().and_then(|n| egui::Key::from_name(&n)),
        };
        Some(k)
    }

    fn gdk_to_egui_modifiers(modifiers: gdk::ModifierType) -> egui::Modifiers {
        use gdk::ModifierType;
        egui::Modifiers {
            alt: modifiers.contains(ModifierType::ALT_MASK),
            ctrl: modifiers.contains(ModifierType::CONTROL_MASK),
            shift: modifiers.contains(ModifierType::SHIFT_MASK),
            mac_cmd: modifiers.contains(ModifierType::META_MASK),
            #[cfg(target_os = "macos")]
            command: modifiers.contains(ModifierType::META_MASK),
            #[cfg(not(target_os = "macos"))]
            command: modifiers.contains(ModifierType::CONTROL_MASK),
        }
    }

    fn is_cut_command(modifiers: egui::Modifiers, key: egui::Key) -> bool {
        key == egui::Key::Cut
            || (modifiers.command && key == egui::Key::X)
            || (cfg!(target_os = "windows") && modifiers.shift && key == egui::Key::Delete)
    }

    fn is_copy_command(modifiers: egui::Modifiers, key: egui::Key) -> bool {
        key == egui::Key::Copy
            || (modifiers.command && key == egui::Key::C)
            || (cfg!(target_os = "windows") && modifiers.ctrl && key == egui::Key::Insert)
    }

    fn is_paste_command(modifiers: egui::Modifiers, key: egui::Key) -> bool {
        key == egui::Key::Paste
            || (modifiers.command && key == egui::Key::V)
            || (cfg!(target_os = "windows") && modifiers.shift && key == egui::Key::Insert)
    }

    fn init_epoxy() {
        static EPOXY_INIT: std::sync::Once = std::sync::Once::new();
        EPOXY_INIT.call_once(|| {
            #[cfg(target_os = "macos")]
            let library =
                unsafe { libloading::os::unix::Library::new("libepoxy.0.dylib").unwrap() };

            #[cfg(all(unix, not(target_os = "macos")))]
            let library =
                unsafe { libloading::os::unix::Library::new("libepoxy.so.0").unwrap() };

            #[cfg(windows)]
            let library = libloading::os::windows::Library::open_already_loaded("libepoxy-0.dll")
                .or_else(|_| libloading::os::windows::Library::open_already_loaded("epoxy-0.dll"))
                .unwrap();

            epoxy::load_with(|name| unsafe {
                library
                    .get::<_>(name.as_bytes())
                    .map(|sym| *sym)
                    .unwrap_or(std::ptr::null())
            });
        });
    }
}