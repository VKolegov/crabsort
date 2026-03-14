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
    let dir = get_directory()?;

    traverse_dir(&dir);

    Ok(())
}

fn get_directory() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let first_arg = env::args().nth(1).ok_or("directory argument required")?;

    let path = PathBuf::from(first_arg);

    if !path.exists() {
        return Err("path does not exist".into());
    }

    if !path.is_dir() {
        return Err("path is not a directory".into());
    }

    Ok(path)
}

fn traverse_dir(p: &Path) -> io::Result<()> {
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
                    println!("dir: {}", full_path.display().to_string());
                    ensure_dir(&full_path.display().to_string());
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
