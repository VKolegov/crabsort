use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

#[derive(Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    first_and_last_4kb: [u8; 8192],
}

pub fn find_duplicates_async(
    p: &Path,
    min_file_size_kb: u64,
    max_file_size_kb: u64,
    stage_description: Arc<Mutex<String>>,
    progress_description: Arc<Mutex<String>>,
    progress: Arc<Mutex<u64>>,
    max: Arc<Mutex<u64>>,
) -> Result<HashMap<String, Vec<Arc<FileInfo>>>, Box<dyn Error>> {
    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1); // fallback на 1

    *stage_description.clone().lock().unwrap() = String::from("[Step 1/3] Scanning for files"); // safe: no worker threads yet
    *progress_description.lock().unwrap() = String::new(); // safe: same as above

    let mut files_by_sizes = find_same_size_files_recursive_parallel(
        p,
        min_file_size_kb,
        max_file_size_kb,
        progress.clone(),
        progress_description.clone(),
    )?;

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

    // step 2 - partial hash
    *progress.lock().unwrap() = 0; // safe: between stages, no worker threads active
    *max.lock().unwrap() = files_count_for_partial_hash; // safe: same as above
    *stage_description.clone().lock().unwrap() =
        String::from("[Step 2/3] Calculating potential duplicates"); // safe: same as above
    *progress_description.lock().unwrap() = String::new(); // safe: same as above
    let mut partial_hash_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();

    for (_, file_vec) in duplicate_groups_filtered {
        let mut size_group_partial_hash_map = process_group_partial_hash(
            file_vec.to_vec(),
            n_threads,
            progress_description.clone(),
        );
        *progress.lock().unwrap() += file_vec.len() as u64; // safe: only UI reads concurrently

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
        }
    }

    // step 3 - full hash
    *progress.lock().unwrap() = 0; // safe: between stages, no worker threads active
    *max.lock().unwrap() = files_count_for_full_hash; // safe: same as above
    *stage_description.clone().lock().unwrap() =
        String::from("[Step 3/3] Evaluating duplicates"); // safe: same as above
    *progress_description.lock().unwrap() = String::new(); // safe: same as above
    let mut full_hash_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();
    for (_, file_vec) in partial_hash_map {
        let mut phash_group_full_hash_map = process_group_full_hash(
            file_vec.clone(),
            n_threads,
            progress_description.clone(),
        );

        *progress.lock().unwrap() += file_vec.len() as u64; // safe: only UI reads concurrently

        // filtering groups within size groups
        phash_group_full_hash_map.retain(|_k, v| v.len() >= 2);

        for (k, v) in phash_group_full_hash_map {
            if full_hash_map.get(&k).is_none() {
                full_hash_map.insert(k.to_string(), v.to_vec());
            }
        }
    }

    Ok(full_hash_map)
}

fn file_hash(path: &Path) -> Result<String, Box<dyn Error>> {
    use blake3::Hasher;

    let mut f = File::open(path)?;
    let mut hasher = Hasher::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let n = f.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hasher.finalize().to_hex().to_string())
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

//================ parallel processing
//
//
fn process_group_partial_hash(
    file_vec: Vec<Arc<FileInfo>>,
    n_threads: usize,
    progress_description: Arc<Mutex<String>>,
) -> HashMap<String, Vec<Arc<FileInfo>>> {
    let chunk_size = (file_vec.len() + n_threads - 1) / n_threads;
    let chunks: Vec<_> = file_vec.chunks(chunk_size).map(|c| c.to_vec()).collect();

    let handles: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            let chunk = chunk.to_vec(); // клонируем слайс ссылок
            let progress_description = Arc::clone(&progress_description);
            thread::spawn(move || {
                let mut local_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();
                for file_d in chunk {
                    *progress_description.lock().unwrap() = file_d.path.display().to_string(); // safe: only UI reads concurrently
                    let hash = format!("{:x}", md5::compute(&file_d.first_and_last_4kb));
                    local_map.entry(hash).or_default().push(file_d);
                }
                local_map
            })
        })
        .collect();

    let mut merged_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();
    for handle in handles {
        if let Ok(local_map) = handle.join() {
            for (hash, vec) in local_map {
                merged_map.entry(hash).or_default().extend(vec);
            }
        }
        // thread panicked — skip its chunk, results from other threads are still valid
    }

    merged_map
}

