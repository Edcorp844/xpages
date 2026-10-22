pub fn draw_selection(
    painter: &egui::Painter,
    page_rect: egui::Rect,
    selection_rects: &[(usize, wordcraft_geom::Rect)],
    page_index: usize,
    logical_scale: f32,
    accent_color: egui::Color32, // NEW: 6th argument
) {
    let sel_fill = accent_color.gamma_multiply(0.4);
    for (rpage, r) in selection_rects {
        if *rpage != page_index {
            continue;
        }
        let rr = egui::Rect::from_min_size(
            page_rect.min + egui::vec2(r.x * logical_scale, r.y * logical_scale),
            egui::vec2(r.w * logical_scale, r.h * logical_scale),
        );
        painter.rect_filled(rr, 0.0, sel_fill);
    }
}

pub fn draw_caret(
    painter: &egui::Painter,
    page_rect: egui::Rect,
    caret: Option<&wordcraft_layout::hit::Caret>,
    page_index: usize,
    logical_scale: f32,
) {
    if let Some(c) = caret {
        if c.page == page_index {
            let r = egui::Rect::from_min_size(
                page_rect.min + egui::vec2(c.x * logical_scale, c.top * logical_scale),
                egui::vec2(c.width.max(1.5), c.height * logical_scale),
            );
            painter.rect_filled(r, 0.0, egui::Color32::BLACK);
        }
    }
}
