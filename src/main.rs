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
Usage: crabsort <command> [options] <directory>

Commands:
  sort    Sort files into subdirectories by type (dry run by default)
  clean   Find duplicate files

Options:
  --execute   Actually perform file operations (sort: move files, clean: TBD)
  --verbose   Show detailed output
";

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprint!("{}", USAGE);
        return Err("dir required".into());
    }

    let command = args[1].as_str();
    if command == "--help" || command == "-h" {
        print!("{}", USAGE);
        return Ok(());
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

    term::enable_raw_mode();
    term::enter_alternate_screen();
    term::hide_cursor();

    /* main state */
    let mut selected_n: usize = 0;
    let mut mode: usize = 0;
    let mut sort_stage: usize = 0;

    let mut sortable: Option<HashMap<String, i32>> = None;
    /* /main state */

    let (w, h) = term::terminal_size();

    let mut b = buffer::Buffer::new(w, h);

    let lm = 2;
    let rm = 2;

    let menu_h = 6;
    let menu_mb = 1;

    let menu_items = vec![
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
    ];

    loop {
        let (w, h) = term::terminal_size();

        if w != b.width || h != b.height {
            b.resize(w, h);
        }

        b.clear();

        match mode {
            0 => {
                let menu_rect = &Rect {
                    x: lm,
                    y: h - menu_mb - menu_h,
                    w: w - lm - rm,
                    h: menu_h,
                };
                ui::draw_menu(&mut b, &menu_rect, "crabsort", &menu_items, selected_n);
            }
            1 => {
                let sort_rect = &Rect {
                    x: lm,
                    y: h - menu_mb - 12,
                    w: w - lm - rm,
                    h: 12,
                };
                ui::draw_box(&mut b, &sort_rect, dir_arg, true);

                if let Some(ref h) = sortable {
                    let mut i = 0;
                    for (k, v) in h {
                        let s = format!("{}: {}", k, v);
                        b.put_str(sort_rect.x + 3, sort_rect.y + 1 + i, &s, buffer::Color::White, buffer::Color::Reset);
                        i += 1;
                    }
                }
            }
            _ => (),
        }

        print!("{}", b.flush());
        term::t_flush();

        match read_key() {
            term::Key::Char('k') => {
                selected_n = match selected_n > 0 {
                    true => selected_n - 1,
                    false => selected_n,
                };
            }
            term::Key::Char('j') => {
                selected_n = match selected_n < menu_items.len() - 1 {
                    true => selected_n + 1,
                    false => selected_n,
                };
            }
            term::Key::Char('q') => break,
            term::Key::Enter => match mode {
                0 => match selected_n {
                    0 => {
                        sortable = traverse_dir(&dir, true, false).ok();
                        mode = 1;
                    }
                    2 => break,
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        }
    }

    term::exit_alternate_screen();
    term::disable_raw_mode();
    term::show_cursor();

    Ok(())
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
) -> Result<HashMap<String, i32>, Box<dyn Error>> {
    if !p.is_dir() {
        return Err("not a dir".into());
    }

    let r_dir = fs::read_dir(p)?;
    let mut count_map: HashMap<String, i32> = HashMap::new();

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

                if let Some(current_count) = count_map.get("unsupported") {
                    count_map.insert("unsupported".to_string(), current_count + 1);
                } else {
                    count_map.insert("unsupported".to_string(), 1);
                }

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

        if let Some(current_count) = count_map.get(full_path_str) {
            count_map.insert(full_path_str.to_string(), current_count + 1);
        } else {
            count_map.insert(full_path_str.to_string(), 1);
        }

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

    Ok(count_map)
}

fn ensure_dir(p: &str) -> Result<(), io::Error> {
    let exists = fs::exists(p)?;

    if exists {
        return Ok(());
    }

    fs::create_dir_all(p)?;

    Ok(())
}
