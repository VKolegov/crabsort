use crate::buffer::{Buffer, Color};

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
    let border_fg = if focused { Color::Cyan } else { Color::Grey };
    let bg = Color::Reset;


    for x in r.x..(r.x + r.w) {
        if x == r.x {
            buf.set(x, r.y, '┌', border_fg, bg);
            buf.set(x, r.y + r.h - 1, '└', border_fg, bg);
        } else if x + 1 == r.x + r.w {
            buf.set(x, r.y, '┐', border_fg, bg);
            buf.set(x, r.y + r.h - 1, '┘', border_fg, bg);
        } else {
            buf.set(x, r.y, '─', border_fg, bg);
            buf.set(x, r.y + r.h - 1, '─', border_fg, bg);
        }
    }

    for y in (r.y + 1) ..(r.y + r.h - 1) {
        buf.set(r.x, y, '│', border_fg, bg);
        buf.set(r.x + r.w - 1, y, '│', border_fg, bg);
    }


    //
    // // corners
    // buf.set(r.x, r.y, '┌', border_fg, bg, false);
    // buf.set(r.x + r.w - 1, r.y, '┐', border_fg, bg, false);
    // buf.set(r.x, r.y + r.h - 1, '└', border_fg, bg, false);
    // buf.set(r.x + r.w - 1, r.y + r.h - 1, '┘', border_fg, bg, false);
    //
    // // horizontal lines
    // for x in (r.x + 1)..(r.x + r.w - 1) {
    //     buf.set(x, r.y, '─', border_fg, bg, false);
    //     buf.set(x, r.y + r.h - 1, '─', border_fg, bg, false);
    // }
    //
    // // vertical lines
    // for y in (r.y + 1)..(r.y + r.h - 1) {
    //     buf.set(r.x, y, '│', border_fg, bg, false);
    //     buf.set(r.x + r.w - 1, y, '│', border_fg, bg, false);
    // }
    //
    // // title
    // if !title.is_empty() && r.w > 4 {
    //     let t = format!(" {} ", title);
    //     let max_len = (r.w - 4) as usize;
    //     let display: String = t.chars().take(max_len).collect();
    //     buf.put_str(r.x + 2, r.y, &display, border_fg, bg, true);
    // }
}

