mod file_types;

use crate::file_types::{detect_file_type, type_dir};
use std::{
    env, error::Error, fs::{self}, io::{self}, path::{Path, PathBuf}, process
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

    for arg in &args[1..] {
        if arg == "--dry" {
            println!("[warn] dry run");
            dry_run = true;
        }

        if !arg.starts_with("--") {
            dir_arg = arg;
        }
    }

    if dir_arg == "" {
        return Err("directory argument required".into());
    }

    let dir = get_directory(&dir_arg)?;

    traverse_dir(&dir, &dry_run)?;

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

fn traverse_dir(p: &Path, dry: &bool) -> Result<(), Box<dyn Error>> {
    if !p.is_dir() {
        return Ok(());
    }

    let r_dir = fs::read_dir(p)?;

    for entry in r_dir {
        let e = entry?;
        let path = e.path();

        if path.is_dir() {
            continue;
        }

        let file_type = if let Ok(ft) = detect_file_type(&path) {
            ft
        } else {
            println!("Unsupported file type: {}", path.display());
            continue;
        };

        let paths: Option<(PathBuf, PathBuf)> = type_dir(file_type)
            .and_then(|target_dir| {
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

        println!("{} -> {}", full_path.display(), new_path.display());

        if *dry {
            println!("dry run - skip");
            continue;
        }

        let full_path_str = full_path
            .to_str()
            .ok_or("wrong target dir name")?;

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
