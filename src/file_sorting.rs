use std::{
    collections::HashMap,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

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

/// Move files using pre-collected plan. No dir traversal.
pub fn move_files_with_progress(
    plan: Vec<FileTreeItem>,
    progress: Arc<Mutex<u64>>,
    max: Arc<Mutex<u64>>,
    description: Arc<Mutex<String>>,
) -> Result<Vec<FileTreeItem>, Box<dyn Error>> {
    let total: u64 = plan
        .iter()
        .filter(|item| item.path != "unsupported")
        .map(|item| item.children.len() as u64)
        .sum();
    *max.lock().unwrap() = total;
    *progress.lock().unwrap() = 0;

    for item in &plan {
        if item.path == "unsupported" {
            continue;
        }
        let target_dir = &item.path;
        ensure_dir(target_dir)?;

        for child in &item.children {
            let source = PathBuf::from(&child.path);
            let filename = source
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let new_path = Path::new(target_dir).join(filename);

            *description.lock().unwrap() = child.path.clone();

            if let Err(e) = fs::copy(&source, &new_path) {
                eprintln!(
                    "Failed to copy {} -> {}: {}",
                    source.display(),
                    new_path.display(),
                    e
                );
            } else if let Err(e) = fs::remove_file(&source) {
                eprintln!("Failed to remove {}: {}", source.display(), e);
            }

            *progress.lock().unwrap() += 1;
        }
    }

    Ok(plan)
}

fn traverse_dir(p: &Path, dry: bool, verbose: bool) -> Result<HashMap<String, Vec<String>>, Box<dyn Error>> {
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
            Err(_) => {
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
