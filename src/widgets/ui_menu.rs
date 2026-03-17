use crate::{
    buffer::Buffer,
    event_bus::EventBus,
    term::Key,
    ui::{MenuItem, Rect, draw_menu},
};

use super::widget::Widget;

pub struct UIMenu<F>
where
    F: Fn(u16, u16) -> Rect,
{
    id: &'static str,
    title: String,
    selected_n: usize,
    items: Vec<MenuItem>,
    bus: EventBus,

    r: Rect,

    layout_cb: F,
}

impl<F> UIMenu<F>
where
    F: Fn(u16, u16) -> Rect,
{
    pub fn new(id: &'static str, title: String, items: Vec<MenuItem>, bus: EventBus, c: F) -> Self {
        Self {
            id,
            title,
            items,
            selected_n: 0,
            bus,
            layout_cb: c,
            r: Rect::new(0,0,0,0),
        }
    }

    pub fn add_item(&mut self, label: String, event: String) {
        self.items.push(MenuItem { label, event });
    }
}

impl<F> Widget for UIMenu<F>
where
    F: Fn(u16, u16) -> Rect,
{
    fn handle_buf_size_change(&mut self, w: u16, h: u16) {
        self.r = (self.layout_cb)(w,h);
    }
    fn draw(&mut self, buffer: &mut Buffer, focused: bool) {
        if self.r.h == 0 || self.r.w == 0 {
            self.handle_buf_size_change(buffer.width, buffer.height);
        }
        draw_menu(
            buffer,
            &self.r,
            self.title.as_str(),
            &self.items,
            self.selected_n.into(),
            focused,
        );
    }

    fn handle_input(&mut self, key: Key) {
        match key {
            Key::Char('k') => {
                if self.selected_n > 0 {
                    self.selected_n -= 1;
                }
            }
            Key::Char('j') => {
                if self.selected_n < self.items.len() - 1 {
                    self.selected_n += 1;
                }
            }
            Key::Enter => {
                let item = &self.items[self.selected_n];
                self.bus.push(self.id, item.event.clone());
            }
            _ => (),
        }
    }
}
