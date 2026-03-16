use crate::{buffer::Buffer, term::Key};

pub trait Widget {
    fn draw(&self, buffer: &mut Buffer, focused: bool);
    fn handle_input(&mut self, key: Key);
}
