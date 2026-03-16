mod buffer;
mod event_bus;
mod file_duplicates;
mod file_types;
mod term;
mod ui;
mod widgets;

use crate::{
    event_bus::EventBus,
    file_duplicates::find_duplicates,
    file_types::{detect_file_type, type_dir},
    term::read_key,
    ui::{FileTreeItem, Rect},
    widgets::{UIFileList, UIMenu, Widget},
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

const FILE_LIST_TOP: u16 = 2;
const FILE_LIST_GAP: u16 = 2;

struct App {
    dir: PathBuf,
    dir_arg: String,
    buffer: buffer::Buffer,

    widgets: Vec<Box<dyn Widget>>,
    selected_widget: usize,
    bus: EventBus,

    progress_active: bool,
    progress_current: usize,
    progress_max: usize,

    quit: bool,
}

const MENU_MAIN: &str = "main_menu";
const MENU_CONFIRM_SORT: &str = "confirm_sort_menu";

impl App {
    fn new(dir: PathBuf, dir_arg: String) -> Self {
        let (w, h) = term::terminal_size();
        Self {
            dir,
            dir_arg,
            widgets: vec![],
            selected_widget: 0,
            bus: EventBus::new(),
            buffer: buffer::Buffer::new(w, h),
            progress_active: false,
            progress_current: 0,
            progress_max: 0,
            quit: false,
        }
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        self.go_to_first_page();

        self.progress_active = false;
        self.progress_current = 0;
        self.progress_max = 1000;

        loop {
            let (w, h) = term::terminal_size();

            if w != self.buffer.width || h != self.buffer.height {
                self.buffer.resize(w, h);
            }

            self.buffer.clear();
            self.render();

            if self.progress_active {
                self.render_progress(self.progress_current, self.progress_max);
            }

            print!("{}", self.buffer.flush());
            term::t_flush();

            if self.quit || !self.handle_input() {
                break;
            }

            self.handle_events();
        }

        Ok(())
    }

    fn render_progress(&mut self, c: usize, m: usize) {
        let bw = self.buffer.width;
        let bh = self.buffer.height;

        let p = c * (bw - 10) as usize / m;

        let a = "█".repeat(p as usize);
        let b = format!("{}/{}", self.progress_current, self.progress_max);

        let h = 7;
        let w = bw - 10;

        let r = Rect {
            x: bw / 2 - w / 2,
            y: bh / 2 - h / 2,
            w: w,
            h: h,
        };


        let mut ri = r.clone();
        ri.x += 1;
        ri.y += 1;
        ri.w -= 2;
        ri.h -= 2;

        ui::draw_box(
            &mut self.buffer,
            &r,
            "Working...",
            true,
        );
        ui::fill_rect(&mut self.buffer, &ri, ' ', buffer::Color::Black, buffer::Color::Black);

        self.buffer.put_str(
            r.x + 2,
            r.y + 2,
            &b,
            buffer::Color::White,
            buffer::Color::Black,
        );
        self.buffer.put_str(
            r.x + 2,
            r.y + 4,
            &a,
            buffer::Color::Yellow,
            buffer::Color::Yellow,
        );
    }

    fn render(&mut self) {
        for (i, w) in self.widgets.iter().enumerate() {
            w.draw(&mut self.buffer, !self.progress_active && i == self.selected_widget);
        }
    }

    fn handle_input(&mut self) -> bool {
        let k = read_key();
        match k {
            term::Key::Char('q') => return false,
            term::Key::Char('s') => {
                self.progress_active = !false;
            },
            term::Key::Tab => {
                if self.selected_widget < self.widgets.len() - 1 {
                    self.selected_widget += 1;
                } else {
                    self.selected_widget = 0;
                }
            }
            _ => (),
        }

        if !self.progress_active {
            self.widgets[self.selected_widget].handle_input(k);
        }

        true
    }

    fn handle_events(&mut self) {
        for event in &self.bus.drain() {
            match (event.source, event.payload.as_str()) {
                (MENU_MAIN, "sort_by_type") => self.handle_sort_by_type(true),
                (MENU_MAIN, "find_duplicates") => {
                    // TODO: implement
                }
                (MENU_MAIN, "quit") => {
                    self.quit = true;
                }
                (MENU_CONFIRM_SORT, "no") => self.go_to_first_page(),
                _ => {}
            }
        }
    }

    fn handle_sort_by_type(&mut self, dry: bool) {
        match fix_duplicates_in_dir(&self.dir, dry) {
            Ok(files) => {
                let dir_list = UIFileList::new(
                    self.dir.display().to_string(),
                    files,
                    2,
                    |w: u16, h: u16| {
                        let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
                        let file_list_h = menu_y.saturating_sub(FILE_LIST_TOP + FILE_LIST_GAP);
                        Rect {
                            x: LEFT_MARGIN,
                            y: FILE_LIST_TOP,
                            w: w - LEFT_MARGIN - RIGHT_MARGIN,
                            h: file_list_h,
                        }
                    },
                );
                let mut menu = UIMenu::new(
                    MENU_CONFIRM_SORT,
                    "Confirm action".to_string(),
                    vec![],
                    self.bus.clone(),
                    |w: u16, h: u16| {
                        let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
                        Rect {
                            x: LEFT_MARGIN,
                            y: menu_y,
                            w: w - LEFT_MARGIN - RIGHT_MARGIN,
                            h: MENU_HEIGHT,
                        }
                    },
                );

                menu.add_item("Confirm".to_string(), "yes".to_string());
                menu.add_item("Cancel".to_string(), "no".to_string());

                self.widgets = vec![Box::new(menu), Box::new(dir_list)];
            }
            Err(_) => (),
        }
    }

    fn go_to_first_page(&mut self) {
        let dir_files = read_dir_files(&self.dir);

        let dir_list = UIFileList::new(
            self.dir.display().to_string(),
            dir_files,
            2,
            |w: u16, h: u16| {
                let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
                let file_list_h = menu_y.saturating_sub(FILE_LIST_TOP + FILE_LIST_GAP);
                Rect {
                    x: LEFT_MARGIN,
                    y: FILE_LIST_TOP,
                    w: w - LEFT_MARGIN - RIGHT_MARGIN,
                    h: file_list_h,
                }
            },
        );

        let mut menu = UIMenu::new(
            MENU_MAIN,
            "crabsort".to_string(),
            vec![],
            self.bus.clone(),
            |w: u16, h: u16| {
                let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
                Rect {
                    x: LEFT_MARGIN,
                    y: menu_y,
                    w: w - LEFT_MARGIN - RIGHT_MARGIN,
                    h: MENU_HEIGHT,
                }
            },
        );

        menu.add_item("Sort by type".to_string(), "sort_by_type".to_string());
        menu.add_item("Find duplicates".to_string(), "find_duplicates".to_string());
        menu.add_item("Quit".to_string(), "quit".to_string());

        self.widgets = vec![Box::new(menu), Box::new(dir_list)];
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

fn read_dir_files(p: &Path) -> Vec<FileTreeItem> {
    let Ok(entries) = fs::read_dir(p) else {
        return Vec::new();
    };

    let mut items: Vec<FileTreeItem> = Vec::new();
    for entry in entries {
        let Ok(e) = entry else { continue };
        let path = e.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if name.starts_with('.') {
            continue;
        }

        items.push(FileTreeItem {
            path: name,
            children: Vec::new(),
        });
    }
    items
}

fn fix_duplicates_in_dir(p: &Path, dry: bool) -> Result<Vec<FileTreeItem>, Box<dyn Error>> {
    traverse_dir(p, dry, false).map(|map| {
        map.into_iter()
            .map(|(key, files)| FileTreeItem {
                path: key,
                children: files
                    .into_iter()
                    .map(|f| FileTreeItem {
                        path: f,
                        children: Vec::new(),
                    })
                    .collect(),
            })
            .collect()
    })
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

        let is_dot = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if is_dot {
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
            .push(path.display().to_string());

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
