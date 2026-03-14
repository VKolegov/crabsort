mod file_types;

use std::{
    env,
    fs::{self},
    io::{self},
    path::{Path, PathBuf},
    process,
};
use crate::file_types::{detect_file_type, type_dir};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        process::exit(1); } }

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

        // if path.is_dir() {
        //     continue;
        // }
        //
        // let mut f = match File::open(&path) {
        //     Ok(file) => file,
        //     Err(e) => {
        //         eprintln!("Failed to read file: {}", e);
        //         continue;
        //     }
        // };
        //
        // let path_str = path.display().to_string();
        //
        // let mut file_buff = [0u8; 512];
        // let n = match f.read(&mut file_buff) {
        //     Ok(n) => n,
        //     Err(e) => {
        //         eprintln!("Error while reading {}: {}", path_str, e);
        //         continue;
        //     }
        // };
        //



        // if let Some(kind) = infer::get(&file_buff[..n]) {
        //     if let Some(file_type) = calculate_file_type(kind.mime_type(), kind.extension()) {
        //         println!("file: {}, mime: {}, type: {:?}", path_str, kind.mime_type(), file_type);
        //         if let Some(d) = type_dir(&file_type) {
        //             let full_path = p.join(d);
        //             println!("dir: {}", full_path.display().to_string());
        //             ensure_dir(&full_path.display().to_string());
        //         }
        //     } else {
        //         println!(
        //             "unknown type, file: {}, mime: {}, ext: {}",
        //             path_str,
        //             kind.mime_type(),
        //             kind.extension()
        //         );
        //     }
        // }
        //

        let path_str = path.display().to_string();

        if let Some(file_type) = detect_file_type(p) {
            println!("file: {}, type: {:?}", path_str, file_type);
            if let Some(d) = type_dir(&file_type) {
                let full_path = p.join(d);
                println!("dir: {}", full_path.display().to_string());
                ensure_dir(&full_path.display().to_string());
            }
        } else {
            println!(
                "unknown type, file: {}",
                path_str,
            );
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
