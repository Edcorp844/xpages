use std::collections::HashMap;
use std::time::Instant;

pub struct DocumentCanvas {
    pub textures: HashMap<usize, (u64, egui::TextureHandle)>,
    pub generation: u64,
    pub editing: bool,
    pub caret_visible: bool,
    pub last_blink: Instant,
    pub last_ppp: f32,
}

impl Default for DocumentCanvas {
    fn default() -> Self {
        Self {
            textures: HashMap::new(),
            generation: 0,
            editing: false,
            caret_visible: true,
            last_blink: Instant::now(),
            last_ppp: 1.0,
        }
    }
}

impl DocumentCanvas {
    pub fn clear(&mut self) {
        self.textures.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn caret_blink_reset(&mut self) {
        self.caret_visible = true;
        self.last_blink = Instant::now();
    }

    pub fn caret_blink_tick(&mut self) {
        if self.last_blink.elapsed() >= std::time::Duration::from_millis(500) {
            self.caret_visible = !self.caret_visible;
            self.last_blink = Instant::now();
        }
    }
}
