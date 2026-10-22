use adw::prelude::*;
//use gettextrs::{gettext, ngettext};
use relm4::prelude::*;

//use crate::{config, settings::GenaralSettings, util::external_controls::ExternalControlsMode};

pub struct AppMenu {}

impl AppMenu {
    pub fn register() {
        let app = relm4::main_application();

        let new_action = gtk::gio::SimpleAction::new("new", None);
        new_action.connect_activate(move |_, _| {
            println!("new requested");
        });
        app.add_action(&new_action);
        //app.set_accels_for_action("app.new", &["<Primary>n"]);

        let open_action = gtk::gio::SimpleAction::new("open", None);
        open_action.connect_activate(move |_, _| {
            println!("open requested");
        });
        app.add_action(&open_action);
        app.set_accels_for_action("app.open", &["<Primary><Shift>r"]);

        let save_action = gtk::gio::SimpleAction::new("open", None);
        save_action.connect_activate(move |_, _| {
            println!("save requested");
        });
        app.add_action(&save_action);
        app.set_accels_for_action("app.save", &["<Primary>s"]);

        let save_as_action = gtk::gio::SimpleAction::new("open", None);
        save_as_action.connect_activate(move |_, _| {
            println!("saveas requested");
        });
        app.add_action(&save_as_action);
        app.set_accels_for_action("app.save_as", &["<Primary><Shift>s"]);

        let print_action = gtk::gio::SimpleAction::new("open", None);
        print_action.connect_activate(move |_, _| {
            println!("print requested");
        });
        app.add_action(&print_action);
        app.set_accels_for_action("app.print", &["<Primary>p"]);

        let export_action = gtk::gio::SimpleAction::new("open", None);
        export_action.connect_activate(move |_, _| {
            println!("export requested");
        });
        app.add_action(&export_action);
        app.set_accels_for_action("app.export", &["<Primary>e"]);

        let preferences_action = gtk::gio::SimpleAction::new("preferences", None);
        preferences_action.connect_activate(move |_, _| {
            Self::show_preferences_window();
        });

        app.add_action(&preferences_action);
        app.set_accels_for_action("app.preferences", &["<Primary>comma"]);

        let shortcuts_action = gtk::gio::SimpleAction::new("shortcuts", None);
        shortcuts_action.connect_activate(move |_, _| {
            Self::show_shortcuts_window();
        });
        app.add_action(&shortcuts_action);
        app.set_accels_for_action("app.shortcuts", &["<Primary>question"]);

        let about_action = gtk::gio::SimpleAction::new("about", None);
        about_action.connect_activate(move |_, _| {
            Self::show_about_window();
        });
        app.add_action(&about_action);

        let help_action = gtk::gio::SimpleAction::new("about", None);
        help_action.connect_activate(move |_, _| {
            Self::show_about_window();
        });
        app.add_action(&help_action);

        let quit_action = gtk::gio::SimpleAction::new("quit", None);
        let app_clone = app.clone();
        quit_action.connect_activate(move |_, _| {
            app_clone.quit();
        });
        app.add_action(&quit_action);
        app.set_accels_for_action("app.quit", &["<Primary>q"]);
    }

    pub(crate) fn show_about_window() {
        if let Some(active_window) = relm4::main_application().active_window() {
            let about = adw::AboutDialog::builder()
                .application_name("XPages")
                .application_icon("github.io.edcorp844.xpages")
                .version("0.0.1")
                .developer_name("Edson Frost")
                .website("https://github.com/Edcorp844/gnome_podcasts.git")
                .issue_url("https://github.com/Edcorp844/gnome_podcasts/issues")
                .copyright("© 2026 Edson Frost")
                .license_type(gtk::License::Gpl30)
                .developers(vec!["Frost Edson"])
                .artists(vec!["Frost Edson"])
                .build();

            about.present(Some(&active_window));
        }
    }

    pub(crate) fn show_shortcuts_window() {
        if let Some(active_window) = relm4::main_application().active_window() {
            let shortcuts_window = adw::ShortcutsDialog::builder()
                .title("Keyboard Shortcuts")
                .width_request(600)
                .height_request(500)
                .build();

            // --- SECTION: Window ---
            let window_section = adw::ShortcutsSection::new(Some("Window"));

            let refresh = adw::ShortcutsItem::new("Refresh", "<Primary>r");
            refresh.set_subtitle("Refresh all views content");

            let refresh_episodes = adw::ShortcutsItem::new("Refresh Episodes", "<Primary><Shift>r");
            refresh_episodes.set_subtitle("Refresh the database and new episodes");
            window_section.add(refresh);
            window_section.add(refresh_episodes);

            let quit = adw::ShortcutsItem::new("Quit", "<Primary>q");
            quit.set_subtitle("Close the application");
            window_section.add(quit);

            // --- SECTION: Application ---
            let application_section = adw::ShortcutsSection::new(Some("Application"));

            let prefs = adw::ShortcutsItem::new("Preferences", "<Primary>comma");
            prefs.set_subtitle("Configure application preferences");
            application_section.add(prefs);

            let shorts = adw::ShortcutsItem::new("Shortcuts", "<Primary>question");
            shorts.set_subtitle("Shows shortcuts window");
            application_section.add(shorts);

            // --- SECTION: Navigation ---
            let nav_section = adw::ShortcutsSection::new(Some("Navigation"));

            let search = adw::ShortcutsItem::new("Search", "<Primary>f");
            search.set_subtitle("Search Podcasts, Episodes, Shows");
            nav_section.add(search);

            // Add sections to the dialog
            shortcuts_window.add(window_section);
            shortcuts_window.add(application_section);
            shortcuts_window.add(nav_section);

            shortcuts_window.present(Some(&active_window));
        }
    }

    pub(crate) fn show_preferences_window() {
        if let Some(active_window) = relm4::main_application().active_window() {
            let preferences_window = adw::PreferencesDialog::builder()
                .search_enabled(true)
                .build();

            preferences_window.present(Some(&active_window));
        }
    }
}
