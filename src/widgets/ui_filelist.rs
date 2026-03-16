use crate::{
    buffer::Buffer,
    term::Key,
    ui::{FileTreeItem, Rect, draw_string_list},
};

use super::widget::Widget;

pub struct UIFileList<F>
where
    F: Fn(u16, u16) -> Rect,
{
    title: String,
    selected_n: usize,

    items: Vec<FileTreeItem>,
    c: F,
}

impl<F> UIFileList<F>
where
    F: Fn(u16, u16) -> Rect,
{
    pub fn new(title: String, items: Vec<FileTreeItem>, c: F) -> Self {
        Self {
            title,
            items,
            selected_n: 0,
            c,
        }
    }
}

impl<F> Widget for UIFileList<F>
where
    F: Fn(u16, u16) -> Rect,
{
    fn draw(&self, buffer: &mut Buffer, focused: bool) {
        let r = (self.c)(buffer.width, buffer.height);

        draw_string_list(
            buffer,
            &r,
            self.title.as_str(),
            &self.items,
            1,
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
            _ => (),
        }
    }
}
