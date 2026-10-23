// document_canvas.rs — show() with zoom + smooth page scrolling

use super::caret::{draw_caret, draw_selection};
use crate::state::DocumentCanvas;
use serde_json::json;
use std::time::Duration;

const PX_PER_PT: f32 = 96.0 / 72.0;
const PAGE_GAP: f32 = 24.0;
const RULER_SIZE: f32 = 12.0;
const RULER_GAP: f32 = 6.0;
const RULER_SAFETY_GAP: f32 = RULER_SIZE / 4.0;

const ZOOM_MIN: f32 = 0.25;
const ZOOM_MAX: f32 = 4.0;
const ZOOM_STEP: f32 = 1.1;

impl DocumentCanvas {
    pub fn set_zoom(&mut self, z: f32) {
        self.zoom = z.clamp(ZOOM_MIN, ZOOM_MAX);
    }

    #[allow(dead_code)]
    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom * ZOOM_STEP);
    }

    #[allow(dead_code)]
    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom / ZOOM_STEP);
    }

    #[allow(dead_code)]
    pub fn zoom_reset(&mut self) {
        self.set_zoom(1.0);
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        session: &mut wordcraft_engine::Session,
        scroll_to_page: &mut Option<usize>,
        focus_id: egui::Id,
    ) {
        let ppp = ui.ctx().pixels_per_point();

        if (self.last_ppp - ppp).abs() > 0.001 {
            self.last_ppp = ppp;
            if scroll_to_page.is_none() {
                *scroll_to_page = Some(session.page_hint);
            }
        }

        // ------------------------------------------------------------------
        // Zoom: pinch + Ctrl/Cmd+wheel (does NOT disable normal scrolling)
        // ------------------------------------------------------------------
        let mut zoom_changed = false;

        let zoom_delta = ui.input(|i| i.zoom_delta());
        if (zoom_delta - 1.0).abs() > 0.001 {
            self.set_zoom(self.zoom * zoom_delta);
            zoom_changed = true;
        }

        let (cmd, scroll_y) = ui.input(|i| {
            (i.modifiers.command, i.smooth_scroll_delta.y)
        });

        if cmd && scroll_y.abs() > 0.0 {
            let factor = if scroll_y > 0.0 {
                ZOOM_STEP
            } else {
                1.0 / ZOOM_STEP
            };
            self.set_zoom(self.zoom * factor);
            zoom_changed = true;

            // Consume scroll so the document does not pan while zooming
            ui.ctx().input_mut(|i| {
                i.smooth_scroll_delta = egui::Vec2::ZERO;
            });
        }

        if zoom_changed {
            self.textures.clear();
            ui.ctx().request_repaint();
        }

        let logical_scale = self.zoom * PX_PER_PT;

        // ------------------------------------------------------------------

        let rev = session.rev();
        let layout = session.layout();
        let target = scroll_to_page.take();

        self.caret_blink_tick();
        if self.editing {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
            ui.memory_mut(|m| m.request_focus(focus_id));
        }

        let caret_hint = session.page_hint;
        let caret = layout.caret_on(&session.sel.focus, caret_hint);

        let (sel_a, sel_b) = session.sel.ordered();
        let selection_rects = if session.sel.is_collapsed() {
            Vec::new()
        } else {
            layout.selection_rects(&session.doc, &sel_a, &sel_b, caret_hint)
        };

        let accent_color = crate::utils::theme::adwaita_accent_color();

        let window_rect = ui.available_rect_before_wrap();
        let mut first_page_rect = egui::Rect::NOTHING;
        let mut hovering_page = false;

        ui.style_mut().visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;

        let mask_height = RULER_SIZE + RULER_GAP + RULER_SAFETY_GAP;
        let (_ruler_area, scroll_area) =
            window_rect.split_top_bottom_at_y(window_rect.min.y + mask_height);

        let mut scroll_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(scroll_area)
                .layout(*ui.layout()),
        );

        // Smooth continuous scrolling — defaults enabled (no .scroll(false))
        egui::ScrollArea::both()
            .id_salt("doc_scroll")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .animated(true)
            .show(&mut scroll_ui, |ui| {
                ui.add_space(RULER_GAP);

                let max_side = ui.ctx().input(|i| i.max_texture_side) as f32;

                for (i, page) in layout.pages.iter().enumerate() {
                    // Desired physical size at current zoom
                    let mut phys_w = (page.w * logical_scale * ppp).ceil().max(1.0);
                    let mut phys_h = (page.h * logical_scale * ppp).ceil().max(1.0);

                    // Cap texture to GPU limit (prevents panic at high zoom)
                    let longest = phys_w.max(phys_h);
                    if longest > max_side {
                        let s = max_side / longest;
                        phys_w = (phys_w * s).floor().max(1.0);
                        phys_h = (phys_h * s).floor().max(1.0);
                    }

                    // On-screen size follows zoom (may upscale a capped texture)
                    let screen_w = (page.w * logical_scale * ppp).ceil() / ppp;
                    let screen_h = (page.h * logical_scale * ppp).ceil() / ppp;
                    let page_render_scale = phys_w / page.w;

                    let row_h = screen_h + PAGE_GAP;
                    let row_w = ui.available_width().max(screen_w + 64.0);

                    let (response, painter) = ui
                        .allocate_painter(egui::vec2(row_w, row_h), egui::Sense::click_and_drag());

                    if target == Some(i) {
                        response.scroll_to_me(Some(egui::Align::TOP));
                    }

                    let center_x = response.rect.center().x;
                    let top_y = response.rect.min.y;
                    let snap_x = ((center_x - screen_w * 0.5) * ppp).round() / ppp;
                    let snap_y = (top_y * ppp).round() / ppp;

                    let page_rect = egui::Rect::from_min_size(
                        egui::pos2(snap_x, snap_y),
                        egui::vec2(screen_w, screen_h),
                    );

                    if i == 0 {
                        first_page_rect = page_rect;
                    }

                    if let Some(pos) = ui.ctx().pointer_hover_pos() {
                        if page_rect.contains(pos) {
                            hovering_page = true;
                        }
                    }

                    // Click → caret
                    if response.clicked() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            if page_rect.contains(pos) {
                                self.editing = true;
                                ui.memory_mut(|m| m.request_focus(focus_id));
                                let local = (pos - page_rect.min) / logical_scale;
                                let story = layout
                                    .story_at(i, local.x, local.y)
                                    .unwrap_or(wordcraft_doc::StoryRef::Body);

                                if let Some(hit) = layout.hit(i, local.x, local.y, story) {
                                    let extend = ui.input(|m| m.modifiers.shift);
                                    let _ = session
                                        .run("caret.set", &json!({ "pos": hit, "extend": extend }));
                                    session.page_hint = i;
                                    session.close_typing();
                                    self.caret_blink_reset();
                                    ui.ctx().request_repaint();
                                }
                            }
                        }
                    }

                    // Drag → select
                    if response.dragged() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            if page_rect.contains(pos) {
                                self.editing = true;
                                ui.memory_mut(|m| m.request_focus(focus_id));
                                let local = (pos - page_rect.min) / logical_scale;
                                let story = layout
                                    .story_at(i, local.x, local.y)
                                    .unwrap_or(wordcraft_doc::StoryRef::Body);

                                if let Some(hit) = layout.hit(i, local.x, local.y, story) {
                                    let _ = session
                                        .run("caret.set", &json!({ "pos": hit, "extend": true }));
                                    session.page_hint = i;
                                    session.close_typing();
                                    self.caret_blink_reset();
                                    ui.ctx().request_repaint();
                                }
                            }
                        }
                    }

                    painter.rect_filled(
                        page_rect.translate(egui::vec2(3.0, 3.0)),
                        2.0,
                        egui::Color32::from_black_alpha(40),
                    );

                    let key = crate::utils::theme::texture_key(
                        layout.pages.len(),
                        page_render_scale,
                        rev,
                        ppp,
                    ) ^ ((i as u64) << 32);
                    let stale = self
                        .textures
                        .get(&i)
                        .map(|(k, _)| *k != key)
                        .unwrap_or(true);

                    if stale {
                        let img = wordcraft_render::render_page(
                            &session.doc,
                            page,
                            page_render_scale,
                            &wordcraft_render::RenderOptions::default(),
                        );
                        let color = egui::ColorImage::from_rgba_premultiplied(
                            [img.width as usize, img.height as usize],
                            &img.pixels,
                        );
                        match self.textures.get_mut(&i) {
                            Some((k, tex)) => {
                                tex.set(color, egui::TextureOptions::NEAREST);
                                *k = key;
                            }
                            None => {
                                let tex = ui.ctx().load_texture(
                                    format!("page{i}"),
                                    color,
                                    egui::TextureOptions::NEAREST,
                                );
                                self.textures.insert(i, (key, tex));
                            }
                        }
                    }

                    if let Some((_, tex)) = self.textures.get(&i) {
                        painter.image(
                            tex.id(),
                            page_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }

                    draw_selection(
                        &painter,
                        page_rect,
                        &selection_rects,
                        i,
                        logical_scale,
                        accent_color,
                    );

                    if self.editing && self.caret_visible {
                        draw_caret(&painter, page_rect, caret.as_ref(), i, logical_scale);
                    }

                    painter.rect_stroke(
                        page_rect,
                        0.0,
                        egui::Stroke::new(1.0, egui::Color32::from_black_alpha(50)),
                        egui::StrokeKind::Outside,
                    );
                }
            });

        // Sticky rulers
        if first_page_rect.is_positive() {
            let visual = ui.style().visuals.clone();
            let ruler_bg = if visual.dark_mode {
                egui::Color32::from_gray(40)
            } else {
                egui::Color32::from_rgb(243, 243, 243)
            };
            let ruler_fg = if visual.dark_mode {
                egui::Color32::from_gray(120)
            } else {
                egui::Color32::from_gray(180)
            };
            let ruler_text = if visual.dark_mode {
                egui::Color32::from_gray(200)
            } else {
                egui::Color32::from_gray(80)
            };
            let margin_highlight = if visual.dark_mode {
                egui::Color32::from_white_alpha(15)
            } else {
                egui::Color32::from_black_alpha(15)
            };

            let workspace_bg = visual.extreme_bg_color;

            let h_ruler_rect = egui::Rect::from_min_size(
                egui::pos2(first_page_rect.min.x, window_rect.min.y),
                egui::vec2(first_page_rect.width(), RULER_SIZE),
            );

            let v_ruler_rect = egui::Rect::from_min_size(
                egui::pos2(window_rect.min.x, first_page_rect.min.y),
                egui::vec2(RULER_SIZE, first_page_rect.height()),
            );

            let h_painter = ui.painter().with_clip_rect(h_ruler_rect);
            let v_painter = ui.painter().with_clip_rect(v_ruler_rect);

            let mask_rect = egui::Rect::from_min_size(
                egui::pos2(first_page_rect.min.x, window_rect.min.y + RULER_SIZE),
                egui::vec2(first_page_rect.width(), RULER_GAP + RULER_SAFETY_GAP),
            );
            ui.painter().rect_filled(mask_rect, 0.0, workspace_bg);

            h_painter.rect_filled(h_ruler_rect, 0.0, ruler_bg);

            let page_w = page_width_for_ruler(&layout);
            let base_x = first_page_rect.min.x;

            let margin_left = 72.0 * logical_scale;
            let margin_right = page_w - (72.0 * logical_scale);

            let text_area_start = base_x + margin_left;
            let text_area_end = base_x + margin_right;

            if text_area_start > h_ruler_rect.min.x {
                h_painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(base_x.max(h_ruler_rect.min.x), h_ruler_rect.min.y),
                        egui::pos2(text_area_start.min(h_ruler_rect.max.x), h_ruler_rect.max.y),
                    ),
                    0.0,
                    margin_highlight,
                );
            }
            if text_area_end < h_ruler_rect.max.x {
                h_painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(text_area_end.max(h_ruler_rect.min.x), h_ruler_rect.min.y),
                        egui::pos2(
                            (base_x + page_w).min(h_ruler_rect.max.x),
                            h_ruler_rect.max.y,
                        ),
                    ),
                    0.0,
                    margin_highlight,
                );
            }

            let step = 9.0 * logical_scale;
            let mut x = base_x;
            while x < h_ruler_rect.max.x {
                if x >= h_ruler_rect.min.x {
                    let dist = x - text_area_start;
                    let tick_idx = (dist / step).round() as i32;
                    let is_inch = tick_idx % 8 == 0;

                    let height = if is_inch { 8.0 } else { 3.0 };
                    h_painter.line_segment(
                        [
                            egui::pos2(x, h_ruler_rect.max.y - height),
                            egui::pos2(x, h_ruler_rect.max.y),
                        ],
                        egui::Stroke::new(1.0, ruler_fg),
                    );

                    if is_inch && x >= text_area_start && x <= text_area_end {
                        let label = format!("{}", tick_idx / 8);
                        h_painter.text(
                            egui::pos2(x, h_ruler_rect.center().y + 1.0),
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::proportional(9.0),
                            ruler_text,
                        );
                    }
                }
                x += step;
            }

            let draw_tri = |painter: &egui::Painter, center: egui::Pos2, dir: f32| {
                let mut points = vec![
                    center + egui::vec2(0.0, 3.0 * dir),
                    center + egui::vec2(-3.0, -1.0 * dir),
                    center + egui::vec2(3.0, -1.0 * dir),
                ];
                if dir < 0.0 {
                    points.reverse();
                }
                painter.add(egui::Shape::convex_polygon(
                    points,
                    visual.text_color(),
                    egui::Stroke::NONE,
                ));
            };

            let tri_y = h_ruler_rect.max.y - 2.0;
            if text_area_start >= h_ruler_rect.min.x && text_area_start <= h_ruler_rect.max.x {
                draw_tri(&h_painter, egui::pos2(text_area_start, tri_y), 1.0);
            }
            if text_area_end >= h_ruler_rect.min.x && text_area_end <= h_ruler_rect.max.x {
                draw_tri(&h_painter, egui::pos2(text_area_end, tri_y), 1.0);
            }

            v_painter.rect_filled(v_ruler_rect, 0.0, ruler_bg);

            let base_y = first_page_rect.min.y;
            let margin_top = 72.0 * logical_scale;
            let text_area_start_y = base_y + margin_top;

            if text_area_start_y > v_ruler_rect.min.y {
                v_painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(v_ruler_rect.min.x, base_y.max(v_ruler_rect.min.y)),
                        egui::pos2(
                            v_ruler_rect.max.x,
                            text_area_start_y.min(v_ruler_rect.max.y),
                        ),
                    ),
                    0.0,
                    margin_highlight,
                );
            }

            let mut y = base_y;
            while y < v_ruler_rect.max.y {
                if y >= v_ruler_rect.min.y {
                    let dist = y - text_area_start_y;
                    let tick_idx = (dist / step).round() as i32;
                    let is_inch = tick_idx % 8 == 0;

                    let width = if is_inch { 8.0 } else { 3.0 };
                    v_painter.line_segment(
                        [
                            egui::pos2(v_ruler_rect.max.x - width, y),
                            egui::pos2(v_ruler_rect.max.x, y),
                        ],
                        egui::Stroke::new(1.0, ruler_fg),
                    );

                    if is_inch && y >= text_area_start_y {
                        let label = format!("{}", tick_idx / 8);
                        v_painter.text(
                            egui::pos2(v_ruler_rect.center().x, y),
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::proportional(9.0),
                            ruler_text,
                        );
                    }
                }
                y += step;
            }

            let center = egui::pos2(v_ruler_rect.max.x - 2.0, text_area_start_y);
            let points = vec![
                center + egui::vec2(1.0, 0.0),
                center + egui::vec2(-3.0, -3.0),
                center + egui::vec2(-3.0, 4.0),
            ];
            v_painter.add(egui::Shape::convex_polygon(
                points,
                visual.text_color(),
                egui::Stroke::NONE,
            ));
        }

        if hovering_page {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        } else {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
        }

        let n = layout.pages.len();
        self.textures.retain(|i, _| *i < n);
    }
}

fn page_width_for_ruler(layout: &wordcraft_layout::DocLayout) -> f32 {
    if let Some(p) = layout.pages.first() {
        p.w * PX_PER_PT
    } else {
        600.0
    }
}