use crate::{
    buffer::Buffer,
    term::Key,
    ui::{FileTreeItem, Rect, draw_string_list_flat},
};

use super::widget::Widget;

pub struct UIFileList<F>
where
    F: Fn(u16, u16) -> Rect,
{
    title: String,
    max_depth: usize,
    selected_n: usize,
    scroll_offset: usize,

    items: Vec<FileTreeItem>,
    lines: Vec<String>,
    c: F,
}

impl<F> UIFileList<F>
where
    F: Fn(u16, u16) -> Rect,
{
    pub fn new(title: String, items: Vec<FileTreeItem>, max_depth: usize, c: F) -> Self {

        let mut lines: Vec<String> = Vec::new();
        flatten_tree(&items, max_depth, 0, &mut lines);

        Self {
            title,
            items,
            selected_n: 0,
            max_depth,
            scroll_offset: 0,
            lines,
            c,
        }
    }

    pub fn flatten(&mut self) {

        let mut lines: Vec<String> = Vec::new();
        flatten_tree(&self.items, self.max_depth, 0, &mut lines);
        self.lines = lines;
    }
}

impl<F> Widget for UIFileList<F>
where
    F: Fn(u16, u16) -> Rect,
{
    fn draw(&self, buffer: &mut Buffer, focused: bool) {
        let r = (self.c)(buffer.width, buffer.height);

        draw_string_list_flat(
            buffer,
            &r,
            self.title.as_str(),
            &self.lines,
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
                if self.scroll_offset < self.lines.len() - 1 {
                    self.scroll_offset += 1;
                }
            }
            _ => (),
        }
    }
}


fn flatten_tree(items: &[FileTreeItem], max_depth: usize, depth: usize, out: &mut Vec<String>) {
    if depth >= max_depth {
        return;
    }
    let indent = "    ".repeat(depth);
    for item in items {
        out.push(format!("{}{}", indent, item.path));
        flatten_tree(&item.children, max_depth, depth + 1, out);
    }
}

