use crate::{buffer::Buffer, term::Key};

pub trait Widget {
    fn handle_buf_size_change(&mut self, w: u16, h: u16);
    fn draw(&mut self, buffer: &mut Buffer, focused: bool);
    fn handle_input(&mut self, key: Key);
}

