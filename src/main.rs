mod buffer;
mod file_duplicates;
mod file_types;
mod term;
mod ui;

use crate::{
    file_duplicates::find_duplicates,
    file_types::{detect_file_type, type_dir},
    term::read_key,
    ui::{MenuItem, Rect},
};
use std::{
    collections::HashMap,
    env,
    error::Error,
    fs::{self},
    io::{self},
    path::{Path, PathBuf},
    process,
};

const USAGE: &str = "\
Usage: crabsort <directory>

Sort and deduplicate files interactively.
";

const LEFT_MARGIN: u16 = 2;
const RIGHT_MARGIN: u16 = 2;
const MENU_HEIGHT: u16 = 6;
const MENU_MARGIN_BOTTOM: u16 = 1;
const SORT_BOX_HEIGHT: u16 = 50;

struct App {
    dir: PathBuf,
    dir_arg: String,
    selected_n: usize,
    mode: usize,
    sortable: Option<HashMap<String, Vec<String>>>,
    menu_items: Vec<MenuItem>,
    buffer: buffer::Buffer,
}

impl App {
    fn new(dir: PathBuf, dir_arg: String) -> Self {
        let (w, h) = term::terminal_size();
        Self {
            dir,
            dir_arg,
            selected_n: 0,
            mode: 0,
            sortable: None,
            menu_items: vec![
                MenuItem {
                    label: String::from("Sort"),
                    key: "sort",
                },
                MenuItem {
                    label: String::from("Duplicates (recursively)"),
                    key: "duplicates",
                },
                MenuItem {
                    label: String::from("Exit"),
                    key: "exit",
                },
            ],
            buffer: buffer::Buffer::new(w, h),
        }
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let (w, h) = term::terminal_size();

            if w != self.buffer.width || h != self.buffer.height {
                self.buffer.resize(w, h);
            }

            self.buffer.clear();
            self.render(w, h);

            print!("{}", self.buffer.flush());
            term::t_flush();

            if !self.handle_input() {
                break;
            }
        }

        Ok(())
    }

    fn render(&mut self, w: u16, h: u16) {
        match self.mode {
            0 => self.render_menu(w, h),
            1 => self.render_sort(w, h),
            _ => (),
        }
    }

    fn render_menu(&mut self, w: u16, h: u16) {
        let menu_rect = Rect {
            x: LEFT_MARGIN,
            y: h - MENU_MARGIN_BOTTOM - MENU_HEIGHT,
            w: w - LEFT_MARGIN - RIGHT_MARGIN,
            h: MENU_HEIGHT,
        };
        ui::draw_menu(
            &mut self.buffer,
            &menu_rect,
            "crabsort",
            &self.menu_items,
            self.selected_n,
        );
    }

    fn render_sort(&mut self, w: u16, h: u16) {
        let sort_rect = Rect {
            x: LEFT_MARGIN,
            y: 2,
            w: w - LEFT_MARGIN - RIGHT_MARGIN,
            h: h - MENU_MARGIN_BOTTOM - 2,
        };
        if let Some(ref h) = self.sortable {
            ui::draw_string_list(&mut self.buffer, &sort_rect, &self.dir_arg, h);
        }
    }

    fn handle_input(&mut self) -> bool {
        match read_key() {
            term::Key::Char('k') => {
                if self.selected_n > 0 {
                    self.selected_n -= 1;
                }
            }
            term::Key::Char('j') => {
                if self.selected_n < self.menu_items.len() - 1 {
                    self.selected_n += 1;
                }
            }
            term::Key::Char('q') => return false,
            term::Key::Enter => match self.mode {
                0 => match self.selected_n {
                    0 => {
                        self.sortable = traverse_dir(&self.dir, true, false).ok();
                        self.mode = 1;
                    }
                    2 => return false,
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        }
        true
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let (dir, dir_arg) = parse_args()?;

    term::enable_raw_mode();
    term::enter_alternate_screen();
    term::hide_cursor();

    let mut app = App::new(dir, dir_arg);
    let result = app.run();

    term::exit_alternate_screen();
    term::disable_raw_mode();
    term::show_cursor();

    result
}

fn parse_args() -> Result<(PathBuf, String), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprint!("{}", USAGE);
        return Err("dir required".into());
    }

    let command = args[1].as_str();
    if command == "--help" || command == "-h" {
        print!("{}", USAGE);
        process::exit(0);
    }

    let mut dir_arg = "";

    for arg in &args[1..] {
        match arg.as_str() {
            s if !s.starts_with('-') => dir_arg = s,
            other => return Err(format!("unknown option: {}", other).into()),
        }
    }

    if dir_arg.is_empty() {
        eprint!("{}", USAGE);
        return Err("directory argument required".into());
    }

    let dir = get_directory(dir_arg)?;
    Ok((dir, dir_arg.to_string()))
}

fn get_directory(s: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = PathBuf::from(s);

    if !path.exists() {
        return Err("path does not exist".into());
    }

    if !path.is_dir() {
        return Err("path is not a directory".into());
    }

    Ok(path)
}

fn traverse_dir(
    p: &Path,
    dry: bool,
    verbose: bool,
) -> Result<HashMap<String, Vec<String>>, Box<dyn Error>> {
    if !p.is_dir() {
        return Err("not a dir".into());
    }

    let r_dir = fs::read_dir(p)?;
    let mut files_map: HashMap<String, Vec<String>> = HashMap::new();

    for entry in r_dir {
        let e = entry?;
        let path = e.path();

        if path.is_dir() {
            continue;
        }

        let file_type = match detect_file_type(&path) {
            Ok(ft) => ft,
            Err(e) => {
                // println!("unsupported file {} : {}", path.display(), e);

                files_map
                    .entry(String::from("unsupported"))
                    .or_default()
                    .push(path.display().to_string());
                continue;
            }
        };

        let paths: Option<(PathBuf, PathBuf)> = type_dir(file_type).and_then(|target_dir| {
            let full_path = p.join(target_dir);
            path.file_name()
                .and_then(|filename| filename.to_str())
                .map(|filename_str| {
                    let new_path = full_path.join(filename_str);
                    (full_path, new_path)
                })
        });

        if paths.is_none() {
            continue;
        }

        let (full_path, new_path) = paths.unwrap();

        let full_path_str = full_path.to_str().ok_or("wrong target dir name")?;

        files_map
            .entry(full_path_str.to_string())
            .or_default()
            .push(new_path.display().to_string());

        if verbose {
            println!("{} -> {}", path.display(), new_path.display());
        }

        if dry {
            if verbose {
                println!("dry run - skip");
            }
            continue;
        }

        ensure_dir(full_path_str)?;

        if let Err(e) = fs::copy(&path, &new_path) {
            eprintln!(
                "Failed to copy {} -> {}: {}",
                path.display(),
                new_path.display(),
                e
            );
        } else {
            if let Err(e) = fs::remove_file(&path) {
                eprintln!("Failed to remove {}: {}", path.display(), e);
            }
        }
    }

    Ok(files_map)
}

fn ensure_dir(p: &str) -> Result<(), io::Error> {
    let exists = fs::exists(p)?;

    if exists {
        return Ok(());
    }

    fs::create_dir_all(p)?;

    Ok(())
}
