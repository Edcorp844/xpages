use std::cell::RefCell;
use std::rc::Rc;
use gtk::prelude::*;

pub fn rebuild_thumb_list(
    list: &gtk::ListBox,
    session: &Rc<RefCell<wordcraft_engine::Session>>,
    page_count: usize,
) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }

    for i in 0..page_count {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(true);

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 4);
        vbox.set_margin_top(8);
        vbox.set_margin_bottom(8);
        vbox.set_margin_start(8);
        vbox.set_margin_end(8);
        vbox.set_halign(gtk::Align::Center);

        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_can_shrink(true);
        picture.set_size_request(100, 130);

        {
            let mut sess = session.borrow_mut();
            if let Some(tex) = page_to_texture(&mut sess, i, 0.12) {
                picture.set_paintable(Some(&tex));
            }
        }

        let label = gtk::Label::new(Some(&format!("Page {}", i + 1)));
        label.add_css_class("caption");

        vbox.append(&picture);
        vbox.append(&label);
        row.set_child(Some(&vbox));
        list.append(&row);
    }
}

fn page_to_texture(
    session: &mut wordcraft_engine::Session,
    page_index: usize,
    scale: f32,
) -> Option<gtk::gdk::MemoryTexture> {
    let layout = session.layout();
    let page = layout.pages.get(page_index)?;
    let img = wordcraft_render::render_page(&session.doc, page, scale, &wordcraft_render::RenderOptions::default());
    if img.width == 0 || img.height == 0 { return None; }

    let bytes = gtk::glib::Bytes::from(img.pixels.as_slice());
    Some(gtk::gdk::MemoryTexture::new(
        img.width as i32,
        img.height as i32,
        gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied,
        &bytes,
        (img.width as usize) * 4,
    ))
}