use std::{cell::RefCell, collections::HashSet, rc::Rc};

use crate::{
    buffer::Buffer,
    term::Key,
    ui::{Rect, draw_string_list_flat},
};

use super::widget::Widget;

#[derive(Clone)]
pub struct FileTreeItem {
    pub path: String,
    pub children: Vec<FileTreeItem>,
}

pub struct UIFileList {
    title: String,
    max_depth: usize,
    highlighted_line: usize,
    selected: HashSet<(usize, usize)>,
    scroll_offset: usize,

    items: Vec<FileTreeItem>,
    lines: Vec<String>,

    output: Option<Rc<RefCell<Vec<FileTreeItem>>>>,
    group_offsets: Vec<usize>,

    r: Rect,
    layout_cb: fn(u16, u16) -> Rect,
}

impl UIFileList {
    pub fn new(
        title: String,
        items: Vec<FileTreeItem>,
        max_depth: usize,
        output: Option<Rc<RefCell<Vec<FileTreeItem>>>>,
        preselect_all: bool,
        c: fn(u16, u16) -> Rect,
    ) -> Self {
        let mut lines: Vec<String> = Vec::new();
        flatten_tree(&items, max_depth, 0, &mut lines);

        let mut group_offsets = Vec::new();
        group_offsets.resize(items.len(), 0);

        let mut selected = HashSet::new();

        for (i, item) in items.iter().enumerate() {
            if preselect_all {
                for (j, _child) in item.children.iter().enumerate() {
                    selected.insert((i, j));
                }
            }

            group_offsets[i] = if i > 0 {
                items[i - 1].children.len() + 1
            } else {
                0
            }
        }

        Self {
            title,
            items,
            highlighted_line: 0,
            selected,
            max_depth,
            scroll_offset: 0,
            lines,
            output,
            group_offsets,
            r: Rect::new(0, 0, 0, 0),
            layout_cb: c,
        }
    }

    // 3 groups: 2, 3, 1
    // 0 -> None
    // 1 -> 0,0
    // 2 -> 0,1
    // 3 -> None
    // 4 -> 1,0
    // 5 -> 1,1
    // 6 -> 1,2
    fn flat_to_coords(&self, n: usize) -> Option<(usize, usize)> {
        for (i, item) in self.items.iter().enumerate() {
            for (j, _) in item.children.iter().enumerate() {
                if self.coords_to_flat(i, j) == n {
                    return Some((i, j));
                }
            }
        }
        None
    }

    fn coords_to_flat(&self, i: usize, j: usize) -> usize {
        self.group_offsets[i] + 1 + j
    }

    fn refresh_output(&self) -> Vec<FileTreeItem> {
        let mut m: Vec<FileTreeItem> = vec![];

        for (i, group) in self.items.iter().enumerate() {
            let children: Vec<FileTreeItem> = group
                .children
                .iter()
                .enumerate()
                .filter_map(|(f_j, file)| {
                    if !&self.selected.contains(&(i, f_j)) {
                        return None;
                    }

                    Some(FileTreeItem {
                        path: file.path.clone(),
                        children: vec![],
                    })
                })
                .collect();
            if children.len() > 0 {
                m.push(FileTreeItem {
                    path: group.path.clone(),
                    children,
                });
            }
        }
        m
    }
}

impl Widget for UIFileList {
    fn handle_buf_size_change(&mut self, w: u16, h: u16) {
        self.r = (self.layout_cb)(w, h);
    }
    fn draw(&mut self, buffer: &mut Buffer, focused: bool) {
        if self.r.h == 0 || self.r.w == 0 {
            self.handle_buf_size_change(buffer.width, buffer.height);
        }
        let selected_lines: Vec<usize> = self
            .selected
            .iter()
            .map(|(i, j)| self.coords_to_flat(*i, *j))
            .collect();
        draw_string_list_flat(
            buffer,
            &self.r,
            self.title.as_str(),
            &self.lines,
            self.scroll_offset,
            self.highlighted_line,
            focused,
            Some(selected_lines),
        );
    }

    fn handle_input(&mut self, key: Key) {
        match key {
            Key::Char('k') => {
                if self.highlighted_line > 0 {
                    self.highlighted_line -= 1;
                }
                if self.scroll_offset > 0 && self.highlighted_line <= self.scroll_offset + 1 {
                    self.scroll_offset -= 1;
                }
            }
            Key::Char('j') => {
                let l = self.lines.len();

                if l == 0 {
                    return;
                }
                //
                if self.highlighted_line < l - 1 {
                    self.highlighted_line += 1;
                }

                let h = (self.r.h - 2) as usize; // 2 lines margin
                if l < h {
                    return;
                }

                let max_scroll = l - h; //e.g. 20 lines, height is 15, max scroll is 5
                // or offset is at the bottom already
                if self.scroll_offset >= max_scroll {
                    return;
                }
                if (self.scroll_offset + self.highlighted_line + 1) >= h {
                    self.scroll_offset += 1;
                }
            }
            Key::Space => {
                let Some(coords) = self.flat_to_coords(self.highlighted_line) else {
                    return;
                };

                if self.selected.contains(&coords) {
                    self.selected.remove(&coords);
                } else {
                    self.selected.insert(coords);
                }

                if let Some(o) = self.output.as_ref() {
                    *o.borrow_mut() = self.refresh_output();
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
