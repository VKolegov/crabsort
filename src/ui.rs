use crate::buffer::{Buffer, Color};

#[derive(Clone)]
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
    let border_fg = if focused { Color::Yellow } else { Color::Grey };
    let bg = Color::Black;

    let min_x = r.x;
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
    for y in r.y..(r.y + r.h) {
        for x in r.x..(r.x + r.w) {
            buf.set(x, y, c, fg, bg);
        }
    }
}

pub fn draw_string_list_flat(
    buf: &mut Buffer,
    r: &Rect,
    title: &str,
    lines: &Vec<String>,
    scroll_offset: usize,
    highlighted_n: usize,
    focused: bool,
    selected: Option<Vec<usize>>,
) {
    draw_box(buf, r, title, focused);

    let lm: usize = 3;
    let rm: usize = 3;
    let tm: usize = 1;
    let bm: usize = 1;

    let inner_w = (r.w as usize).saturating_sub(lm + rm);
    let inner_h = (r.h as usize).saturating_sub(tm + bm);

    for i in 0..inner_h {
        let item_idx = i + scroll_offset;
        if item_idx >= lines.len() {
            break;
        }

        let item_highlighted = highlighted_n == item_idx;

        let item_selected = selected.as_ref()
            .map_or(false, |v| v.contains(&item_idx));

        let line: String = lines[item_idx].chars().take(inner_w).collect();

        let (fg, bg) = match focused {
            true => {
                if item_highlighted {
                    (Color::Black, Color::Yellow)
                } else if item_selected {
                    (Color::Black, Color::Green) 
                } else {
                    (Color::White, Color::Reset)
                }
            }
            false => {
                if item_highlighted {
                    (Color::Black, Color::Grey)
                } else if item_selected {
                    (Color::Green, Color::Black) 
                } else {
                    (Color::White, Color::Reset)
                }
            }
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

        buf.put_str(r.x + lm as u16, r.y + (tm + i) as u16, &line, fg, bg);
    }
}
