mod buffer;
mod event_bus;
mod file_duplicates;
mod file_types;
mod term;
mod ui;
mod widgets;

use crate::{
    event_bus::EventBus,
    file_duplicates::{FileInfo, find_duplicates_async},
    file_types::{detect_file_type, type_dir},
    term::read_key,
    ui::{FileTreeItem, Rect},
    widgets::{UIFileList, UIInputDialog, UIMenu, Widget},
};
use std::{
    collections::HashMap,
    env,
    error::Error,
    fs::{self},
    io::{self},
    path::{Path, PathBuf},
    process,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

const USAGE: &str = "\
Usage: crabsort <directory>

Sort and deduplicate files interactively.
";

const LEFT_MARGIN: u16 = 2;
const RIGHT_MARGIN: u16 = 2;
const MENU_HEIGHT: u16 = 6;
const MENU_MARGIN_BOTTOM: u16 = 1;

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
    progress_current: Arc<Mutex<u64>>,
    progress_max: Arc<Mutex<u64>>,

    latest_input: Option<String>,
    important_widget: Option<Box<dyn Widget>>,

    duplicates_map: Arc<Mutex<HashMap<String, Vec<Arc<FileInfo>>>>>,
    duplicates_thread: Option<JoinHandle<Option<HashMap<String, Vec<Arc<FileInfo>>>>>>,

    quit: bool,
}

const MENU_MAIN: &str = "main_menu";
const ACTION_SORT: &str = "sort";
const ACTION_FIND_DUPLICATES: &str = "find_duplicates";
const ACTION_ASK_DUPLICATES_MIN_SIZE: &str = "duplicates_min_size";

const MENU_CONFIRM_SORT: &str = "confirm_sort_menu";
const MENU_DUPLICATES: &str = "duplicates_menu";

const ACTION_CONFIRM: &str = "confirm";
const ACTION_BACK: &str = "back";
const ACTION_QUIT: &str = "quit";

const INPUT_TEST: &str = "input_test";

impl App {
    fn new(dir: PathBuf, dir_arg: String) -> Self {
        let (w, h) = term::terminal_size();
        Self {
            dir,
            dir_arg,
            widgets: vec![],
            selected_widget: 0,
            important_widget: None,
            bus: EventBus::new(),
            buffer: buffer::Buffer::new(w, h),
            progress_active: false,
            progress_current: Arc::new(Mutex::new(0)),
            progress_max: Arc::new(Mutex::new(0)),
            duplicates_map: Arc::new(Mutex::new(HashMap::new())),
            duplicates_thread: None,
            latest_input: None,
            quit: false,
        }
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        self.go_to_first_page();

        loop {
            let (w, h) = term::terminal_size();

            if w != self.buffer.width || h != self.buffer.height {
                self.buffer.resize(w, h);
            }

            self.buffer.clear();
            self.render();

            print!("{}", self.buffer.flush());
            term::t_flush();

            // testing input
            // if self.latest_input == Some("fuck".into()) {
            //     self.quit = true;
            // }

            if self.quit || !self.handle_input() {
                break;
            }

            self.check_duplicates_thread();
            self.handle_events();
        }

        Ok(())
    }

    fn render_progress(&mut self) {
        let bw = self.buffer.width;
        let bh = self.buffer.height;

        let current_val = *self.progress_current.lock().unwrap();
        let max_val = *self.progress_max.lock().unwrap();

        let detail_string;
        let mut progress_line = String::from("");

        if max_val > 0 {
            let p = current_val * ((bw - 10 - 4) as u64) / max_val;

            progress_line = "█".repeat(p as usize);
            detail_string = format!("{}/{}", current_val, max_val);
        } else {
            detail_string = format!("{}", current_val);
        }

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

        ui::draw_box(&mut self.buffer, &r, "Working...", true);
        ui::fill_rect(
            &mut self.buffer,
            &ri,
            ' ',
            buffer::Color::Black,
            buffer::Color::Black,
        );

        self.buffer.put_str(
            r.x + 2,
            r.y + 2,
            &detail_string,
            buffer::Color::White,
            buffer::Color::Black,
        );

        if max_val > 0 {
            self.buffer.put_str(
                r.x + 2,
                r.y + 4,
                &progress_line,
                buffer::Color::Yellow,
                buffer::Color::Yellow,
            );
        }
    }

    fn render(&mut self) {
        for (i, w) in self.widgets.iter().enumerate() {
            w.draw(
                &mut self.buffer,
                !self.progress_active
                    && self.important_widget.is_none()
                    && i == self.selected_widget,
            );
        }
        if self.progress_active {
            self.render_progress();
        }
        if let Some(w) = &self.important_widget {
            w.draw(&mut self.buffer, true);
        }
    }

