use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

pub fn find_duplicates(p: &Path, dry: bool, verbose: bool) -> Result<(), Box<dyn Error>> {
    if !p.is_dir() {
        return Ok(());
    }

    // let r_dir = fs::read_dir(p)?;

    let files_by_sizes = find_same_size_files_recursive(p)?;

    let mut possible_duplicates = 0;

    for (key, v) in &files_by_sizes {
        let l = v.len();
        if l > 2 {
            possible_duplicates += 1;
        }
        if verbose {
            println!("{} -> {}", key, l);
        }
    }

    println!("possible duplicates by size: {}", possible_duplicates);

    Ok(())
}

pub fn find_same_size_files_recursive(p: &Path) -> Result<HashMap<u64, Vec<PathBuf>>, Box<dyn Error>> {
    if !p.is_dir() {
        return Err("not a dir".into());
    }

    let r_dir = fs::read_dir(p)?;

    let mut files_by_sizes: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for entry in r_dir {
        let e = entry?;
        let path = e.path();

        if path.is_dir() {
            let mut child_dir_result = find_same_size_files_recursive(&path)?;

            for (c_s,c_v) in child_dir_result.iter_mut() {
                if let Some(v) = files_by_sizes.get_mut(&c_s) {
                    v.append(c_v);
                } else {
                    files_by_sizes.insert(*c_s, c_v.to_vec());
                }
            }
        }

        let m = path.metadata()?;

        let file_size = m.len();

        if let Some(v) = files_by_sizes.get_mut(&file_size) {
            v.push(path);
        } else {
            files_by_sizes.insert(file_size, vec![path]);
        }
    }

    Ok(files_by_sizes)
}
