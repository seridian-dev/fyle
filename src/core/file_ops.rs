//! Stable facade; filesystem mutations, directory queries and display policy vary independently.
mod listing;
mod operations;
mod presentation;
pub use listing::*;
pub use operations::*;
pub use presentation::*;
// File system operations (copy, move, delete, create).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FileEntry, SortBy, SortOrder};
    use chrono::Local;
    use std::{collections::HashSet, fs, path::PathBuf};
    use tempfile::tempdir;

    #[test]
    fn test_format_size_zero() {
        assert_eq!(format_size(0), "0 B");
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 + 512 * 1024), "1.5 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_format_size_terabytes() {
        assert_eq!(format_size(1024u64 * 1024 * 1024 * 1024), "1.0 TB");
    }

    #[test]
    fn test_get_file_icon_directory() {
        let entry = FileEntry {
            name: "test".to_string(),
            size: 0,
            is_dir: true,
            path: PathBuf::from("/test"),
            modified: Local::now(),
            is_hidden: false,
            file_type: "Directory".to_string(),
            selected: false,
        };
        assert_eq!(get_file_icon(&entry), "📁");
    }

    #[test]
    fn test_get_file_icon_rust_file() {
        let entry = FileEntry {
            name: "main.rs".to_string(),
            size: 1000,
            is_dir: false,
            path: PathBuf::from("/main.rs"),
            modified: Local::now(),
            is_hidden: false,
            file_type: "RS".to_string(),
            selected: false,
        };
        assert_eq!(get_file_icon(&entry), "🦀");
    }

    #[test]
    fn test_get_file_icon_image() {
        let entry = FileEntry {
            name: "photo.jpg".to_string(),
            size: 1000,
            is_dir: false,
            path: PathBuf::from("/photo.jpg"),
            modified: Local::now(),
            is_hidden: false,
            file_type: "JPG".to_string(),
            selected: false,
        };
        assert_eq!(get_file_icon(&entry), "🖼️");
    }

    #[test]
    fn test_get_file_icon_video() {
        let entry = FileEntry {
            name: "movie.mp4".to_string(),
            size: 1000,
            is_dir: false,
            path: PathBuf::from("/movie.mp4"),
            modified: Local::now(),
            is_hidden: false,
            file_type: "MP4".to_string(),
            selected: false,
        };
        assert_eq!(get_file_icon(&entry), "🎬");
    }

    #[test]
    fn test_create_directory() {
        let temp = tempdir().unwrap();
        let result = create_directory(temp.path(), "new_folder");

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());
        assert!(path.is_dir());
    }

    #[test]
    fn test_create_file() {
        let temp = tempdir().unwrap();
        let result = create_file(temp.path(), "test.txt");

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());
        assert!(path.is_file());
    }

    #[test]
    fn test_delete_files() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("delete_me.txt");
        fs::write(&file_path, "test").unwrap();

        assert!(file_path.exists());
        let result = delete_files(std::slice::from_ref(&file_path));

        assert!(result.is_ok());
        assert!(!file_path.exists());
    }

    #[test]
    fn test_copy_files() {
        let temp = tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let dest_dir = temp.path().join("dest");

        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&dest_dir).unwrap();

        let source_file = source_dir.join("file.txt");
        fs::write(&source_file, "content").unwrap();

        let result = copy_files(std::slice::from_ref(&source_file), &dest_dir);

        assert!(result.is_ok());
        assert!(dest_dir.join("file.txt").exists());
        assert!(source_file.exists()); // Original still exists
    }

    #[test]
    fn test_move_files() {
        let temp = tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let dest_dir = temp.path().join("dest");

        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&dest_dir).unwrap();

        let source_file = source_dir.join("file.txt");
        fs::write(&source_file, "content").unwrap();

        let result = move_files(std::slice::from_ref(&source_file), &dest_dir);

        assert!(result.is_ok());
        assert!(dest_dir.join("file.txt").exists());
        assert!(!source_file.exists()); // Original is gone
    }

    #[test]
    fn test_rename_file() {
        let temp = tempdir().unwrap();
        let old_path = temp.path().join("old_name.txt");
        fs::write(&old_path, "content").unwrap();

        let result = rename_file(&old_path, "new_name.txt");

        assert!(result.is_ok());
        let new_path = result.unwrap();
        assert!(new_path.exists());
        assert!(!old_path.exists());
        assert_eq!(new_path.file_name().unwrap(), "new_name.txt");
    }

    #[test]
    fn test_load_directory_entries() {
        let temp = tempdir().unwrap();

        // Create some test files and directories
        fs::write(temp.path().join("file1.txt"), "content").unwrap();
        fs::write(temp.path().join("file2.rs"), "fn main() {}").unwrap();
        fs::create_dir(temp.path().join("subdir")).unwrap();
        fs::write(temp.path().join(".hidden"), "hidden").unwrap();

        // Test with hidden files
        let entries = load_directory_entries(temp.path(), true, &HashSet::new()).unwrap();
        assert_eq!(entries.len(), 4);

        // Test without hidden files
        let entries = load_directory_entries(temp.path(), false, &HashSet::new()).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_sort_entries_by_name() {
        let mut entries = vec![
            create_test_entry("zebra.txt", false),
            create_test_entry("alpha.txt", false),
            create_test_entry("middle.txt", false),
        ];

        sort_entries(&mut entries, &SortBy::Name, &SortOrder::Ascending);

        assert_eq!(entries[0].name, "alpha.txt");
        assert_eq!(entries[1].name, "middle.txt");
        assert_eq!(entries[2].name, "zebra.txt");
    }

    #[test]
    fn test_sort_entries_directories_first() {
        let mut entries = vec![
            create_test_entry("file.txt", false),
            create_test_entry("dir", true),
            create_test_entry("another_file.txt", false),
        ];

        sort_entries(&mut entries, &SortBy::Name, &SortOrder::Ascending);

        assert!(entries[0].is_dir);
    }

    #[test]
    fn test_filter_entries() {
        let mut entries = vec![
            create_test_entry("README.md", false),
            create_test_entry("readme.txt", false),
            create_test_entry("other.rs", false),
        ];

        filter_entries(&mut entries, "readme");

        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_filter_entries_empty_query() {
        let mut entries = vec![
            create_test_entry("file1.txt", false),
            create_test_entry("file2.txt", false),
        ];

        filter_entries(&mut entries, "");

        assert_eq!(entries.len(), 2);
    }

    fn create_test_entry(name: &str, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            size: 100,
            is_dir,
            path: PathBuf::from(format!("/test/{}", name)),
            modified: Local::now(),
            is_hidden: name.starts_with('.'),
            file_type: if is_dir {
                "Directory".to_string()
            } else {
                name.rsplit('.').next().unwrap_or("File").to_uppercase()
            },
            selected: false,
        }
    }
}
