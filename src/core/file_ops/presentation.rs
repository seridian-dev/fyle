//! File system operations (copy, move, delete, create).

use crate::types::FileEntry;

/// Format a file size in human-readable format.
#[must_use]
pub fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];

    if size == 0 {
        return "0 B".to_string();
    }

    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Get an appropriate icon for a file entry.
#[must_use]
pub fn get_file_icon(entry: &FileEntry) -> &'static str {
    if entry.is_dir {
        return "📁";
    }

    match entry.file_type.as_str() {
        // Text files
        "TXT" | "MD" | "RST" | "ORG" => "📄",

        // Images
        "JPG" | "JPEG" | "PNG" | "GIF" | "BMP" | "WEBP" | "SVG" | "ICO" | "TIFF" => "🖼️",

        // Audio
        "MP3" | "WAV" | "FLAC" | "OGG" | "M4A" | "AAC" | "OPUS" | "WMA" => "🎵",

        // Video
        "MP4" | "AVI" | "MKV" | "MOV" | "WEBM" | "FLV" | "WMV" | "M4V" => "🎬",

        // Documents
        "PDF" => "📕",
        "DOC" | "DOCX" | "ODT" => "📘",
        "XLS" | "XLSX" | "ODS" => "📊",
        "PPT" | "PPTX" | "ODP" => "📽️",

        // Archives
        "ZIP" | "RAR" | "7Z" | "TAR" | "GZ" | "BZ2" | "XZ" => "📦",

        // Executables
        "EXE" | "APP" | "DEB" | "RPM" | "APPIMAGE" => "⚙️",

        // Programming languages
        "RS" => "🦀",
        "PY" => "🐍",
        "JS" | "TS" | "JSX" | "TSX" => "📜",
        "GO" => "🐹",
        "C" | "CPP" | "H" | "HPP" => "⚡",
        "JAVA" | "KT" => "☕",
        "RB" => "💎",
        "PHP" => "🐘",
        "SH" | "BASH" | "ZSH" => "🐚",

        // Web
        "HTML" | "HTM" => "🌐",
        "CSS" | "SCSS" | "SASS" | "LESS" => "🎨",

        // Config/Data
        "JSON" | "XML" | "YAML" | "YML" | "TOML" | "INI" | "CFG" => "📋",

        // Default
        _ => "📄",
    }
}
