use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
    io::{self, Read, Write},
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
    let mut files_count_for_partial_hash: u64 = 0;

    // processing groups
    let mut possible_duplicates = Vec::new();
    for (key, v) in &files_by_sizes {
        let l = v.len();
        if l > 2 {
            possible_duplicates.push(key);
            if verbose {
                println!("{} -> {}", key, l);
            }
            files_count_for_partial_hash += l as u64;
        }
    }

    println!(
        "possible duplicate groups by size: {}",
        possible_duplicates.len()
    );

    // step 2 - partial hash
    let mut partial_hash_map: HashMap<String, Vec<&FileInfo>> = HashMap::new();
    let mut file_hash_processed: u64 = 0;

    for ele in possible_duplicates {
        let group = files_by_sizes.get(ele).unwrap();

        for file_d in group {
            let hash = match file_fast_hash(&file_d.path) {
                Ok(h) => h,
                Err(_) => {
                    continue;
                }
            };

            file_hash_processed += 1;

            print_progress("hash processed", &file_hash_processed, &files_count_for_partial_hash)?;


            if let Some(v) = partial_hash_map.get_mut(&hash) {
                v.push(file_d);
            } else {
                partial_hash_map.insert(hash, vec![file_d]);
            }
        }
    }

    println!("");
    
    // processing groups
    let mut possible_duplicates = Vec::new();
    for (key, v) in &partial_hash_map {
        let l = v.len();
        if l > 2 {
            possible_duplicates.push(key);
            if verbose {
                println!("{} -> {}", key, l);
            }
        }
    }
    println!(
        "possible duplicate groups by partial hash: {}",
        possible_duplicates.len()
    );

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

        let m = match e.metadata() {
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

        print_progress("files_read", files_read, &0)?;
    }

    Ok(dir_map)
}

pub fn find_same_size_files_recursive(
    p: &Path,
) -> Result<HashMap<u64, Vec<FileInfo>>, Box<dyn Error>> {
    let map = build_dir_flatmap(p, &mut 0)?;
    println!("");

    let mut total_size = 0;

    // group by size
    let mut files_by_sizes: HashMap<u64, Vec<FileInfo>> = HashMap::new();
    for ele in map {
        if let Some(v) = files_by_sizes.get_mut(&ele.size) {
            total_size += ele.size;
            v.push(ele);
        } else {
            files_by_sizes.insert(ele.size, vec![ele]);
        }
    }

    println!("total possible duplicate size: {} kb", total_size / 1024);

    Ok(files_by_sizes)
}

fn file_fast_hash(p: &PathBuf) -> Result<String, Box<dyn Error>> {
    let mut f = File::open(p)?;

    let mut file_buff = [0u8; 8 * 1024]; //8 kb max

    _ = f.read(&mut file_buff)?;

    let hash = md5::compute(file_buff);

    Ok(format!("{:x}", hash))
}

fn print_progress(metric: &str, c: &u64, t: &u64) -> Result<(), Box<dyn Error>> {
    if *c % 100 == 0 {
        print!("\r{}: {}/{}", metric, c, t);
        io::stdout().flush()?;
    }
    Ok(())
}
