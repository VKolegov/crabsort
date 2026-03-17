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

    c: F,
}

impl<F> UIInputDialog<F>
where
    F: Fn(u16, u16) -> Rect,
{
    pub fn new(id: &'static str, title: String, bus: EventBus, c: F) -> Self {
        Self {
            id,
            title,
            bus,
            input: String::new(),
            c,
        }
    }
}

impl<F> Widget for UIInputDialog<F>
where
    F: Fn(u16, u16) -> Rect,
{
    fn draw(&self, buffer: &mut Buffer, focused: bool) {
        let r = (self.c)(buffer.width, buffer.height);

        draw_box(buffer, &r, &self.title, focused);
        let mut ri = r.clone();
        ri.x += 1;
        ri.y += 1;
        ri.w -= 2;
        ri.h -= 2;
        fill_rect(
            buffer,
            &ri,
            ' ',
            crate::buffer::Color::Black,
            crate::buffer::Color::Black,
        );

        buffer.put_str(r.x+2, r.y+2, &self.input, crate::buffer::Color::White, crate::buffer::Color::Black);
    }

    fn handle_input(&mut self, key: Key) {
        match key {
            Key::Enter => {
                self.bus.push(self.id, self.input.clone());
            },
            Key::Char(c) => {
                self.input.push(c);
            },
            Key::Backspace => {
                self.input.pop();
            },
            _ => (),
        }
    }
}
