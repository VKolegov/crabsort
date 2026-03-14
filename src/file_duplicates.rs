use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};

pub struct FileInfo {
    path: PathBuf,
    size: u64,
    first_and_last_4kb: [u8; 8192],
}

pub fn find_duplicates(p: &Path, dry: bool, verbose: bool) -> Result<(), Box<dyn Error>> {
    if !p.is_dir() {
        return Ok(());
    }

    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1); // fallback на 1

    println!("available threads: {}", n_threads);

    let mut files_by_sizes = find_same_size_files_recursive(p)?;

    let mut files_count_for_partial_hash: u64 = 0;
    let mut files_count_for_full_hash: u64 = 0;

    let duplicate_groups_filtered = &mut files_by_sizes;

    duplicate_groups_filtered.retain(|_key, v| {
        let l = v.len();
        if l >= 2 {
            files_count_for_partial_hash += l as u64;
            true
        } else {
            false
        }
    });

    println!(
        "possible duplicate groups by size: {}",
        duplicate_groups_filtered.keys().count()
    );

    println!("hash to count: {}", files_count_for_partial_hash);

    // step 2 - partial hash
    let mut file_hash_processed: u64 = 0;
    let mut partial_hash_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();

    for (_, file_vec) in duplicate_groups_filtered {
        let arc_file_vec = file_vec
            .iter()
            .map(|fv| {
                Arc::from(FileInfo {
                    path: fv.path.clone(),
                    size: fv.size,
                    first_and_last_4kb: fv.first_and_last_4kb,
                })
            })
            .collect();
        let mut size_group_partial_hash_map = process_group_partial_hash(arc_file_vec, n_threads);

        file_hash_processed += file_vec.len() as u64;

        print_progress("1 step groups processed", file_hash_processed, files_count_for_partial_hash)?;

        // filtering groups within size groups
        size_group_partial_hash_map.retain(|_k, v| {
            let l = v.len();

            if l >= 2 {
                files_count_for_full_hash += l as u64;
                true
            } else {
                false
            }
        });

        for (k, v) in size_group_partial_hash_map.iter_mut() {
            partial_hash_map
                .entry(k.to_string())
                .or_default()
                .extend(v.clone());
            // if partial_hash_map.get(k).is_none() {
            //     partial_hash_map.insert(k.to_string(), v.to_vec());
            // }
        }
    }

    println!("");

    let partial_hash_groups = partial_hash_map.keys().count();

    println!(
        "possible duplicate groups by partial hash: {}",
        partial_hash_groups,
    );

    file_hash_processed = 0;
    // step 3
    let mut full_hash_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();
    for (_, file_vec) in partial_hash_map {
        let mut phash_group_full_hash_map = process_group_full_hash(file_vec.clone(), n_threads);

        file_hash_processed += file_vec.len() as u64;

        print_progress("full hash processed", file_hash_processed, files_count_for_full_hash)?;

        // filtering groups within size groups
        phash_group_full_hash_map.retain(|_k, v| v.len() >= 2);

        for (k, v) in phash_group_full_hash_map {
            if full_hash_map.get(&k).is_none() {
                full_hash_map.insert(k.to_string(), v.to_vec());
            }
        }
    }

    println!("");

    println!(
        "possible duplicate groups by full hash: {}",
        full_hash_map.keys().count(),
    );

    for (_, file_vec) in full_hash_map {
        for file_d in file_vec {
            println!("{}", file_d.path.display());
        }
        println!("-----");
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

        let m = match e.metadata() {
            Ok(meta) => meta,
            Err(_) => {
                continue;
            }
        };

        let file_size = m.len();

        // < 100 kb
        if file_size < 1024 * 100 {
            continue;
        }

        // > 1gb
        if file_size > 1024 * 1024 * 1024 {
            continue;
        }

        let f_a_l_4kb = match read_first_and_last_4kb(&path, file_size) {
            Ok(buf) => buf,
            Err(_) => continue,
        };

        dir_map.push(FileInfo {
            path: path,
            size: file_size,
            first_and_last_4kb: f_a_l_4kb,
        });

        *files_read += 1;

        print_progress("files_read", *files_read, 0)?;
    }

    Ok(dir_map)
}

