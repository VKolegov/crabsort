mod buffer;
mod event_bus;
mod file_duplicates;
mod file_sorting;
mod file_types;
mod term;
mod ui;
mod widgets;

use libc::{PR_SET_PTRACER, PR_SET_PTRACER_ANY, prctl};

use crate::ui::Rect;
use crate::widgets::{FileTreeItem, UIGroupedList, UIGroupedListItem, UIStatusBar};
use crate::{
    event_bus::EventBus,
    file_duplicates::{FileInfo, find_duplicates_async},
    file_sorting::{fix_duplicates_in_dir, move_files_with_progress},
    term::read_key,
    widgets::{UIFileList, UIInputDialog, UIMenu, UIProgressBar, Widget},
};
use std::cell::RefCell;
use std::rc::Rc;
use std::{
    collections::HashMap,
    env,
    error::Error,
    fs::{self},
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

static DIR_LIST_SIZE: fn(u16, u16) -> Rect = |w: u16, h: u16| {
    let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
    let file_list_h = menu_y.saturating_sub(FILE_LIST_TOP + FILE_LIST_GAP);
    Rect {
        x: LEFT_MARGIN,
        y: FILE_LIST_TOP,
        w: w - LEFT_MARGIN - RIGHT_MARGIN,
        h: file_list_h,
    }
};

static MAIN_MENU_SIZE: fn(u16, u16) -> Rect = |w: u16, h: u16| {
    let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
    Rect {
        x: LEFT_MARGIN,
        y: menu_y,
        w: w - LEFT_MARGIN - RIGHT_MARGIN,
        h: MENU_HEIGHT,
    }
};

static PROGRESS_BAR_SIZE: fn(u16, u16) -> Rect = |bw: u16, bh: u16| {
    let h = 7;
    let w = bw - 10;
    Rect {
        x: bw / 2 - w / 2,
        y: bh / 2 - h / 2,
        w,
        h,
    }
};

struct App {
    dir: PathBuf,
    buffer: buffer::Buffer,

    widgets: Vec<Box<dyn Widget>>,
    selected_widget: usize,
    bus: EventBus,

    progress_current: Arc<Mutex<u64>>,
    progress_max: Arc<Mutex<u64>>,

    latest_input: Option<String>,
    important_widget: Option<Box<dyn Widget>>,

    duplicates_map: Arc<Mutex<HashMap<String, Vec<Arc<FileInfo>>>>>,
    duplicates_thread: Option<JoinHandle<Option<HashMap<String, Vec<Arc<FileInfo>>>>>>,
    sort_thread: Option<JoinHandle<Option<Vec<UIGroupedListItem>>>>,
    sort_selected: Rc<RefCell<Vec<UIGroupedListItem>>>,

    status_bar: UIStatusBar,

    quit: bool,
}

const MENU_MAIN: &str = "main_menu";
const ACTION_SORT: &str = "sort";
const ACTION_ASK_DUPLICATES_MIN_SIZE: &str = "duplicates_min_size";

const MENU_CONFIRM_SORT: &str = "confirm_sort_menu";
const MENU_SORT_SUCCESS: &str = "sort_success_menu";
const MENU_DUPLICATES: &str = "duplicates_menu";

const ACTION_CONFIRM: &str = "confirm";
const ACTION_BACK: &str = "back";
const ACTION_QUIT: &str = "quit";

const INPUT_DIALOG: &str = "input_test";

fn split_supported_unsupported(
    items: Vec<UIGroupedListItem>,
) -> (Vec<UIGroupedListItem>, Vec<UIGroupedListItem>) {
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();
    for item in items {
        if item.title == "unsupported" {
            unsupported.push(item);
        } else {
            supported.push(item);
        }
    }
    (supported, unsupported)
}

impl App {
    fn new(dir: PathBuf) -> Self {
        let status_bar = UIStatusBar::new(
            " Tab:switch focus | j/k:navigate | Space:toggle selection | q:quit".to_string(),
            |w: u16, h: u16| Rect {
                x: 0,
                y: h - 1,
                w: w,
                h: 1,
            },
        );

        Self {
            dir,
            widgets: vec![],
            selected_widget: 0,
            important_widget: None,
            bus: EventBus::new(),
            buffer: buffer::Buffer::new(0, 0),
            progress_current: Arc::new(Mutex::new(0)),
            progress_max: Arc::new(Mutex::new(0)),
            duplicates_map: Arc::new(Mutex::new(HashMap::new())),
            duplicates_thread: None,
            sort_thread: None,
            latest_input: None,
            sort_selected: Rc::new(RefCell::new(Vec::new())),
            status_bar,
            quit: false,
        }
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        self.go_to_first_page();

        loop {
            let (w, h) = term::terminal_size();

            if w != self.buffer.width || h != self.buffer.height {
                self.buffer.resize(w, h);
                self.handle_resize(w, h);
            }

            self.buffer.clear();
            self.render();

            print!("{}", self.buffer.flush());
            term::t_flush();

            if self.quit || !self.handle_input() {
                break;
            }

            self.check_duplicates_thread();
            self.check_sort_thread();
            self.handle_events();
        }

        Ok(())
    }

    fn handle_resize(&mut self, w: u16, h: u16) {
        for widget in self.widgets.iter_mut() {
            widget.handle_buf_size_change(w, h);
        }
        if let Some(important_w) = self.important_widget.as_mut() {
            important_w.handle_buf_size_change(w, h);
        }
        self.status_bar.handle_buf_size_change(w, h);
    }

    fn render(&mut self) {
        for (i, w) in self.widgets.iter_mut().enumerate() {
            w.draw(
                &mut self.buffer,
                self.important_widget.is_none() && i == self.selected_widget,
            );
        }
        let mut has_important_widget = false;
        if let Some(w) = self.important_widget.as_mut() {
            w.draw(&mut self.buffer, true);
            has_important_widget = true;
        }
        self.status_bar
            .draw(&mut self.buffer, !has_important_widget);
    }

    fn handle_input(&mut self) -> bool {
        let k = read_key();

        if let Some(w) = self.important_widget.as_mut() {
            w.handle_input(k);

            return true;
        }

        match k {
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

        self.widgets[self.selected_widget].handle_input(k);

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
            self.important_widget = None;
            *self.progress_max.lock().unwrap() = 0;
            *self.progress_current.lock().unwrap() = 0;

            self.go_to_duplicates_page();
        }
    }

    fn check_sort_thread(&mut self) {
        let finished = self.sort_thread.as_ref().is_some_and(|h| h.is_finished());

        if finished {
            let handle = self.sort_thread.take().unwrap();
            if let Some(files) = handle.join().unwrap() {
                self.go_to_sort_success_page(files);
            } else {
                self.go_to_first_page();
            }
            self.important_widget = None;
            *self.progress_max.lock().unwrap() = 0;
            *self.progress_current.lock().unwrap() = 0;
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
                (MENU_CONFIRM_SORT, ACTION_CONFIRM) => self.handle_confirm_sort(),
                (MENU_CONFIRM_SORT, "no")
                | (MENU_SORT_SUCCESS, ACTION_BACK)
                | (MENU_DUPLICATES, ACTION_BACK) => {
                    self.sort_selected.borrow_mut().clear();
                    self.go_to_first_page()
                }
                _ => {}
            }

            match event.source {
                INPUT_DIALOG => match payload_str {
                    "cancel" => {
                        self.latest_input = None;
                        self.important_widget = None;
                    }
                    _ => {
                        self.latest_input = Some(event.payload.clone());
                        self.important_widget = None;

                        self.handle_find_duplicates();
                    }
                },
                _ => (),
            }
        }
    }

    fn handle_confirm_sort(&mut self) {
        let plan = self.sort_selected.borrow().to_vec();

        let desc = Arc::new(Mutex::new("Moving files...".to_string()));
        let progress_desc = Arc::new(Mutex::new(String::new()));

        let progress_bar = UIProgressBar::new(
            desc.clone(),
            Some(progress_desc.clone()),
            self.progress_current.clone(),
            self.progress_max.clone(),
            PROGRESS_BAR_SIZE,
        );
        self.important_widget = Some(Box::new(progress_bar));

        let counter = self.progress_current.clone();
        let max = self.progress_max.clone();

        self.sort_thread = Some(thread::spawn(move || {
            move_files_with_progress(plan, counter, max, progress_desc).ok()
        }));
    }

    fn handle_sort_by_type(&mut self, dry: bool) {
        match fix_duplicates_in_dir(&self.dir, dry) {
            Ok(files) => {
                if dry {
                    *self.sort_selected.borrow_mut() = files.clone();
                }
                let (supported, unsupported) = split_supported_unsupported(files);

                let supported_list = UIGroupedList::new(
                    format!("{} | Files to move", self.dir.display()),
                    supported,
                    Some(self.sort_selected.clone()),
                    true,
                    |w: u16, h: u16| {
                        let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
                        let available = menu_y.saturating_sub(FILE_LIST_TOP + FILE_LIST_GAP);
                        let supported_h = available / 2;
                        Rect {
                            x: LEFT_MARGIN,
                            y: FILE_LIST_TOP,
                            w: w - LEFT_MARGIN - RIGHT_MARGIN,
                            h: supported_h,
                        }
                    },
                );

                let unsupported_list = UIGroupedList::new(
                    "Unsupported (not moved)".to_string(),
                    unsupported,
                    None,
                    false,
                    |w: u16, h: u16| {
                        let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
                        let available = menu_y.saturating_sub(FILE_LIST_TOP + FILE_LIST_GAP);
                        let supported_h = available / 2;
                        let unsupported_h = available.saturating_sub(supported_h + FILE_LIST_GAP);
                        Rect {
                            x: LEFT_MARGIN,
                            y: FILE_LIST_TOP + supported_h + FILE_LIST_GAP,
                            w: w - LEFT_MARGIN - RIGHT_MARGIN,
                            h: unsupported_h,
                        }
                    },
                );

                let mut menu = UIMenu::new(
                    MENU_CONFIRM_SORT,
                    "Confirm action".to_string(),
                    vec![],
                    self.bus.clone(),
                    MAIN_MENU_SIZE,
                );

                menu.add_item("Confirm".to_string(), ACTION_CONFIRM.to_string());
                menu.add_item("Cancel".to_string(), "no".to_string());

                self.widgets = vec![
                    Box::new(menu),
                    Box::new(supported_list),
                    Box::new(unsupported_list),
                ];
                self.selected_widget = 1;
            }
            Err(_) => (),
        }
    }

    fn handle_find_duplicates(&mut self) {
        let desc = Arc::new(Mutex::new(String::new()));
        let progress_desc = Arc::new(Mutex::new(String::new()));

        let progress_bar = UIProgressBar::new(
            desc.clone(),
            Some(progress_desc.clone()),
            self.progress_current.clone(),
            self.progress_max.clone(),
            PROGRESS_BAR_SIZE,
        );
        self.important_widget = Some(Box::new(progress_bar));

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

        let dc = desc.clone();
        let progress_dc = progress_desc.clone();
        self.duplicates_thread = Some(thread::spawn(move || {
            find_duplicates_async(&p, min_size, max_size, dc, progress_dc, counter, max).ok()
        }));
    }

    fn go_to_first_page(&mut self) {
        let dir_files = read_dir_files(&self.dir);

        let dir_list = UIFileList::new(
            self.dir.display().to_string(),
            dir_files,
            2,
            None,
            false,
            DIR_LIST_SIZE,
        );

        let mut menu = UIMenu::new(
            MENU_MAIN,
            "crabsort".to_string(),
            vec![],
            self.bus.clone(),
            MAIN_MENU_SIZE,
        );

        menu.add_item("Sort by type".to_string(), ACTION_SORT.to_string());
        menu.add_item(
            "Find duplicates".to_string(),
            ACTION_ASK_DUPLICATES_MIN_SIZE.to_string(),
        );
        menu.add_item("Quit".to_string(), ACTION_QUIT.to_string());

        self.widgets = vec![Box::new(menu), Box::new(dir_list)];
    }

    fn go_to_sort_success_page(&mut self, files: Vec<UIGroupedListItem>) {
        let (supported, unsupported) = split_supported_unsupported(files);
        let count: usize = supported.iter().map(|item| item.children.len()).sum();

        let supported_list = UIGroupedList::new(
            format!("Moved {} files", count),
            supported,
            None,
            false,
            |w: u16, h: u16| {
                let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
                let available = menu_y.saturating_sub(FILE_LIST_TOP + FILE_LIST_GAP);
                let supported_h = available / 2;
                Rect {
                    x: LEFT_MARGIN,
                    y: FILE_LIST_TOP,
                    w: w - LEFT_MARGIN - RIGHT_MARGIN,
                    h: supported_h,
                }
            },
        );

        let unsupported_list = UIGroupedList::new(
            "Unsupported (not moved)".to_string(),
            unsupported,
            None,
            false,
            |w: u16, h: u16| {
                let menu_y = h - MENU_MARGIN_BOTTOM - MENU_HEIGHT;
                let available = menu_y.saturating_sub(FILE_LIST_TOP + FILE_LIST_GAP);
                let supported_h = available / 2;
                let unsupported_h = available.saturating_sub(supported_h + FILE_LIST_GAP);
                Rect {
                    x: LEFT_MARGIN,
                    y: FILE_LIST_TOP + supported_h + FILE_LIST_GAP,
                    w: w - LEFT_MARGIN - RIGHT_MARGIN,
                    h: unsupported_h,
                }
            },
        );

        let mut menu = UIMenu::new(
            MENU_SORT_SUCCESS,
            format!("Done! Moved {} files.", count),
            vec![],
            self.bus.clone(),
            MAIN_MENU_SIZE,
        );

        menu.add_item("Back".to_string(), ACTION_BACK.to_string());

        self.widgets = vec![
            Box::new(menu),
            Box::new(supported_list),
            Box::new(unsupported_list),
        ];
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

        // TODO: select oldest
        let dir_list = UIFileList::new(title, dir_files, 2, None, false, DIR_LIST_SIZE);

        let mut menu = UIMenu::new(
            MENU_DUPLICATES,
            "Duplicates deletion".to_string(),
            vec![],
            self.bus.clone(),
            MAIN_MENU_SIZE,
        );

        menu.add_item("Confirm".to_string(), ACTION_CONFIRM.to_string());
        menu.add_item("Back".to_string(), ACTION_BACK.to_string());
        menu.add_item("Quit".to_string(), ACTION_QUIT.to_string());

        self.widgets = vec![Box::new(menu), Box::new(dir_list)];
        self.selected_widget = 1;
    }

    fn show_input_dialogue(&mut self, title: String) {
        let input_dialogue = UIInputDialog::new(
            INPUT_DIALOG,
            title,
            Some(String::from("1024")),
            self.bus.clone(),
            |bw: u16, bh: u16| {
                let h = 5;
                let w = bw - 10;

                Rect {
                    x: bw / 2 - w / 2,
                    y: bh / 2 - h / 2,
                    w: w,
                    h: h,
                }
            },
        );

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
    unsafe {
        prctl(PR_SET_PTRACER, PR_SET_PTRACER_ANY);
    }

    let (dir, _) = parse_args()?;

    term::enable_raw_mode();
    term::enter_alternate_screen();
    term::hide_cursor();

    let mut app = App::new(dir);
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
