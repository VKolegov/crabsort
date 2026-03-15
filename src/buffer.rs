/// Double-buffered screen. Renders only the diff between frames.
pub struct Buffer {
    pub width: u16,
    pub height: u16,
    current: Vec<Cell>,
    previous: Vec<Cell>,
}

impl Buffer {
    pub fn new(w: u16, h: u16) -> Self {
        let size = (w as usize) * (h as usize);
        Buffer {
            width: w,
            height: h,
            current: vec![Cell::default(); size],
            previous: vec![],
        }
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
        let size = (w as usize) * (h as usize);
        self.current = vec![Cell::default(); size];
        self.previous = vec![];
    }

    pub fn clear(&mut self) {
        for cell in &mut self.current {
            *cell = Cell::default(); // on-stack memcpy!
        }
    }

    fn idx(&self, x: u16, y: u16) -> usize {
        (y as usize) * (self.width as usize) + (x as usize)
    }

    pub fn set(&mut self, x: u16, y: u16, ch: char, fg: Color, bg: Color) {
        if x < self.width && y < self.height {
            let i = self.idx(x, y);
            self.current[i] = Cell {
                c: ch,
                color: fg,
                bg,
            };
        }
    }

    pub fn flush(&mut self) -> String {
        let mut out = String::with_capacity(self.current.len());

        let full_redraw = self.current.len() != self.previous.len();

        let mut last_color = Color::Reset;
        let mut last_bg = Color::Reset;
        let mut last_pos: (u16, u16) = (255, 255);

        for x in 0..self.width {
            for y in 0..self.height {
                let i = self.idx(x, y);

                let cell = &self.current[i];

                if !full_redraw && self.previous[i] == *cell {
                    continue;
                }

                if last_pos != (x, y) {
                    out.push_str(&format!("\x1b[{};{}H", y + 1, x + 1));
                }

                if last_color != cell.color || last_bg != cell.bg {
                    out.push_str(&format!(
                        "\x1b[0;{};{}m",
                        cell.color.fg_code(),
                        cell.bg.bg_code()
                    ));

                    last_color = cell.color;
                    last_bg = cell.bg;
                }

                out.push(cell.c);
                last_pos = (x, y);
            }
        }

        out.push_str("\x1b[0m");

        self.previous = self.current.clone(); // swap

        out
    }
}

#[derive(Clone, PartialEq)]
pub struct Cell {
    pub c: char,
    pub color: Color,
    pub bg: Color,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            c: ' ',
            color: Color::Reset,
            bg: Color::Reset,
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
pub enum Color {
    Reset,
    White,
    Black,
    Grey,
    Blue,
    Cyan,
    Yellow,
    Green,
}

impl Color {
    pub fn fg_code(self) -> &'static str {
        match self {
            Color::Reset => "0",
            Color::White => "97",
            Color::Black => "30",
            Color::Grey => "90",
            Color::Blue => "34",
            Color::Cyan => "36",
            Color::Yellow => "33",
            Color::Green => "32",
        }
    }

    pub fn bg_code(self) -> &'static str {
        match self {
            Color::Reset => "0",
            Color::White => "107",
            Color::Black => "40",
            Color::Grey => "100",
            Color::Blue => "44",
            Color::Cyan => "46",
            Color::Yellow => "43",
            Color::Green => "42",
        }
    }
}
