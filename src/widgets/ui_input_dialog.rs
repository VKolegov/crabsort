use crate::{
    buffer::Buffer,
    event_bus::EventBus,
    term::Key,
    ui::{Rect, draw_box, fill_rect},
};

use super::widget::Widget;

pub struct UIInputDialog<F>
where
    F: Fn(u16, u16) -> Rect,
{
    id: &'static str,
    title: String,
    bus: EventBus,

    input: String,

    r: Rect,
    layout_cb: F,
}

impl<F> UIInputDialog<F>
where
    F: Fn(u16, u16) -> Rect,
{
    pub fn new(id: &'static str, title: String, default: Option<String>, bus: EventBus, c: F) -> Self {
        Self {
            id,
            title,
            bus,
            input: match default {
                Some(d) => d,
                None => String::new(),
            },
            r: Rect::new(0, 0, 0, 0),
            layout_cb: c,
        }
    }
}

impl<F> Widget for UIInputDialog<F>
where
    F: Fn(u16, u16) -> Rect,
{
    fn handle_buf_size_change(&mut self, w: u16, h: u16) {
        self.r = (self.layout_cb)(w, h);
    }
    fn draw(&mut self, buffer: &mut Buffer, focused: bool) {
        if self.r.h == 0 || self.r.w == 0 {
            self.handle_buf_size_change(buffer.width, buffer.height);
        }
        let r = &self.r;
        draw_box(buffer, r, &self.title, focused);

        let ri = Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2);
        fill_rect(
            buffer,
            &ri,
            ' ',
            crate::buffer::Color::Black,
            crate::buffer::Color::Black,
        );

        buffer.put_str(
            r.x + 2,
            r.y + 2,
            &self.input,
            crate::buffer::Color::White,
            crate::buffer::Color::Black,
        );
    }

    fn handle_input(&mut self, key: Key) {
        match key {
            Key::Escape => {
                if self.input.is_empty() {
                    self.bus.push(self.id, "cancel".to_string());
                } else {
                    self.input.clear();
                }
            }
            Key::Enter => {
                self.bus.push(self.id, self.input.clone());
            }
            Key::Char(c) => {
                self.input.push(c);
            }
            Key::Backspace => {
                self.input.pop();
            }
            _ => (),
        }
    }
}
