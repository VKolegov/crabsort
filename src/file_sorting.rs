use std::{collections::HashMap, error::Error, fs, io, path::{Path, PathBuf}};

use crate::{file_types::{detect_file_type, type_dir}, ui::FileTreeItem};

pub fn fix_duplicates_in_dir(p: &Path, dry: bool) -> Result<Vec<FileTreeItem>, Box<dyn Error>> {
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
