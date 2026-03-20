use crate::buffer::Color;
use crate::ui::draw_box;
use crate::ui::fill_rect;
use crate::{buffer::Buffer, event_bus::EventBus, term::Key, ui::Rect};

use super::widget::Widget;

pub struct MenuItem {
    pub label: String,
    pub event: String,
}

pub struct UIMenu {
    id: &'static str,
    title: String,
    selected_n: usize,
    items: Vec<MenuItem>,
    bus: EventBus,

    r: Rect,

    layout_cb: fn(u16, u16) -> Rect,
}

impl UIMenu {
    pub fn new(id: &'static str, title: String, items: Vec<MenuItem>, bus: EventBus, c: fn(u16, u16) -> Rect) -> Self {
        Self {
            id,
            title,
            items,
            selected_n: 0,
            bus,
            layout_cb: c,
            r: Rect::new(0, 0, 0, 0),
        }
    }

    pub fn add_item(&mut self, label: String, event: String) {
        self.items.push(MenuItem { label, event });
    }
}

impl Widget for UIMenu {
    fn handle_buf_size_change(&mut self, w: u16, h: u16) {
        self.r = (self.layout_cb)(w, h);
    }
    fn draw(&mut self, buffer: &mut Buffer, focused: bool) {
        if self.r.h == 0 || self.r.w == 0 {
            self.handle_buf_size_change(buffer.width, buffer.height);
        }
        draw_menu(
            buffer,
            &self.r,
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
                let item = &self.items[self.selected_n];
                self.bus.push(self.id, item.event.clone());
            }
            _ => (),
        }
    }
}

pub fn draw_menu(
    buf: &mut Buffer,
    r: &Rect,
    title: &str,
    items: &[MenuItem],
    selected_i: usize,
    focused: bool,
) {
    draw_box(buf, r, title, focused);

    let rm: usize = 2;
    let lm = 2;
    let tm = 1;
    let bm: usize = 1;

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
            (Color::Black, Color::Yellow)
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
