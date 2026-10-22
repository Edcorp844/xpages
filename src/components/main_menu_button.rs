use adw::prelude::*;
use relm4::{Component, prelude::*};

pub struct MainMenuButton;

#[relm4::component(pub)]
impl Component for MainMenuButton {
    type Init = ();
    type Input = ();
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::MenuButton {
            set_icon_name: "open-menu-symbolic",
            set_tooltip_text: Some("Main Menu"),
            add_css_class: "flat",

            #[wrap(Some)]
            set_popover = &gtk::PopoverMenu::from_model(Some(&{
                let menu = gtk::gio::Menu::new();

                let global_section = gtk::gio::Menu::new();
                let new_item = gtk::gio::MenuItem::new(Some("New"), Some("app.new"));
                new_item.set_attribute_value("accel", Some(&"<Primary>N".to_variant()));
                menu.append_section(None, &global_section);

                let window_section = gtk::gio::Menu::new();

                let open_item = gtk::gio::MenuItem::new(Some("Open"), Some("app.open"));
                open_item.set_attribute_value("accel", Some(&"<Primary>O".to_variant()));
                window_section.append_item(&open_item);
                menu.append_section(None, &window_section);

                let save_section = gtk::gio::Menu::new();

                let save_item = gtk::gio::MenuItem::new(Some("Save"), Some("app.save"));
                save_item.set_attribute_value("accel", Some(&"<Primary>S".to_variant()));

                let save_as_item = gtk::gio::MenuItem::new(Some("Save As"), Some("app.save_as"));
                save_as_item.set_attribute_value("accel", Some(&"<Primary><Shift>S".to_variant()));

                save_section.append_item(&save_item);
                save_section.append_item(&save_as_item);
                menu.append_section(None, &save_section);

                let document_section = gtk::gio::Menu::new();

                let print_item = gtk::gio::MenuItem::new(Some("Print"), Some("app.print"));
                print_item.set_attribute_value("accel", Some(&"<Primary>P".to_variant()));

                let export_item = gtk::gio::MenuItem::new(Some("Export"), Some("app.export"));
                export_item.set_attribute_value("accel", Some(&"<Primary>E".to_variant()));

                document_section.append_item(&print_item);
                document_section.append_item(&export_item);
                menu.append_section(None, &document_section);


                let app_section = gtk::gio::Menu::new();

                let prefs_item = gtk::gio::MenuItem::new(Some("Preferences"), Some("app.preferences"));
                prefs_item.set_attribute_value("accel", Some(&"<Primary>comma".to_variant()));
                app_section.append_item(&prefs_item);

                let shortcuts_item = gtk::gio::MenuItem::new(Some("Keyboard Shortcuts"), Some("app.shortcuts"));
                shortcuts_item.set_attribute_value("accel", Some(&"<Primary>question".to_variant()));
                app_section.append_item(&shortcuts_item);

                app_section.append(Some("About XPodcasts"), Some("app.about"));

                let help_item = gtk::gio::MenuItem::new(Some("Help"), Some("app.help"));
                help_item.set_attribute_value("accel", Some(&"F1".to_variant()));
                app_section.append_item(&help_item);

                menu.append_section(None, &app_section);

                let quit_window_section = gtk::gio::Menu::new();
                let quit_window_item = gtk::gio::MenuItem::new(Some("Quit"), Some("app.quit"));
                quit_window_item.set_attribute_value("accel", Some(&"<Primary>Q".to_variant()));
                quit_window_section.append_item(&quit_window_item);
                menu.append_section(None, &quit_window_section);

                menu
            })) {}
        },
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = MainMenuButton {};

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }
}
