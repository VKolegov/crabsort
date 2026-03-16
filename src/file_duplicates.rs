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

pub fn find_duplicates_async(p: &Path, progress: Arc<Mutex<u64>>, max: Arc<Mutex<u64>>) -> Result<HashMap<String, Vec<Arc<FileInfo>>>, Box<dyn Error>> {

    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1); // fallback на 1
            

    let mut files_by_sizes = find_same_size_files_recursive_parallel(p, progress.clone(), n_threads)?;

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
    *progress.lock().unwrap() = 0;
    *max.lock().unwrap() = files_count_for_partial_hash;
    let mut partial_hash_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();

    for (_, file_vec) in duplicate_groups_filtered {
        let mut size_group_partial_hash_map = process_group_partial_hash(file_vec.to_vec(), n_threads);
        *progress.lock().unwrap() += file_vec.len() as u64;

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
    *progress.lock().unwrap() = 0;
    *max.lock().unwrap() = files_count_for_full_hash;
    let mut full_hash_map: HashMap<String, Vec<Arc<FileInfo>>> = HashMap::new();
    for (_, file_vec) in partial_hash_map {
        let mut phash_group_full_hash_map = process_group_full_hash(file_vec.clone(), n_threads);

        *progress.lock().unwrap() += file_vec.len() as u64;

        // filtering groups within size groups
        phash_group_full_hash_map.retain(|_k, v| v.len() >= 2);

        for (k, v) in phash_group_full_hash_map {
            if full_hash_map.get(&k).is_none() {
                full_hash_map.insert(k.to_string(), v.to_vec());
            }
        }
    }

    // println!("-----");
    // let mut size_estimate = 0;
    // for (_, file_vec) in &full_hash_map {
    //
    //     println!("{} kb x {}", file_vec[0].size / 1024, file_vec.len());
    //
    //     for file_d in file_vec {
    //         println!("{} | {} kb", file_d.path.display(), file_d.size / 1024);
    //     }
    //     size_estimate += (file_vec.len() as u64 - 1) * file_vec[0].size;
    //     println!("-----");
    // }
    //
    // println!("can save up to: {} kb", size_estimate / 1024);

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

pub fn build_dir_flatmap_parallel(
    p: &Path,
    files_read: Arc<Mutex<u64>>,
    max_threads: usize,
) -> Vec<Arc<FileInfo>> {
    let dir_map = Arc::new(Mutex::new(Vec::new()));
    let mut handles: Vec<thread::JoinHandle<()>> = vec![];

    if let Ok(r_dir) = fs::read_dir(p) {
        for entry in r_dir.filter_map(Result::ok) {
            let path = entry.path();
            let dir_map_clone = Arc::clone(&dir_map);
            let files_read_clone = Arc::clone(&files_read);

            if path.is_dir() {
                // Ограничиваем количество потоков
                if handles.len() >= max_threads {
                    for h in handles.drain(..) {
                        h.join().unwrap();
                    }
                }

                handles.push(thread::spawn(move || {
                    let child_files =
                        build_dir_flatmap_parallel(&path, files_read_clone, max_threads);
                    let mut dm = dir_map_clone.lock().unwrap();
                    dm.extend(child_files);
                }));
            } else if path.is_file() && !path.is_symlink() {
                if let Ok(meta) = entry.metadata() {
                    let file_size = meta.len();
                    if file_size < 1024 * 100 || file_size > 1024 * 1024 * 1024 {
                        continue;
                    }

                    if let Ok(buf) = read_first_and_last_4kb(&path, file_size) {
                        let fi = Arc::new(FileInfo {
                            path: path.clone(),
                            size: file_size,
                            first_and_last_4kb: buf,
                        });

                        let mut dm = dir_map_clone.lock().unwrap();
                        dm.push(fi);
                        let mut fr = files_read_clone.lock().unwrap();
                        *fr += 1;
                    }
                }
            }
        }
    }

    for h in handles {
        h.join().unwrap();
    }

    Arc::try_unwrap(dir_map).unwrap().into_inner().unwrap()
}

pub fn find_same_size_files_recursive_parallel(
    p: &Path,
    files_read: Arc<Mutex<u64>>,
    max_threads: usize,
) -> Result<HashMap<u64, Vec<Arc<FileInfo>>>, Box<dyn Error>> {
    let files = build_dir_flatmap_parallel(p, Arc::clone(&files_read), max_threads);

    // Группировка по размеру
    let mut files_by_sizes: HashMap<u64, Vec<Arc<FileInfo>>> = HashMap::new();
    for f in files {
        files_by_sizes.entry(f.size).or_default().push(f);
    }

    Ok(files_by_sizes)
}

