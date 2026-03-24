use crate::{
    buffer::{self, Buffer},
    term::Key,
    ui::{Rect, draw_box, fill_rect},
};

use super::widget::Widget;

pub struct UIProgressBar {
    title: String,
    description: String,
    current: u64,
    max: u64,

    r: Rect,
    layout_cb: fn(u16, u16) -> Rect,
}

impl UIProgressBar {
    pub fn new(title: String, c: fn(u16, u16) -> Rect) -> Self {
        Self {
            title,
            description: String::new(),
            current: 0,
            max: 0,
            r: Rect::new(0, 0, 0, 0),
            layout_cb: c,
        }
    }

    pub fn update(&mut self, current: u64, max: u64, description: String) {
        self.current = current;
        self.max = max;
        self.description = description;
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }
}

impl Widget for UIProgressBar {
    fn handle_buf_size_change(&mut self, w: u16, h: u16) {
        self.r = (self.layout_cb)(w, h);
    }
    fn draw(&mut self, buffer: &mut Buffer, focused: bool) {
        if self.r.h == 0 || self.r.w == 0 {
            self.handle_buf_size_change(buffer.width, buffer.height);
        }

        let r = &self.r;

        let detail_string;
        let mut progress_line = String::new();

        if self.max > 0 {
            let bar_width = r.w.saturating_sub(4) as u64;
            let p = self.current * bar_width / self.max;
            progress_line = "█".repeat(p as usize);
            detail_string = format!("{}/{}", self.current, self.max);
        } else {
            detail_string = format!("{}", self.current);
        }

        let mut ri = r.clone();
        ri.x += 1;
        ri.y += 1;
        ri.w -= 2;
        ri.h -= 2;

        draw_box(buffer, &r, &self.title, focused);
        fill_rect(buffer, &ri, ' ', buffer::Color::Black, buffer::Color::Black);

        buffer.put_str(
            r.x + 2,
            r.y + 2,
            &detail_string,
            buffer::Color::White,
            buffer::Color::Black,
        );

        if !self.description.is_empty() {
            buffer.put_str(
                r.x + 2,
                r.y + 3,
                &self.description,
                buffer::Color::Yellow,
                buffer::Color::Black,
            );
        }

        if self.max > 0 {
            buffer.put_str(
                r.x + 2,
                r.y + 4,
                &progress_line,
                buffer::Color::Yellow,
                buffer::Color::Yellow,
            );
        }
    }

    fn handle_input(&mut self, _key: Key) {}
}
