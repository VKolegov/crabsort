use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub struct FileInfo {
    path: PathBuf,
    size: u64,
}

pub fn find_duplicates(p: &Path, dry: bool, verbose: bool) -> Result<(), Box<dyn Error>> {
    if !p.is_dir() {
        return Ok(());
    }

    let files_by_sizes = find_same_size_files_recursive(p)?;

    // processing groups
    let mut possible_duplicates = Vec::new();
    for (key, v) in &files_by_sizes {
        let l = v.len();
        if l > 2 {
            possible_duplicates.push(key);
            if verbose {
                println!("{} -> {}", key, l);
            }
        }
    }

    println!(
        "possible duplicate groups by size: {}",
        possible_duplicates.len()
    );

    for ele in possible_duplicates {
        let group = files_by_sizes.get(ele).unwrap();

        for file_d in group {
            if verbose {
                println!("{}: {}", ele, file_d.display());
            }
        }
    }

    Ok(())
}

pub fn build_dir_flatmap(p: &Path, files_read: &mut u64) -> Result<Vec<FileInfo>, Box<dyn Error>> {
    if !p.is_dir() {
        return Err("not a dir".into());
    }

    let mut dir_map: Vec<FileInfo> = Vec::new();

    let r_dir = fs::read_dir(p)?;

    for entry in r_dir {
        let e = entry?;
        let path = e.path();

        if path.is_dir() {
            let mut child_dir_map = match build_dir_flatmap(&path, files_read) {
                Ok(cdm) => cdm,
                Err(_) => {
                    continue;
                }
            };

            dir_map.append(&mut child_dir_map);

            continue;
        }

        if !path.is_file() || path.is_symlink() {
            continue;
        }

        let m = match path.metadata() {
            Ok(meta) => meta,
            Err(_) => {
                continue;
            }
        };

        let file_size = m.len();

        if file_size == 0 {
            continue;
        }

        dir_map.push(FileInfo {
            path: path,
            size: file_size,
        });

        *files_read += 1;

        if *files_read % 100 == 0 {
            print!("\rfiles read: {}", files_read);
            io::stdout().flush()?;
        }
    }

    Ok(dir_map)
}

pub fn find_same_size_files_recursive(
    p: &Path,
) -> Result<HashMap<u64, Vec<PathBuf>>, Box<dyn Error>> {
    let map = build_dir_flatmap(p, &mut 0)?;
    println!("");

    let mut total_size = 0;

    // group by size
    let mut files_by_sizes: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for ele in map {
        if let Some(v) = files_by_sizes.get_mut(&ele.size) {
            v.push(ele.path);
            total_size += ele.size;
        } else {
            files_by_sizes.insert(ele.size, vec![ele.path]);
        }
    }

    println!("total possible duplicate size: {} kb", total_size / 1024);

    Ok(files_by_sizes)
}
