use std::time::Duration;
use serde_json::json;
use crate::state::DocumentCanvas;
use super::caret::{draw_caret, draw_selection};

const PX_PER_PT: f32 = 96.0 / 72.0;
const PAGE_GAP: f32 = 24.0;

impl DocumentCanvas {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        session: &mut wordcraft_engine::Session,
        zoom: f32,
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

        let rev = session.rev();
        let layout = session.layout();
        let logical_scale = zoom * PX_PER_PT;
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

        egui::ScrollArea::both()
            .id_salt("doc_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                for (i, page) in layout.pages.iter().enumerate() {
                    let phys_w = (page.w * logical_scale * ppp).ceil();
                    let phys_h = (page.h * logical_scale * ppp).ceil();
                    let screen_w = phys_w / ppp;
                    let screen_h = phys_h / ppp;
                    let page_render_scale = phys_w / page.w;

                    let row_h = screen_h + PAGE_GAP;
                    let row_w = ui.available_width().max(screen_w + 64.0);

                    let (response, painter) = ui.allocate_painter(
                        egui::vec2(row_w, row_h),
                        egui::Sense::click_and_drag(),
                    );

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

                    if ui.ctx().pointer_hover_pos().map_or(false, |p| page_rect.contains(p)) {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                    }

                    if response.clicked() || response.drag_started() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            if page_rect.contains(pos) {
                                self.editing = true;
                                ui.memory_mut(|m| m.request_focus(focus_id));
                                let local = (pos - page_rect.min) / logical_scale;
                                let story = layout.story_at(i, local.x, local.y).unwrap_or(wordcraft_doc::StoryRef::Body);
                                
                                if let Some(hit) = layout.hit(i, local.x, local.y, story) {
                                    let extend = ui.input(|m| m.modifiers.shift);
                                    let _ = session.run("caret.set", &json!({ "pos": hit, "extend": extend }));
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

                    let key = crate::utils::theme::texture_key(layout.pages.len(), page_render_scale, rev, ppp) ^ ((i as u64) << 32);
                    let stale = self.textures.get(&i).map(|(k, _)| *k != key).unwrap_or(true);

                    if stale {
                        let img = wordcraft_render::render_page(&session.doc, page, page_render_scale, &wordcraft_render::RenderOptions::default());
                        let color = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
                        match self.textures.get_mut(&i) {
                            Some((k, tex)) => {
                                tex.set(color, egui::TextureOptions::NEAREST);
                                *k = key;
                            }
                            None => {
                                let tex = ui.ctx().load_texture(format!("page{i}"), color, egui::TextureOptions::NEAREST);
                                self.textures.insert(i, (key, tex));
                            }
                        }
                    }

                    if let Some((_, tex)) = self.textures.get(&i) {
                        painter.image(tex.id(), page_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                    }

                    draw_selection(&painter, page_rect, &selection_rects, i, logical_scale);
                    
                    if self.editing && self.caret_visible {
                        draw_caret(&painter, page_rect, caret.as_ref(), i, logical_scale);
                    }

                    painter.rect_stroke(page_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::from_black_alpha(50)), egui::StrokeKind::Outside);
                }
            });

        let n = layout.pages.len();
        self.textures.retain(|i, _| *i < n);
    }
}