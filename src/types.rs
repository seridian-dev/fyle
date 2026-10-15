//! Core types and data structures for the file manager.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Custom error types for file manager operations.
#[derive(Error, Debug)]
#[allow(dead_code)] // Some variants reserved for future features
pub enum FileManagerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path error: {0}")]
    Path(String),

    #[error("Operation cancelled by user")]
    Cancelled,

    #[error("Invalid file operation: {0}")]
    InvalidOperation(String),

    #[error("Failed to open file: {0}")]
    OpenFailed(String),

    #[error("Trash operation failed: {0}")]
    TrashFailed(String),
}

/// Sorting criteria for file entries.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[allow(dead_code)] // DateModified reserved for future sorting feature
pub enum SortBy {
    #[default]
    Name,
    Size,
    DateModified,
    Type,
}

/// Sort direction.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

impl SortOrder {
    /// Toggle between ascending and descending order.
    #[must_use]
    pub fn toggle(&self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// A saved bookmark location.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub name: String,
    pub path: PathBuf,
}

/// Represents a file or directory entry.
#[derive(Clone, Debug, PartialEq)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    pub path: PathBuf,
    pub modified: DateTime<Local>,
    pub is_hidden: bool,
    pub file_type: String,
    pub selected: bool,
}

/// View mode for displaying files.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ViewMode {
    Grid,
    #[default]
    List,
}

/// Types of dialogs that can be displayed.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum DialogType {
    #[default]
    None,
    ConfirmDelete(Vec<PathBuf>),
    ConfirmCopy(Vec<PathBuf>, PathBuf),
    ConfirmMove(Vec<PathBuf>, PathBuf),
    FileProperties(FileEntry),
    CreateFolder,
    CreateFile,
    Rename(PathBuf, String),
}

/// Clipboard operation type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardOperation {
    Copy,
    Cut,
}

/// Quick access location definition.
#[derive(Clone, Debug)]
pub struct QuickAccessLocation {
    pub name: &'static str,
    pub get_path: fn() -> Option<PathBuf>,
}

/// Standard quick access locations.
pub fn get_quick_access_locations() -> Vec<QuickAccessLocation> {
    vec![
        QuickAccessLocation {
            name: "Home",
            get_path: dirs::home_dir,
        },
        QuickAccessLocation {
            name: "Desktop",
            get_path: dirs::desktop_dir,
        },
        QuickAccessLocation {
            name: "Documents",
            get_path: dirs::document_dir,
        },
        QuickAccessLocation {
            name: "Downloads",
            get_path: dirs::download_dir,
        },
        QuickAccessLocation {
            name: "Pictures",
            get_path: dirs::picture_dir,
        },
        QuickAccessLocation {
            name: "Music",
            get_path: dirs::audio_dir,
        },
        QuickAccessLocation {
            name: "Videos",
            get_path: dirs::video_dir,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_order_toggle() {
        assert_eq!(SortOrder::Ascending.toggle(), SortOrder::Descending);
        assert_eq!(SortOrder::Descending.toggle(), SortOrder::Ascending);
    }

    #[test]
    fn test_bookmark_serialization() {
        let bookmark = Bookmark {
            name: "Test".to_string(),
            path: PathBuf::from("/test/path"),
        };

        let json = serde_json::to_string(&bookmark).expect("Failed to serialize");
        let deserialized: Bookmark = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(bookmark.name, deserialized.name);
        assert_eq!(bookmark.path, deserialized.path);
    }

    #[test]
    fn test_dialog_type_default() {
        assert_eq!(DialogType::default(), DialogType::None);
    }

    #[test]
    fn test_view_mode_default() {
        assert_eq!(ViewMode::default(), ViewMode::List);
    }

    #[test]
    fn test_sort_by_default() {
        assert_eq!(SortBy::default(), SortBy::Name);
    }

    #[test]
    fn test_quick_access_locations() {
        let locations = get_quick_access_locations();
        assert!(!locations.is_empty());
        assert_eq!(locations[0].name, "Home");
    }
}
