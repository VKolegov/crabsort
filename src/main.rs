use std::{env, fs, io, path::{Path, PathBuf}, process};

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

    let first_arg = env::args()
        .nth(1)
        .ok_or("directory argument required")?;

    let path = PathBuf::from(first_arg);

    if !path.exists() {
        return Err("path does not exist".into())
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

        println!("str: {}", path.display().to_string());

    }

    Ok(())
}
