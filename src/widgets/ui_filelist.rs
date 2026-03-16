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
    scroll_offset: usize,

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
            scroll_offset: 0,
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
            self.scroll_offset,
            focused,
        );
    }

    fn handle_input(&mut self, key: Key) {
        match key {
            Key::Char('k') => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            Key::Char('j') => {
                // TODO: not by items, by actual size
                if self.scroll_offset < self.items.len() - 1 {
                    self.scroll_offset += 1;
                }
            }
            _ => (),
        }
    }
}
