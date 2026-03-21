use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{
    buffer::Buffer,
    event_bus::EventBus,
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
    selected_n: usize,
    selected: HashSet<usize>,
    scroll_offset: usize,

    items: Vec<FileTreeItem>,
    lines: Vec<String>,

    bus: EventBus,
    output: Option<Rc<RefCell<Vec<FileTreeItem>>>>,
    m: HashMap<usize, (usize, usize)>,
    r_m: HashMap<(usize, usize), usize>,

    r: Rect,
    layout_cb: fn(u16, u16) -> Rect,
}

impl UIFileList {
    pub fn new(
        title: String,
        items: Vec<FileTreeItem>,
        max_depth: usize,
        bus: EventBus,
        // output: Option<Rc<RefCell<HashMap<String, Vec<String>>>>>,
        output: Option<Rc<RefCell<Vec<FileTreeItem>>>>,
        c: fn(u16, u16) -> Rect,
    ) -> Self {
        let mut lines: Vec<String> = Vec::new();
        flatten_tree(&items, max_depth, 0, &mut lines);

        let mut m = HashMap::<usize, (usize, usize)>::new();
        let mut r_m = HashMap::<(usize, usize), usize>::new();

        let mut selected = HashSet::new();

        let mut count = 1;
        for (i, item) in items.iter().enumerate() {
            let pidor = FileTreeItem {
                path: item.path.clone(),
                children: vec![],
            };

            for (j, _child) in item.children.iter().enumerate() {
                m.insert(count + j, (i, j));
                r_m.insert((i, j), count + j);
                selected.insert(count + j);
            }

            if let Some(rc) = &output {
                rc.borrow_mut().push(pidor);
            }
            count += item.children.len() + 1;
        }

        Self {
            title,
            items,
            selected_n: 0,
            selected,
            max_depth,
            scroll_offset: 0,
            lines,
            bus,
            output,
            m,
            r_m,
            r: Rect::new(0, 0, 0, 0),
            layout_cb: c,
        }
    }

    fn refresh_output(&self) -> Vec<FileTreeItem> {
        let mut m: Vec<FileTreeItem> = vec![];

        let mut current_line: usize = 0;
        for file_group in &self.items {
            for f_f in &file_group.children {
                current_line += 1;
                let mut children = vec![];
                if self.selected.contains(&current_line) {
                    children.push(f_f.clone());
                }

                if children.len() > 0 {
                    m.push(FileTreeItem {
                        path: file_group.path.clone(),
                        children,
                    });
                }
            }
            current_line += 1;
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
        let selected_items: Vec<usize> = self.selected.clone().into_iter().collect();
        draw_string_list_flat(
            buffer,
            &self.r,
            self.title.as_str(),
            &self.lines,
            self.scroll_offset,
            self.selected_n,
            focused,
            Some(selected_items),
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

                if l == 0 {
                    return;
                }
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
                    return;
                }
                if (self.scroll_offset + self.selected_n + 1) >= h {
                    self.scroll_offset += 1;
                }
            }
            Key::Space => {
                if self.selected.contains(&self.selected_n) {
                    self.selected.remove(&self.selected_n);
                } else {
                    self.selected.insert(self.selected_n);
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
