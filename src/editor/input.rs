use serde_json::json;
use crate::state::DocumentCanvas;

pub fn handle_editing(
    ui: &mut egui::Ui,
    session: &mut wordcraft_engine::Session,
    canvas: &mut DocumentCanvas,
    _focus_id: egui::Id,
) -> bool {
    if !canvas.editing {
        return false;
    }

    let mut edited = false;
    let mut caret_moved = false;
    let events: Vec<_> = ui.input(|i| i.events.clone());

    for ev in events {
        match ev {
            egui::Event::Text(t) if !t.is_empty() => {
                match session.run("text.insert", &json!({ "text": t })) {
                    Ok(_) => edited = true,
                    Err(e) => eprintln!("text.insert: {e:?}"),
                }
            }
            egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                match session.run("text.backspace", &json!({})) {
                    Ok(_) => edited = true,
                    Err(e) => eprintln!("backspace: {e:?}"),
                }
            }
            egui::Event::Key { key: egui::Key::Delete, pressed: true, .. } => {
                match session.run("text.delete", &json!({})) {
                    Ok(_) => edited = true,
                    Err(e) => eprintln!("delete: {e:?}"),
                }
            }
            egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. } => {
                let id = if modifiers.shift { "text.lineBreak" } else { "text.newParagraph" };
                match session.run(id, &json!({})) {
                    Ok(_) => edited = true,
                    Err(e) => eprintln!("{id}: {e:?}"),
                }
            }
            egui::Event::Key { key: egui::Key::ArrowLeft, pressed: true, modifiers, .. } => {
                caret_moved = true;
                let _ = session.run("caret.left", &json!({ "extend": modifiers.shift }));
                ui.ctx().request_repaint();
            }
            egui::Event::Key { key: egui::Key::ArrowRight, pressed: true, modifiers, .. } => {
                caret_moved = true;
                let _ = session.run("caret.right", &json!({ "extend": modifiers.shift }));
                ui.ctx().request_repaint();
            }
            egui::Event::Key { key: egui::Key::ArrowUp, pressed: true, modifiers, .. } => {
                caret_moved = true;
                let _ = session.run("caret.up", &json!({ "extend": modifiers.shift }));
                ui.ctx().request_repaint();
            }
            egui::Event::Key { key: egui::Key::ArrowDown, pressed: true, modifiers, .. } => {
                caret_moved = true;
                let _ = session.run("caret.down", &json!({ "extend": modifiers.shift }));
                ui.ctx().request_repaint();
            }
            egui::Event::Key { key: egui::Key::Home, pressed: true, modifiers, .. } => {
                caret_moved = true;
                let id = if modifiers.ctrl { "caret.docStart" } else { "caret.home" };
                let _ = session.run(id, &json!({ "extend": modifiers.shift }));
                ui.ctx().request_repaint();
            }
            egui::Event::Key { key: egui::Key::End, pressed: true, modifiers, .. } => {
                caret_moved = true;
                let id = if modifiers.ctrl { "caret.docEnd" } else { "caret.end" };
                let _ = session.run(id, &json!({ "extend": modifiers.shift }));
                ui.ctx().request_repaint();
            }
            _ => {}
        }
    }

    if edited || caret_moved {
        canvas.caret_blink_reset();
    }
    edited
}