fn process_group_full_hash(
    file_vec: Vec<Arc<FileInfo>>,
    n_threads: usize,
    progress_description: Arc<Mutex<String>>,
) -> HashMap<String, Vec<Arc<FileInfo>>> {
    let chunk_size = (file_vec.len() + n_threads - 1) / n_threads;
    let chunks: Vec<_> = file_vec.chunks(chunk_size).map(|c| c.to_vec()).collect();

    let handles: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            let chunk = chunk.to_vec(); // клонируем слайс ссылок
            let progress_description = Arc::clone(&progress_description);
            thread::spawn(move || {
                let mut local_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();
                for file_d in chunk {
                    *progress_description.lock().unwrap() = file_d.path.display().to_string(); // safe: only UI reads concurrently
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
        if let Ok(local_map) = handle.join() {
            for (hash, vec) in local_map {
                merged_map.entry(hash).or_default().extend(vec);
            }
        }
        // thread panicked — skip its chunk, results from other threads are still valid
    }

    merged_map
}

/// Recursively scans directory tree for files.
/// Spawns one thread per top-level subdirectory; each thread walks its subtree synchronously.
pub fn build_dir_flatmap_parallel(
    p: &Path,
    min_file_size_kb: u64,
    max_file_size_kb: u64,
    files_read: Arc<Mutex<u64>>,
    progress_description: Arc<Mutex<String>>,
) -> Vec<Arc<FileInfo>> {
    let dir_map = Arc::new(Mutex::new(Vec::new()));
    let mut handles: Vec<thread::JoinHandle<()>> = vec![];

    let r_dir = match fs::read_dir(p) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    for entry in r_dir.filter_map(Result::ok) {
        let path = entry.path();

        if path.is_dir() && !path.is_symlink() {
            let dir_map_clone = Arc::clone(&dir_map);
            let files_read_clone = Arc::clone(&files_read);
            let progress_description_clone = Arc::clone(&progress_description);

            handles.push(thread::spawn(move || {
                let child_files = scan_dir_recursive(
                    &path,
                    min_file_size_kb,
                    max_file_size_kb,
                    &files_read_clone,
                    &progress_description_clone,
                );
                let mut dm = dir_map_clone.lock().unwrap(); // safe: short-lived lock
                dm.extend(child_files);
            }));
        } else if path.is_file() && !path.is_symlink() {
            *progress_description.lock().unwrap() = // safe: only UI reads concurrently
                format!("Scanning: {}", path.display());
            if let Ok(fi) = try_read_file_info(&path, min_file_size_kb, max_file_size_kb) {
                dir_map.lock().unwrap().push(fi); // safe: short-lived lock
                *files_read.lock().unwrap() += 1; // safe: only UI reads concurrently
            }
        }
    }

    for h in handles {
        let _ = h.join(); // if a thread panicked, its subtree is skipped
    }

    Arc::try_unwrap(dir_map)
        .expect("bug: Arc still has multiple owners after all threads joined")
        .into_inner()
        .unwrap_or_default()
}

/// Single-threaded recursive directory walk — used inside per-directory threads.
fn scan_dir_recursive(
    p: &Path,
    min_file_size_kb: u64,
    max_file_size_kb: u64,
    files_read: &Arc<Mutex<u64>>,
    progress_description: &Arc<Mutex<String>>,
) -> Vec<Arc<FileInfo>> {
    let mut result = Vec::new();
    let mut dirs_to_visit = vec![p.to_path_buf()];

    while let Some(dir) = dirs_to_visit.pop() {
        let r_dir = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue, // permission denied, etc — skip directory
        };

        for entry in r_dir.filter_map(Result::ok) {
            let path = entry.path();

            if path.is_dir() && !path.is_symlink() {
                dirs_to_visit.push(path);
            } else if path.is_file() && !path.is_symlink() {
                *progress_description.lock().unwrap() = // safe: only UI reads concurrently
                    format!("Scanning: {}", path.display());
                if let Ok(fi) = try_read_file_info(&path, min_file_size_kb, max_file_size_kb) {
                    result.push(fi);
                    *files_read.lock().unwrap() += 1; // safe: only UI reads concurrently
                }
            }
        }
    }

    result
}

fn try_read_file_info(
    path: &Path,
    min_file_size_kb: u64,
    max_file_size_kb: u64,
) -> Result<Arc<FileInfo>, Box<dyn Error>> {
    let meta = fs::metadata(path)?;
    let file_size = meta.len();
    if file_size < min_file_size_kb || file_size > max_file_size_kb {
        return Err("file size out of range".into());
    }
    let buf = read_first_and_last_4kb(path, file_size)?;
    Ok(Arc::new(FileInfo {
        path: path.to_path_buf(),
        size: file_size,
        first_and_last_4kb: buf,
    }))
}

pub fn find_same_size_files_recursive_parallel(
    p: &Path,
    min_file_size_kb: u64,
    max_file_size_kb: u64,
    files_read: Arc<Mutex<u64>>,
    progress_description: Arc<Mutex<String>>,
) -> Result<HashMap<u64, Vec<Arc<FileInfo>>>, Box<dyn Error>> {
    let files = build_dir_flatmap_parallel(
        p,
        min_file_size_kb,
        max_file_size_kb,
        Arc::clone(&files_read),
        progress_description,
    );

    // Группировка по размеру
    let mut files_by_sizes: HashMap<u64, Vec<Arc<FileInfo>>> = HashMap::new();
    for f in files {
        files_by_sizes.entry(f.size).or_default().push(f);
    }

    Ok(files_by_sizes)
}
