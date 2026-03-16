use crate::{
    buffer::{Buffer, Color},
};

pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }
}
/// Draw a box border with Unicode box-drawing characters.
pub fn draw_box(buf: &mut Buffer, r: &Rect, title: &str, focused: bool) {
    let border_fg = if focused { Color::White } else { Color::Grey };
    let bg = Color::Black;

    let min_x = r.x + 1;
    let max_x = r.x + r.w - 1;
    let max_y = r.y + r.h - 1;

    for x in min_x..(max_x + 1) {
        if x == min_x {
            buf.set(x, r.y, '┌', border_fg, bg);
            buf.set(x, max_y, '└', border_fg, bg);
        } else if x == max_x {
            buf.set(x, r.y, '┐', border_fg, bg);
            buf.set(x, max_y, '┘', border_fg, bg);
        } else {
            buf.set(x, r.y, '─', border_fg, bg);
            buf.set(x, max_y, '─', border_fg, bg);
        }
    }

    for y in (r.y + 1)..max_y {
        buf.set(min_x, y, '│', border_fg, bg);
        buf.set(max_x, y, '│', border_fg, bg);
    }

    // title
    if !title.is_empty() && r.w > 4 {
        let t = format!(" {} ", title);
        let max_len = (r.w - 4) as usize;
        let display: String = t.chars().take(max_len).collect();
        buf.put_str(r.x + 3, r.y, &display, border_fg, bg);
    }
}

pub fn fill_rect(buf: &mut Buffer, r: &Rect, c: char, fg: Color, bg: Color) {
    for x in r.x..(r.x + r.w) {
        buf.set(x, r.y, c, fg, bg);
    }
    for y in r.y..(r.y + r.h) {
        buf.set(r.x, y, c, fg, bg);
    }
}

pub struct MenuItem {
    pub label: String,
    pub event: String,
}

pub struct FileTreeItem {
    pub path: String,
    pub children: Vec<FileTreeItem>,
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

pub fn draw_string_list(
    buf: &mut Buffer,
    r: &Rect,
    title: &str,
    items: &[FileTreeItem],
    max_display_depth: usize,
    scroll_offset: usize,
    focused: bool,
) {
    draw_box(buf, r, title, focused);

    let lm: usize = 3;
    let rm: usize = 3;
    let tm: usize = 1;
    let bm: usize = 2;

    let inner_w = (r.w as usize).saturating_sub(lm + rm);
    let inner_h = (r.h as usize).saturating_sub(tm + bm);

    let mut lines: Vec<String> = Vec::new();
    flatten_tree(items, max_display_depth, 0, &mut lines);

    for i in 0..inner_h {
        let item_idx = i + scroll_offset;
        if item_idx >= lines.len() {
            break;
        }

        let line: String = lines[item_idx].chars().take(inner_w).collect();

        buf.put_str(
            r.x + lm as u16,
            r.y + (tm + i) as u16,
            &line,
            Color::White,
            Color::Reset,
        );
    }
}

pub fn draw_menu(buf: &mut Buffer, r: &Rect, title: &str, items: &[MenuItem], selected_i: usize, focused: bool) {
    draw_box(buf, r, title, focused);

    let rm: usize = 2;
    let lm = 2;
    let tm = 1;
    let bm: usize = 2;

    let inner_w = (r.w as usize) - rm - lm as usize;
    let inner_h = (r.h as usize) - tm - bm as usize;

    for i in 0..inner_h {
        if i >= items.len() {
            break;
        }

        let item = &items[i];

        let selected = i == selected_i;

        let label: String = item
            .label
            .chars()
            .take(inner_w.saturating_sub(rm + bm))
            .collect();
        let check = if selected { ">" } else { " " };

        let line = format!(" {} {}", check, label);

        let (fg, bg) = if selected && focused {
            (Color::Black, Color::Cyan)
        } else if selected {
            (Color::Black, Color::Grey)
        } else {
            (Color::White, Color::Reset)
        };

        fill_rect(
            buf,
            &Rect {
                x: r.x + (lm as u16),
                y: r.y + (tm + i) as u16,
                w: inner_w as u16,
                h: 1,
            },
            ' ',
            fg,
            bg,
        );

        buf.put_str(r.x + (rm as u16), r.y + (tm + i) as u16, &line, fg, bg);
    }
}
