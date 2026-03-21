
use crate::{
    buffer::{self, Buffer},
    term::Key,
    ui::{Rect, fill_rect},
};

use super::widget::Widget;

pub struct UIStatusBar {
    title: String,

    r: Rect,
    layout_cb: fn(u16, u16) -> Rect,
}

impl UIStatusBar {
    pub fn new(
        title: String,
        c: fn(u16, u16) -> Rect,
    ) -> Self {
        Self {
            title,
            r: Rect::new(0, 0, 0, 0),
            layout_cb: c,
        }
    }
}

impl Widget for UIStatusBar {
    fn handle_buf_size_change(&mut self, w: u16, h: u16) {
        self.r = (self.layout_cb)(w, h);
    }
    fn draw(&mut self, buffer: &mut Buffer, _focused: bool) {
        if self.r.h == 0 || self.r.w == 0 {
            self.handle_buf_size_change(buffer.width, buffer.height);
        }

        let s: String = self.title.chars().take(self.r.w as usize).collect();

        fill_rect(buffer, &self.r, ' ', buffer::Color::Black, buffer::Color::White);

        buffer.put_str(0, self.r.y, &s, buffer::Color::Black, buffer::Color::White);
    }

    fn handle_input(&mut self, _key: Key) {}
}
