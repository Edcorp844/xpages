use gtk::prelude::*;
use relm4::RelmWidgetExt;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct TocEntry {
    pub title: String,
    /// 1 = Title / Heading 1, 2 = Heading 2 / Subtitle, 3 = Heading 3, 4 = Caption / H4
    pub level: u8,
    pub page_index: usize,
}

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

pub fn rebuild_toc_list(list: &gtk::ListBox, session: &Rc<RefCell<wordcraft_engine::Session>>) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }

    let entries = {
        let mut sess = session.borrow_mut();
        extract_toc(&mut sess)
    };

    if entries.is_empty() {
        let row = gtk::ListBoxRow::new();
        row.set_sensitive(false);
        let label = gtk::Label::new(Some("No headings in this document"));
        label.add_css_class("dim-label");
        label.set_margin_all(12);
        label.set_wrap(true);
        row.set_child(Some(&label));
        list.append(&row);
        return;
    }

    const BASE: i32 = 10;
    const STEP: i32 = 14;

    for entry in &entries {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(true);
        // Page index for GoToPage on activate
        row.set_widget_name(&entry.page_index.to_string());

        let level = entry.level.max(1);
        let indent = BASE + (level as i32 - 1) * STEP;

        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row_box.set_margin_start(indent);
        row_box.set_margin_end(8);
        row_box.set_margin_top(5);
        row_box.set_margin_bottom(5);

        if level > 1 {
            let bullet = gtk::Label::new(Some("•"));
            bullet.add_css_class("dim-label");
            row_box.append(&bullet);
        }

        let label = gtk::Label::new(Some(&entry.title));
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_max_width_chars(22);
        label.set_hexpand(true);
        match level {
            1 => label.add_css_class("heading"),
            2 => {}
            _ => label.add_css_class("caption"),
        }
        row_box.append(&label);

        row.set_child(Some(&row_box));
        list.append(&row);
    }
}

/// Map style name → outline depth. Body text returns None.
fn outline_level(style: &str) -> Option<u8> {
    let s = style.to_ascii_lowercase().replace(['-', '_'], " ");
    let s = s.trim();

    if s == "title" || s.contains("heading 1") || s == "heading1" || s == "h1" {
        Some(1)
    } else if s == "subtitle" || s.contains("heading 2") || s == "heading2" || s == "h2" {
        Some(2)
    } else if s.contains("heading 3") || s == "heading3" || s == "h3" {
        Some(3)
    } else if s == "caption" || s.contains("heading 4") || s == "heading4" || s == "h4" {
        Some(4)
    } else {
        None
    }
}

/// Build TOC from the open document.
///
/// Adjust the walk to match `wordcraft_doc` once you know the real fields.
/// Until then this returns an empty list (sidebar shows the empty state).
fn extract_toc(session: &mut wordcraft_engine::Session) -> Vec<TocEntry> {
    let mut out = Vec::new();
    let _layout = session.layout();
    let _doc = &session.doc;

    // Example once API is known:
    //
    // for block in doc.blocks() {
    //     let Some(level) = outline_level(block.style_name()) else { continue };
    //     let title = block.plain_text().trim().to_string();
    //     if title.is_empty() { continue; }
    //     let page_index = layout.page_index_of(block.start()).unwrap_or(0);
    //     out.push(TocEntry { title, level, page_index });
    // }

    let _ = outline_level; // keep helper used when you wire the walk
    let _ = session;
    out
}

fn page_to_texture(
    session: &mut wordcraft_engine::Session,
    page_index: usize,
    scale: f32,
) -> Option<gtk::gdk::MemoryTexture> {
    let layout = session.layout();
    let page = layout.pages.get(page_index)?;
    let img = wordcraft_render::render_page(
        &session.doc,
        page,
        scale,
        &wordcraft_render::RenderOptions::default(),
    );
    if img.width == 0 || img.height == 0 {
        return None;
    }

    let bytes = gtk::glib::Bytes::from(img.pixels.as_slice());
    Some(gtk::gdk::MemoryTexture::new(
        img.width as i32,
        img.height as i32,
        gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied,
        &bytes,
        (img.width as usize) * 4,
    ))
}