    fn handle_input(&mut self) -> bool {
        let k = read_key();

        if let Some(w) = self.important_widget.as_mut() {
            w.handle_input(k);

            return true;
        }

        match k {
            // TODO: this will cause issues with input dialog
            term::Key::Char('q') => return false,
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

    fn check_duplicates_thread(&mut self) {
        let finished = self
            .duplicates_thread
            .as_ref()
            .is_some_and(|h| h.is_finished());

        if finished {
            let handle = self.duplicates_thread.take().unwrap();
            if let Some(hm) = handle.join().unwrap() {
                *self.duplicates_map.lock().unwrap() = hm;
                self.bus.push(MENU_MAIN, "duplicates_ready".to_string());
            }
            self.progress_active = false;
            *self.progress_max.lock().unwrap() = 0;
            *self.progress_current.lock().unwrap() = 0;

            self.go_to_duplicates_page();
        }
    }

    fn handle_events(&mut self) {
        for event in &self.bus.drain() {
            if event.payload == ACTION_QUIT {
                self.quit = true;
                return;
            }

            let payload_str = event.payload.as_str();

            match (event.source, payload_str) {
                (MENU_MAIN, ACTION_SORT) => self.handle_sort_by_type(true),
                (MENU_MAIN, ACTION_ASK_DUPLICATES_MIN_SIZE) => {
                    self.show_input_dialogue("Input min size (in kilobytes)".into());
                }
                (MENU_MAIN, ACTION_FIND_DUPLICATES) => {
                    self.progress_active = true;

                    let p = self.dir.clone();
                    let counter = self.progress_current.clone();
                    let max = self.progress_max.clone();

                    let min_size: u64 = match self.latest_input.take() {
                        Some(txt) => {
                            txt.parse::<u64>().unwrap() * 1024 // kb
                        }
                        None => 100 * 1024, // 100 kb
                    };

                    let max_size = 1 * 1024 * 1024 * 1024; // 1gb

                    self.duplicates_thread = Some(thread::spawn(move || {
                        find_duplicates_async(&p, min_size, max_size, counter, max).ok()
                    }));
                }
                (MENU_CONFIRM_SORT, "no") | (MENU_DUPLICATES, ACTION_BACK) => {
                    self.go_to_first_page()
                }
                _ => {}
            }

            match event.source {
                INPUT_TEST => match payload_str {
                    "cancel" => {
                        self.latest_input = None;
                        self.important_widget = None;
                    }
                    _ => {
                        self.latest_input = Some(event.payload.clone());
                        self.important_widget = None;

                        self.bus.push(MENU_MAIN, ACTION_FIND_DUPLICATES.to_string());
                    }
                },
                _ => (),
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

                menu.add_item("Confirm".to_string(), ACTION_CONFIRM.to_string());
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

        menu.add_item("Sort by type".to_string(), ACTION_SORT.to_string());
        menu.add_item(
            "Find duplicates".to_string(),
            ACTION_ASK_DUPLICATES_MIN_SIZE.to_string(),
        );
        menu.add_item("Quit".to_string(), ACTION_QUIT.to_string());

        self.widgets = vec![Box::new(menu), Box::new(dir_list)];
    }

    fn go_to_duplicates_page(&mut self) {
        let dm = self.duplicates_map.lock().unwrap();

        let mut dir_files = vec![];

        let mut total_size = 0;
        let mut possible_savings = 0;

        let mut i = 1;
        for (_, files) in dm.clone() {
            for f in &files {
                total_size += f.size;
            }
            possible_savings += files[0].size * (files.len() - 1) as u64;

            let children = files
                .iter()
                .map(|f| FileTreeItem {
                    path: f.path.display().to_string(),
                    children: Vec::new(),
                })
                .collect();

            dir_files.push(FileTreeItem {
                path: format!("Group {i} | {} MB", files[0].size / 1024 / 1024),
                children,
            });

            i += 1;
        }

        let groups = i - 1;

        let title = format!(
            "{} | {} groups | {}/{} MB",
            self.dir.display().to_string(),
            groups,
            possible_savings / 1024 / 1024,
            total_size / 1024 / 1024,
        );

        let dir_list = UIFileList::new(title, dir_files, 2, |w: u16, h: u16| {
            let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
            let file_list_h = menu_y.saturating_sub(FILE_LIST_TOP + FILE_LIST_GAP);
            Rect {
                x: LEFT_MARGIN,
                y: FILE_LIST_TOP,
                w: w - LEFT_MARGIN - RIGHT_MARGIN,
                h: file_list_h,
            }
        });

        let mut menu = UIMenu::new(
            MENU_DUPLICATES,
            "Duplicates deletion".to_string(),
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

        menu.add_item("Confirm".to_string(), ACTION_CONFIRM.to_string());
        menu.add_item("Back".to_string(), ACTION_BACK.to_string());
        menu.add_item("Quit".to_string(), ACTION_QUIT.to_string());

        self.widgets = vec![Box::new(menu), Box::new(dir_list)];
    }

    fn show_input_dialogue(&mut self, title: String) {
        let input_dialogue =
            UIInputDialog::new(INPUT_TEST, title, self.bus.clone(), |bw: u16, bh: u16| {
                let h = 5;
                let w = bw - 10;

                Rect {
                    x: bw / 2 - w / 2,
                    y: bh / 2 - h / 2,
                    w: w,
                    h: h,
                }
            });

        self.important_widget = Some(Box::new(input_dialogue));
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
