use std::{error::Error, fs::File, io::Read, path::Path};

use phf::phf_map;

#[derive(Debug)]
pub enum FileType {
    Image,
    AnimatedImage,
    Video,
    Audio,
    Document,
    Text,
    Table,
    Archive,
    Application,
    Code,
    Torrent,
}

pub static TYPE_MAP: phf::Map<&'static str, FileType> = phf_map! {
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

    "text/html" => FileType::Code,
    "text/css" => FileType::Code,
    "text/javascript" => FileType::Code,
    "application/json" => FileType::Code,
    "application/xml" => FileType::Code,
    "application/x-yaml" => FileType::Code,
};

pub fn type_dir(t: &FileType) -> Option<&'static str> {
    return match t {
        FileType::Image => Some("images"),
        FileType::AnimatedImage => Some("images/animated"),
        FileType::Video => Some("videos"),
        FileType::Audio => Some("audios"),
        FileType::Document => Some("documents"),
        FileType::Text => Some("texts"),
        FileType::Table => Some("tables"),
        // FileType::Archive => Some("archives"),
        FileType::Application => Some("applications"),
        FileType::Code => Some("code"),
        FileType::Torrent => Some("torrents"),
        _ => None,
    };
}

pub fn detect_file_type(p: &Path) -> Result<&'static FileType, Box<dyn Error>> {
    if p.is_dir() {
        return Err("it is a directory".into());
    }

    let mut f = match File::open(p) {
        Ok(file) => file,
        Err(e) => {
            return Err(format!("Failed to open file: {}", e).into());
        }
    };


    let mut file_buff = [0u8; 512];
    let n = match f.read(&mut file_buff) {
        Ok(n) => n,
        Err(e) => {
            return Err(format!("Failed to read file: {}", e).into());
        }
    };

    if let Some(kind) = infer::get(&file_buff[..n]) {
        if let Some(f_type) = calculate_file_type(kind.mime_type(), kind.extension()) {
            return Ok(f_type);
        } else {
            Err(format!("unsupported mime type: {}", kind.mime_type()).into())
        }
    } else {

        let ext = p.extension().ok_or("file type unknown")?;

        if let Some(f_type) = calculate_file_type("", &ext.display().to_string()) {
            return Ok(f_type);
        }

        Err("file mime type undetected".into())
    }
}

pub fn calculate_file_type(mime: &str, ext: &str) -> Option<&'static FileType> {
    match TYPE_MAP.get(mime) {
        Some(FileType::Archive) => match ext {
            "docx" | "xlsx" | "pptx" => Some(&FileType::Document),
            _ => Some(&FileType::Archive),
        },
        Some(t) => Some(t),
        None => match ext {
            "txt" => Some(&FileType::Text),
            "torrent" => Some(&FileType::Torrent),
            "md" | "csv" => Some(&FileType::Document),
            "json" => Some(&FileType::Code),
            _ => None,
        },
    }
}
