//! File system operations (copy, move, delete, create).

use crate::types::{FileEntry, SortBy, SortOrder};
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Load directory entries with filtering options.
///
/// # Errors
/// Returns an error if the directory cannot be read.
pub fn load_directory_entries(
    path: &Path,
    show_hidden: bool,
    selected_entries: &HashSet<PathBuf>,
) -> Result<Vec<FileEntry>> {
    let mut entries = Vec::new();

    let dir_entries = fs::read_dir(path)
        .with_context(|| format!("Failed to read directory {}", path.display()))?;

    for entry in dir_entries {
        let entry = entry.with_context(|| "Failed to read directory entry")?;
        let entry_path = entry.path();

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                log::warn!(
                    "Failed to read metadata for {}: {}",
                    entry_path.display(),
                    e
                );
                continue;
            }
        };

        let name = entry_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let is_hidden = name.starts_with('.');

        if !show_hidden && is_hidden {
            continue;
        }

        let size = if metadata.is_file() {
            metadata.len()
        } else {
            0
        };
        let is_dir = metadata.is_dir();

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t: SystemTime| {
                let duration = t.duration_since(SystemTime::UNIX_EPOCH).ok()?;
                DateTime::from_timestamp(duration.as_secs() as i64, 0)
                    .map(|dt| dt.with_timezone(&Local))
            })
            .unwrap_or_else(Local::now);

        let file_type = if is_dir {
            "Directory".to_string()
        } else {
            entry_path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|s| s.to_uppercase())
                .unwrap_or_else(|| "File".to_string())
        };

        let selected = selected_entries.contains(&entry_path);

        entries.push(FileEntry {
            name,
            size,
            is_dir,
            path: entry_path,
            modified,
            is_hidden,
            file_type,
            selected,
        });
    }

    Ok(entries)
}

/// Sort file entries according to the specified criteria.
pub fn sort_entries(entries: &mut [FileEntry], sort_by: &SortBy, sort_order: &SortOrder) {
    if *sort_by == SortBy::Name {
        if *sort_order == SortOrder::Ascending {
            entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
        } else {
            entries.sort_by_cached_key(|entry| {
                (!entry.is_dir, std::cmp::Reverse(entry.name.to_lowercase()))
            });
        }
        return;
    }
    entries.sort_by(|a, b| {
        // Directories always come first
        let dir_ordering = match (a.is_dir, b.is_dir) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        };

        let ordering = match sort_by {
            SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortBy::Size => a.size.cmp(&b.size),
            SortBy::DateModified => a.modified.cmp(&b.modified),
            SortBy::Type => {
                if dir_ordering != std::cmp::Ordering::Equal {
                    dir_ordering
                } else {
                    a.file_type.cmp(&b.file_type)
                }
            }
        };

        if *sort_order == SortOrder::Descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
}

/// Filter entries by search query (case-insensitive).
pub fn filter_entries(entries: &mut Vec<FileEntry>, search_query: &str) {
    if search_query.is_empty() {
        return;
    }

    let query_lower = search_query.to_lowercase();
    entries.retain(|entry| entry.name.to_lowercase().contains(&query_lower));
}
