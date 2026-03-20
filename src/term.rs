use std::{
    io::{self, Read, Write},
    os::fd::AsRawFd,
    sync::OnceLock,
};

static ORIGINAL_TERMIOS: OnceLock<libc::termios> = OnceLock::new();

pub fn enable_raw_mode() {
    let fd = ti_fd();

    unsafe {
        let mut termios: libc::termios = std::mem::zeroed();

        libc::tcgetattr(fd, &mut termios);

        ORIGINAL_TERMIOS.set(termios).ok();

        let mut raw = termios;

        // local flags (termial behaviour)
        raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG | libc::IEXTEN);

        // input flags (incoming bytes preprocessing)
        raw.c_iflag &= !(libc::IXON | libc::ICRNL | libc::BRKINT | libc::INPCK | libc::ISTRIP);

        // output flags (post-processing)
        raw.c_oflag &= !libc::OPOST;

        // control characters (read behavior)
        // VMIN = 0 - read returns immediately even with 0 bytes
        // VTIME = 1 (100ms) - wait 100ms for input
        // (for 100ms polling)
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 1;

        libc::tcsetattr(fd, libc::TCSAFLUSH, &raw);
    }
}

pub fn disable_raw_mode() {
    if let Some(termios) = ORIGINAL_TERMIOS.get() {
        unsafe {
            libc::tcsetattr(ti_fd(), libc::TCSAFLUSH, termios);
        }
    }
}

pub fn terminal_size() -> (u16, u16) {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        libc::ioctl(to_fd(), libc::TIOCGWINSZ, &mut ws);
        (ws.ws_col, ws.ws_row)
    }
}


pub fn enter_alternate_screen() {
    print!("\x1b[?1049h");
    t_flush();
}

pub fn exit_alternate_screen() {
    print!("\x1b[?1049l");
    t_flush();
}

pub fn hide_cursor() {
    print!("\x1b[?25l");
    t_flush();
}

pub fn show_cursor() {
    print!("\x1b[?25h");
    t_flush();
}

// keys

pub enum Key {
    Char(char),
    Tab,
    Enter,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Backspace,
    Space,
    None,
}

pub fn read_key() -> Key {
    let mut buf = [0u8; 4];

    let n = io::stdin().read(&mut buf).unwrap_or(0);

    if n == 0 {
        return Key::None;
    }

    match buf[0] {
        b'\t' => Key::Tab,
        b'\r' => Key::Enter,
        b' ' => Key::Space,
        0x08 | 0x7F => Key::Backspace,
        0x1b => {
            if n == 1 {
                return Key::Escape;
            }

            if n == 3 && buf[1] == b'[' {
                match buf[2] {
                    b'A' => Key::Up,
                    b'B' => Key::Down,
                    b'C' => Key::Right,
                    b'D' => Key::Left,
                    _ => Key::None,
                }
            } else {
                Key::Escape
            }
        }
        c if c >= 32 && c < 127 => Key::Char(c as char),
        _ => Key::None,
    }
}

// io helpers
fn ti_fd() -> i32 {
    return io::stdout().as_raw_fd();
}

fn to_fd() -> i32 {
    return io::stdout().as_raw_fd();
}

pub fn t_flush() {
    io::stdout().flush().unwrap();
}


