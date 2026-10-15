//! Bookmark management for saved locations.

use crate::types::Bookmark;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR_NAME: &str = "cyberpunk_fm";
const BOOKMARKS_FILE: &str = "bookmarks.json";

/// Get the path to the bookmarks file.
fn get_bookmarks_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_DIR_NAME)
        .join(BOOKMARKS_FILE)
}

/// Load bookmarks from the configuration file.
///
/// Returns an empty vector if the file doesn't exist or cannot be parsed.
#[must_use]
pub fn load_bookmarks() -> Vec<Bookmark> {
    let bookmark_path = get_bookmarks_path();

    if !bookmark_path.exists() {
        return Vec::new();
    }

    match fs::read_to_string(&bookmark_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(bookmarks) => bookmarks,
            Err(e) => {
                log::warn!("Failed to parse bookmarks file: {}", e);
                Vec::new()
            }
        },
        Err(e) => {
            log::warn!("Failed to read bookmarks file: {}", e);
            Vec::new()
        }
    }
}

/// Save bookmarks to the configuration file.
///
/// # Errors
/// Returns an error if:
/// - The config directory cannot be created
/// - The file cannot be written
/// - JSON serialization fails
pub fn save_bookmarks(bookmarks: &[Bookmark]) -> Result<()> {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_DIR_NAME);

    fs::create_dir_all(&config_dir).with_context(|| {
        format!(
            "Failed to create config directory: {}",
            config_dir.display()
        )
    })?;

    let bookmark_path = config_dir.join(BOOKMARKS_FILE);
    let content =
        serde_json::to_string_pretty(bookmarks).context("Failed to serialize bookmarks")?;

    fs::write(&bookmark_path, content)
        .with_context(|| format!("Failed to write bookmarks to {}", bookmark_path.display()))?;

    Ok(())
}

/// Add a new bookmark to the list.
pub fn add_bookmark(bookmarks: &mut Vec<Bookmark>, name: String, path: PathBuf) {
    // Check for duplicates
    if bookmarks.iter().any(|b| b.path == path) {
        return;
    }

    bookmarks.push(Bookmark { name, path });
}

/// Remove a bookmark by index.
///
/// Returns `true` if a bookmark was removed.
pub fn remove_bookmark(bookmarks: &mut Vec<Bookmark>, index: usize) -> bool {
    if index < bookmarks.len() {
        bookmarks.remove(index);
        true
    } else {
        false
    }
}

/// Find a bookmark by path.
#[must_use]
#[allow(dead_code)] // Public API for library use
pub fn find_bookmark_by_path<'a>(
    bookmarks: &'a [Bookmark],
    path: &PathBuf,
) -> Option<&'a Bookmark> {
    bookmarks.iter().find(|b| &b.path == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_bookmark() {
        let mut bookmarks = Vec::new();

        add_bookmark(&mut bookmarks, "Test".to_string(), PathBuf::from("/test"));

        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].name, "Test");
        assert_eq!(bookmarks[0].path, PathBuf::from("/test"));
    }

    #[test]
    fn test_add_duplicate_bookmark() {
        let mut bookmarks = Vec::new();

        add_bookmark(&mut bookmarks, "Test".to_string(), PathBuf::from("/test"));
        add_bookmark(&mut bookmarks, "Test2".to_string(), PathBuf::from("/test"));

        // Should not add duplicate
        assert_eq!(bookmarks.len(), 1);
    }

    #[test]
    fn test_remove_bookmark() {
        let mut bookmarks = vec![
            Bookmark {
                name: "First".to_string(),
                path: PathBuf::from("/first"),
            },
            Bookmark {
                name: "Second".to_string(),
                path: PathBuf::from("/second"),
            },
        ];

        let removed = remove_bookmark(&mut bookmarks, 0);

        assert!(removed);
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].name, "Second");
    }

    #[test]
    fn test_remove_bookmark_invalid_index() {
        let mut bookmarks = vec![Bookmark {
            name: "First".to_string(),
            path: PathBuf::from("/first"),
        }];

        let removed = remove_bookmark(&mut bookmarks, 5);

        assert!(!removed);
        assert_eq!(bookmarks.len(), 1);
    }

    #[test]
    fn test_find_bookmark_by_path() {
        let bookmarks = vec![
            Bookmark {
                name: "First".to_string(),
                path: PathBuf::from("/first"),
            },
            Bookmark {
                name: "Second".to_string(),
                path: PathBuf::from("/second"),
            },
        ];

        let path = PathBuf::from("/second");
        let found = find_bookmark_by_path(&bookmarks, &path);

        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Second");
    }

    #[test]
    fn test_find_bookmark_by_path_not_found() {
        let bookmarks = vec![Bookmark {
            name: "First".to_string(),
            path: PathBuf::from("/first"),
        }];

        let path = PathBuf::from("/nonexistent");
        let found = find_bookmark_by_path(&bookmarks, &path);

        assert!(found.is_none());
    }

    #[test]
    fn test_bookmark_serialization_roundtrip() {
        let bookmarks = vec![
            Bookmark {
                name: "Home".to_string(),
                path: PathBuf::from("/home/user"),
            },
            Bookmark {
                name: "Projects".to_string(),
                path: PathBuf::from("/home/user/projects"),
            },
        ];

        let json = serde_json::to_string(&bookmarks).unwrap();
        let deserialized: Vec<Bookmark> = serde_json::from_str(&json).unwrap();

        assert_eq!(bookmarks, deserialized);
    }
}