pub fn find_same_size_files_recursive(
    p: &Path,
) -> Result<HashMap<u64, Vec<FileInfo>>, Box<dyn Error>> {
    let map = build_dir_flatmap(p, &mut 0)?;
    println!("");

    // group by size
    let mut files_by_sizes: HashMap<u64, Vec<FileInfo>> = HashMap::new();
    for ele in map {
        if let Some(v) = files_by_sizes.get_mut(&ele.size) {
            v.push(ele);
        } else {
            files_by_sizes.insert(ele.size, vec![ele]);
        }
    }

    Ok(files_by_sizes)
}

fn file_hash(path: &Path) -> Result<String, Box<dyn Error>> {
    use blake3::Hasher;

    let mut f = File::open(path)?;
    let mut hasher = Hasher::new();
    let mut buffer = [0u8; 8192];

    loop {
        let n = f.read(&mut buffer)?;
        if n == 0 { break; }
        hasher.update(&buffer[..n]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

fn print_progress(metric: &str, c: u64, t: u64) -> Result<(), Box<dyn Error>> {
    if (t < 100) || (t > 0 && t - c < 100) || (c % 100 == 0) {
        print!("\r{}: {}/{}", metric, c, t);
        io::stdout().flush()?;
    }
    Ok(())
}

//================ parallel processing
//
//
fn process_group_partial_hash(
    file_vec: Vec<Arc<FileInfo>>,
    n_threads: usize,
) -> HashMap<String, Vec<Arc<FileInfo>>> {
    let chunk_size = (file_vec.len() + n_threads - 1) / n_threads;
    let chunks: Vec<_> = file_vec.chunks(chunk_size).map(|c| c.to_vec()).collect();

    let handles: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            let chunk = chunk.to_vec(); // клонируем слайс ссылок
            thread::spawn(move || {
                let mut local_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();
                for file_d in chunk {
                    let hash = format!("{:x}", md5::compute(&file_d.first_and_last_4kb));
                    local_map.entry(hash).or_default().push(file_d);
                }
                local_map
            })
        })
        .collect();

    let mut merged_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();
    for handle in handles {
        let local_map = handle.join().unwrap();
        for (hash, vec) in local_map {
            merged_map.entry(hash).or_default().extend(vec);
        }
    }

    merged_map
}


fn process_group_full_hash(
    file_vec: Vec<Arc<FileInfo>>,
    n_threads: usize,
) -> HashMap<String, Vec<Arc<FileInfo>>> {
    let chunk_size = (file_vec.len() + n_threads - 1) / n_threads;
    let chunks: Vec<_> = file_vec.chunks(chunk_size).map(|c| c.to_vec()).collect();

    let handles: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            let chunk = chunk.to_vec(); // клонируем слайс ссылок
            thread::spawn(move || {
                let mut local_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();
                for file_d in chunk {
                    let hash = match file_hash(&file_d.path) {
                        Ok(h) => h,
                        Err(_) => {
                            continue;
                        }
                    };
                    local_map.entry(hash).or_default().push(file_d);
                }
                local_map
            })
        })
        .collect();

    let mut merged_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();
    for handle in handles {
        let local_map = handle.join().unwrap();
        for (hash, vec) in local_map {
            merged_map.entry(hash).or_default().extend(vec);
        }
    }

    merged_map
}



fn read_first_and_last_4kb(path: &Path, file_size: u64) -> Result<[u8; 8192], Box<dyn Error>> {
    let mut buffer = [0u8; 8192];
    let mut f = File::open(path)?;

    let first_read = std::cmp::min(file_size, 4096) as usize;
    f.read(&mut buffer[..first_read])?;

    if file_size > 4096 {
        let last_read = std::cmp::min(file_size, 4096) as usize;
        f.seek(SeekFrom::End(-(last_read as i64)))?;
        f.read(&mut buffer[4096..4096 + last_read])?;
    }

    Ok(buffer)
}
