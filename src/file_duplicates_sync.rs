

pub fn build_dir_flatmap(
    p: &Path,
    files_read: &mut u64,
) -> Result<Vec<Arc<FileInfo>>, Box<dyn Error>> {
    if !p.is_dir() {
        return Err("not a dir".into());
    }

    let mut dir_map: Vec<Arc<FileInfo>> = Vec::new();

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

        dir_map.push(Arc::from(FileInfo {
            path: path,
            size: file_size,
            first_and_last_4kb: f_a_l_4kb,
        }));

        *files_read += 1;

        print_progress("files_read", *files_read, 0)?;
    }

    Ok(dir_map)
}

pub fn find_same_size_files_recursive(
    p: &Path,
) -> Result<HashMap<u64, Vec<Arc<FileInfo>>>, Box<dyn Error>> {
    let map = build_dir_flatmap(p, &mut 0)?;
    println!("");

    // group by size
    let mut files_by_sizes: HashMap<u64, Vec<Arc<FileInfo>>> = HashMap::new();
    for ele in map {
        if let Some(v) = files_by_sizes.get_mut(&ele.size) {
            v.push(ele);
        } else {
            files_by_sizes.insert(ele.size, vec![ele]);
        }
    }

    Ok(files_by_sizes)
}
