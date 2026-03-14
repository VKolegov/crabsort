mod file_types;

use crate::file_types::{detect_file_type, type_dir};
use std::{
    env,
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
    let first_arg = args.get(1).ok_or("directory argument required")?;
    let mut dry_run = false;

    for arg in &args {
        if arg == "--dry" {
            println!("[warn] dry run");
            dry_run = true;
        }
    }

    let dir = get_directory(&first_arg)?;


    traverse_dir(&dir, &dry_run);

    Ok(())
}

fn get_directory(s: &String) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = PathBuf::from(s);

    if !path.exists() {
        return Err("path does not exist".into());
    }

    if !path.is_dir() {
        return Err("path is not a directory".into());
    }

    Ok(path)
}

fn traverse_dir(p: &Path, dry: &bool) -> io::Result<()> {
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

        let path_str = path.display().to_string();

        match detect_file_type(&path) {
            Ok(file_type) => {
                // println!("file: {}, type: {:?}", path_str, file_type);
                if let Some(d) = type_dir(&file_type) {
                    let full_path = p.join(d);
                    let filename = path.file_name().unwrap().display().to_string();
                    let new_path = full_path.join(filename);
                    ensure_dir(&full_path.display().to_string());
                    println!("{} -> {}", path_str, new_path.display().to_string());

                    if *dry {
                        println!("dry run - skip");
                        continue;
                    }

                    if let Err(e) = fs::copy(&path, &new_path) {
                        eprintln!("Failed to copy {} -> {}: {}", path.display(), new_path.display(), e);
                    } else {
                        if let Err(e) = fs::remove_file(&path) {
                            eprintln!("Failed to remove {}: {}", path.display(), e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("{}: detect file type error: {}", path_str, e)
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
