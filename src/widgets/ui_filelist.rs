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

    r: Rect,
    layout_cb: F,
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
            r: Rect::new(0,0,0,0),
            layout_cb: c,
        }
    }

    // pub fn flatten(&mut self) {
    //
    //     let mut lines: Vec<String> = Vec::new();
    //     flatten_tree(&self.items, self.max_depth, 0, &mut lines);
    //     self.lines = lines;
    // }
}

impl<F> Widget for UIFileList<F>
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
        draw_string_list_flat(
            buffer,
            &self.r,
            self.title.as_str(),
            &self.lines,
            self.scroll_offset,
            self.selected_n,
            focused,
        );
    }

    fn handle_input(&mut self, key: Key) {
        match key {
            Key::Char('k') => {
                if self.selected_n > 0 {
                    self.selected_n -= 1;
                }
                if self.scroll_offset > 0 && self.selected_n <= self.scroll_offset + 1 {
                    self.scroll_offset -= 1;
                }
            }
            Key::Char('j') => {
                let l = self.lines.len();
                                                 //
                if self.selected_n < l - 1 {
                    self.selected_n += 1;
                }
                
                let h = (self.r.h - 2) as usize; // 2 lines margin
                if l < h {
                    return;
                }

                let max_scroll = l - h; //e.g. 20 lines, height is 15, max scroll is 5
                // or offset is at the bottom already
                if self.scroll_offset >= max_scroll {
                    return
                }
                if (self.scroll_offset + self.selected_n + 1) >= h {
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

