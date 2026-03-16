use crate::{
    buffer::Buffer,
    term::Key,
    ui::{MenuItem, Rect, draw_menu},
};

use super::widget::Widget;

pub struct UIMenu<F>
where
    F: Fn(u16, u16) -> Rect,
{
    title: String,
    selected_n: usize,
    items: Vec<MenuItem>,

    c: F,
}

impl<F> UIMenu<F>
where
    F: Fn(u16, u16) -> Rect,
{
    pub fn new(title: String, items: Vec<MenuItem>, c: F) -> Self {
        Self {
            title,
            items,
            selected_n: 0,
            c,
        }
    }

    pub fn add_item(&mut self, title: String, action: Box<dyn Fn()>) {
        self.items.push(MenuItem {
            label: title,
            action,
        });
    }
}

impl<F> Widget for UIMenu<F>
where
    F: Fn(u16, u16) -> Rect,
{
    fn draw(&self, buffer: &mut Buffer, focused: bool) {
        let r = (self.c)(buffer.width, buffer.height);

        draw_menu(
            buffer,
            &r,
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
                (self.items[self.selected_n].action)();
            }
            _ => (),
        }
    }
}
