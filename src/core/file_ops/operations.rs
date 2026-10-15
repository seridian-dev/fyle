//! File system operations (copy, move, delete, create).

use crate::types::FileManagerError;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Copy files to a destination directory.
///
/// # Errors
/// Returns an error if:
/// - Source file name is invalid
/// - File/directory copy operation fails
pub fn copy_files(sources: &[PathBuf], destination: &Path) -> Result<()> {
    for source in sources {
        let file_name = source.file_name().ok_or_else(|| {
            FileManagerError::InvalidOperation("Invalid source file name".to_string())
        })?;

        let dest_path = destination.join(file_name);

        if source.is_dir() {
            copy_directory_recursive(source, &dest_path)?;
        } else {
            fs::copy(source, &dest_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    source.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

/// Recursively copy a directory and its contents.
///
/// # Errors
/// Returns an error if any file or directory creation fails.
pub fn copy_directory_recursive(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("Failed to create directory {}", destination.display()))?;

    for entry in WalkDir::new(source) {
        let entry =
            entry.with_context(|| format!("Failed to walk directory {}", source.display()))?;
        let source_path = entry.path();
        let relative_path = source_path
            .strip_prefix(source)
            .with_context(|| format!("Failed to strip prefix from {}", source_path.display()))?;
        let dest_path = destination.join(relative_path);

        if source_path.is_dir() {
            fs::create_dir_all(&dest_path)
                .with_context(|| format!("Failed to create directory {}", dest_path.display()))?;
        } else {
            fs::copy(source_path, &dest_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    source_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

/// Move files to a destination directory.
///
/// # Errors
/// Returns an error if:
/// - Source file name is invalid
/// - Rename operation fails
pub fn move_files(sources: &[PathBuf], destination: &Path) -> Result<()> {
    for source in sources {
        let file_name = source.file_name().ok_or_else(|| {
            FileManagerError::InvalidOperation("Invalid source file name".to_string())
        })?;

        let dest_path = destination.join(file_name);

        fs::rename(source, &dest_path).with_context(|| {
            format!(
                "Failed to move {} to {}",
                source.display(),
                dest_path.display()
            )
        })?;
    }
    Ok(())
}

/// Delete files and directories.
///
/// For safer deletion, consider using `trash_files` instead.
///
/// # Errors
/// Returns an error if any delete operation fails.
pub fn delete_files(paths: &[PathBuf]) -> Result<()> {
    for path in paths {
        if path.is_dir() {
            fs::remove_dir_all(path)
                .with_context(|| format!("Failed to delete directory {}", path.display()))?;
        } else {
            fs::remove_file(path)
                .with_context(|| format!("Failed to delete file {}", path.display()))?;
        }
    }
    Ok(())
}

/// Move files to trash instead of permanent deletion.
///
/// # Errors
/// Returns an error if the trash operation fails.
#[cfg(feature = "trash")]
pub fn trash_files(paths: &[PathBuf]) -> Result<()> {
    for path in paths {
        trash::delete(path).map_err(|e| FileManagerError::TrashFailed(e.to_string()))?;
    }
    Ok(())
}

/// Create a new directory.
///
/// # Errors
/// Returns an error if directory creation fails.
pub fn create_directory(parent: &Path, name: &str) -> Result<PathBuf> {
    let path = parent.join(name);
    fs::create_dir(&path)
        .with_context(|| format!("Failed to create directory {}", path.display()))?;
    Ok(path)
}

/// Create a new empty file.
///
/// # Errors
/// Returns an error if file creation fails.
pub fn create_file(parent: &Path, name: &str) -> Result<PathBuf> {
    let path = parent.join(name);
    fs::File::create(&path).with_context(|| format!("Failed to create file {}", path.display()))?;
    Ok(path)
}

/// Rename a file or directory.
///
/// # Errors
/// Returns an error if the rename operation fails.
pub fn rename_file(old_path: &Path, new_name: &str) -> Result<PathBuf> {
    let parent = old_path
        .parent()
        .ok_or_else(|| FileManagerError::Path("Cannot get parent directory".to_string()))?;
    let new_path = parent.join(new_name);

    fs::rename(old_path, &new_path).with_context(|| {
        format!(
            "Failed to rename {} to {}",
            old_path.display(),
            new_path.display()
        )
    })?;

    Ok(new_path)
}
