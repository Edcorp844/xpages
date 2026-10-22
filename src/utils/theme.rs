use relm4::adw;

pub fn adwaita_is_dark() -> bool {
    adw::StyleManager::default().is_dark()
}

pub fn transparent_visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
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