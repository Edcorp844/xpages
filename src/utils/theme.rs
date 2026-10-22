use relm4::adw;

pub fn adwaita_is_dark() -> bool {
    adw::StyleManager::default().is_dark()
}

pub fn transparent_visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let clear = egui::Color32::TRANSPARENT;

    v.panel_fill = clear;
    v.window_fill = clear;
    v.extreme_bg_color = clear;
    v.faint_bg_color = clear;
    v.code_bg_color = clear;

    v.widgets.noninteractive.bg_fill = clear;
    v.widgets.inactive.bg_fill = clear;

    v
}

pub fn texture_key(pages: usize, scale_px: f32, rev: u64, ppp: f32) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    mix(&(pages as u64).to_le_bytes());
    mix(&scale_px.to_le_bytes());
    mix(&rev.to_le_bytes());
    mix(&ppp.to_le_bytes());
    h
}

/// Reads the current libadwaita `@accent_bg_color` from the GTK theme.
/// Falls back to the standard Adwaita blue if not found.
#[allow(deprecated)]
pub fn adwaita_accent_color() -> egui::Color32 {
    use gtk::prelude::*;

    let widget = gtk::Label::new(None);
    if let Some(rgba) = widget.style_context().lookup_color("accent_bg_color") {
        return egui::Color32::from_rgba_unmultiplied(
            (rgba.red() * 255.0).round() as u8,
            (rgba.green() * 255.0).round() as u8,
            (rgba.blue() * 255.0).round() as u8,
            (rgba.alpha() * 255.0).round() as u8,
        );
    }

    // Fallback: standard Adwaita blue
    egui::Color32::from_rgb(0x35, 0x84, 0xe4)
}
