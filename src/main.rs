mod file_types;
mod file_duplicates;

use crate::{file_duplicates::find_duplicates, file_types::{detect_file_type, type_dir}};
use std::{ collections::HashMap, env,
    error::Error,
    fs::{self},
    io::{self},
    path::{Path, PathBuf},
    process,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    // 0 - program name
    let mut dry_run = false;
    let mut dir_arg = "";
    let mut verbose = false;
    let mut duplicates_search = false;

    for arg in &args[1..] {
        if arg == "--dry" {
            println!("[warn] dry run");
            dry_run = true;
        }
        if arg == "--verbose" {
            verbose = true;
        }
        if arg == "-d" {
            duplicates_search = true;
        }

        if !arg.starts_with("-") {
            dir_arg = arg;
        }
    }

    if dir_arg == "" {
        return Err("directory argument required".into());
    }

    let dir = get_directory(&dir_arg)?;


    if duplicates_search {
        find_duplicates(&dir, dry_run, verbose)?;
    } else {
        traverse_dir(&dir, &dry_run, &verbose)?;
    }

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

fn traverse_dir(p: &Path, dry: &bool, verbose: &bool) -> Result<(), Box<dyn Error>> {
    if !p.is_dir() {
        return Ok(());
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
                println!("unsupported file {} : {}", path.display(), e);

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


        if *verbose {
            println!("{} -> {}", path.display(), new_path.display());
        }

        if *dry {
            if *verbose {
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

    for (key, value) in &count_map {
        println!("{} => {}", key, value);
    }

    Ok(())
}

fn ensure_dir(p: &str) -> Result<(), io::Error> {
    let exists = fs::exists(p)?;

    if exists {
        return Ok(());
    }

    fs::create_dir_all(p)?;

    Ok(())
}
