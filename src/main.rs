use phf::phf_map;
use std::{
    env,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process,
};

#[derive(Debug)]
enum FileType {
    Image,
    AnimatedImage,
    Video,
    Audio,
    Document,
    Text,
    Table,
    Archive,
    Application,
}

static TYPE_MAP: phf::Map<&'static str, FileType> = phf_map! {
    // изображения
    "image/jpeg" => FileType::Image,
    "image/png" => FileType::Image,
    "image/gif" => FileType::AnimatedImage,
    "image/bmp" => FileType::Image,
    "image/webp" => FileType::Image,
    "image/tiff" => FileType::Image,
    "image/svg+xml" => FileType::Image,
    "image/x-icon" => FileType::Image,

    // видео
    "video/mp4" => FileType::Video,
    "video/webm" => FileType::Video,
    "video/ogg" => FileType::Video,
    "video/quicktime" => FileType::Video,
    "video/x-msvideo" => FileType::Video, // avi
    "video/x-ms-wmv" => FileType::Video,
    "video/mpeg" => FileType::Video,

    "audio/mpeg" => FileType::Audio,
    "audio/opus" => FileType::Audio,

    // документы
    "application/pdf" => FileType::Document,
    "application/msword" => FileType::Document,
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => FileType::Document, // docx
    "application/vnd.ms-powerpoint" => FileType::Document,
    "application/vnd.openxmlformats-officedocument.presentationml.presentation" => FileType::Document, // pptx
    "application/rtf" => FileType::Document,
    "application/epub+zip" => FileType::Document,
    "application/x-shockwave-flash" => FileType::Document, // swf

    // текст
    "text/plain" => FileType::Text,
    "text/html" => FileType::Text,
    "text/css" => FileType::Text,
    "text/javascript" => FileType::Text,
    "application/json" => FileType::Text,
    "application/xml" => FileType::Text,
    "application/x-yaml" => FileType::Text,
    "text/markdown" => FileType::Text,

    // таблицы
    "text/csv" => FileType::Table,
    "application/vnd.ms-excel" => FileType::Table, // xls
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => FileType::Table, // xlsx
                                                                                            //
    "application/zip" => FileType::Archive,
    "application/gzip" => FileType::Archive,
    "application/x-tar" => FileType::Archive,

    //
    "application/x-executable" => FileType::Application,
    "application/vnd.debian.binary-package" => FileType::Application,
    "text/x-shellscript" => FileType::Application,
};

fn type_dir(t: &FileType) -> Option<&'static str> {
    return match t {
        FileType::Image => Some("images"),
        FileType::AnimatedImage => Some("images/animated"),
        FileType::Video => Some("videos"),
        FileType::Audio => Some("audios"),
        FileType::Document => Some("documents"),
        FileType::Text => Some("texts"),
        FileType::Table => Some("tables"),
        FileType::Archive => Some("archives"),
        FileType::Application => Some("applications"),
    };
}

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
    let first_arg = env::args().nth(1).ok_or("directory argument required")?;

    let path = PathBuf::from(first_arg);

    if !path.exists() {
        return Err("path does not exist".into());
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

        if path.is_dir() {
            continue;
        }

        let mut f = match File::open(&path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Failed to read file: {}", e);
                continue;
            }
        };

        let path_str = path.display().to_string();

        let mut file_buff = [0u8; 512];
        let n = match f.read(&mut file_buff) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error while reading {}: {}", path_str, e);
                continue;
            }
        };

        if let Some(kind) = infer::get(&file_buff[..n]) {
            if let Some(ft) = TYPE_MAP.get(kind.mime_type()) {
                println!("file: {}, type: {:?}", path_str, ft);
                if let Some(d) = type_dir(&ft) {
                    let full_path = p.join(d);
                    println!("dir: {}", full_path.display().to_string());
                    ensure_dir(&full_path.display().to_string());
                }
            } else {
                println!(
                    "unknown type, file: {}, mime: {}, ext: {}",
                    path_str,
                    kind.mime_type(),
                    kind.extension()
                );
            }
        }
    }

    Ok(())
}

fn ensure_dir(p: &str) -> Result<(), io::Error> {
    let exists = fs::exists(p)?;

    if exists {
        return Ok(());
    }

    fs::create_dir(p)?;

    Ok(())
}
