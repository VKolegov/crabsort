use std::sync::{Arc, Mutex};

use crate::{
    buffer::{self, Buffer},
    term::Key,
    ui::{Rect, draw_box, fill_rect},
};

use super::widget::Widget;

pub struct UIProgressBar {
    title: Arc<Mutex<String>>,
    description: Option<Arc<Mutex<String>>>,
    current: Arc<Mutex<u64>>,
    max: Arc<Mutex<u64>>,

    r: Rect,
    layout_cb: fn(u16, u16) -> Rect,
}

impl UIProgressBar {
    pub fn new(
        title: Arc<Mutex<String>>,
        desc: Option<Arc<Mutex<String>>>,
        current: Arc<Mutex<u64>>,
        max: Arc<Mutex<u64>>,
        c: fn(u16, u16) -> Rect,
    ) -> Self {
        Self {
            title,
            description: desc,
            current,
            max,
            r: Rect::new(0, 0, 0, 0),
            layout_cb: c,
        }
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

        let current_val = *self.current.lock().unwrap(); // safe: lock is short-lived, read-only
        let max_val = *self.max.lock().unwrap(); // safe: same as above

        let detail_string;
        let mut progress_line = String::new();


        if max_val > 0 {
            let bar_width = r.w.saturating_sub(4) as u64;
            let p = current_val * bar_width / max_val;
            progress_line = "█".repeat(p as usize);
            detail_string = format!("{}/{}", current_val, max_val);
        } else { 
            detail_string = format!("{}", current_val);
        }

        let mut ri = r.clone();
        ri.x += 1;
        ri.y += 1;
        ri.w -= 2;
        ri.h -= 2;

        let title = self.title.clone();

        draw_box(buffer, &r, &*title.lock().unwrap(), focused); // safe: lock is short-lived, read-only
        fill_rect(buffer, &ri, ' ', buffer::Color::Black, buffer::Color::Black);

        buffer.put_str(
            r.x + 2,
            r.y + 2,
            &detail_string,
            buffer::Color::White,
            buffer::Color::Black,
        );


        if let Some(desc) = &self.description {
            buffer.put_str(
                r.x + 2,
                r.y + 3,
                &*desc.lock().unwrap(), // safe: lock is short-lived, read-only
                buffer::Color::Yellow,
                buffer::Color::Black,
            );
        }

        if max_val > 0 {
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
