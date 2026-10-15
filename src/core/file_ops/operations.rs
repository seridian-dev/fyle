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
    preflight(sources, destination)?;
    for source in sources {
        let file_name = source.file_name().ok_or_else(|| {
            FileManagerError::InvalidOperation("Invalid source file name".to_string())
        })?;

        let dest_path = destination.join(file_name);

        if source.is_dir() {
            copy_directory_recursive(source, &dest_path)?;
        } else {
            copy_new(source, &dest_path).with_context(|| {
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
    validate_copy_tree(source, destination)?;
    fs::create_dir(destination)
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
            copy_new(source_path, &dest_path).with_context(|| {
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
    preflight(sources, destination)?;
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
    validate_name(name)?;
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
    validate_name(name)?;
    let path = parent.join(name);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("Failed to create file {}", path.display()))?;
    Ok(path)
}

/// Rename a file or directory.
///
/// # Errors
/// Returns an error if the rename operation fails.
pub fn rename_file(old_path: &Path, new_name: &str) -> Result<PathBuf> {
    validate_name(new_name)?;
    let parent = old_path
        .parent()
        .ok_or_else(|| FileManagerError::Path("Cannot get parent directory".to_string()))?;
    let new_path = parent.join(new_name);

    ensure_absent(&new_path)?;
    fs::rename(old_path, &new_path).with_context(|| {
        format!(
            "Failed to rename {} to {}",
            old_path.display(),
            new_path.display()
        )
    })?;

    Ok(new_path)
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':', '\0'])
        || Path::new(name).is_absolute()
    {
        anyhow::bail!("Enter a single file or folder name, without path separators");
    }
    Ok(())
}

fn ensure_absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        Ok(_) => anyhow::bail!("Destination already exists: {}", path.display()),
    }
}

fn preflight(sources: &[PathBuf], destination: &Path) -> Result<()> {
    let destination = destination.canonicalize()?;
    anyhow::ensure!(destination.is_dir(), "Destination must be a directory");
    let mut names = std::collections::HashSet::new();
    for source in sources {
        let name = source.file_name().context("Source has no file name")?;
        anyhow::ensure!(names.insert(name), "Multiple sources have the same name");
        let target = destination.join(name);
        ensure_absent(&target)?;
        if source.is_dir() {
            validate_copy_tree(source, &target)?;
        } else {
            anyhow::ensure!(
                fs::symlink_metadata(source)?.is_file(),
                "Copy/move of special files or symlinks is not supported"
            );
        }
    }
    Ok(())
}

fn validate_copy_tree(source: &Path, destination: &Path) -> Result<()> {
    anyhow::ensure!(
        !fs::symlink_metadata(source)?.file_type().is_symlink(),
        "Copy/move of directory symlinks is not supported"
    );
    anyhow::ensure!(source.is_dir(), "Source must be a directory");
    let source = source.canonicalize()?;
    let parent = destination
        .parent()
        .context("Destination has no parent")?
        .canonicalize()?;
    anyhow::ensure!(
        !parent.starts_with(&source),
        "Cannot copy or move a directory into itself"
    );
    ensure_absent(destination)?;
    for entry in WalkDir::new(&source) {
        let entry = entry?;
        anyhow::ensure!(
            entry.file_type().is_file() || entry.file_type().is_dir(),
            "Copy/move of special files or symlinks is not supported: {}",
            entry.path().display()
        );
    }
    Ok(())
}

fn copy_new(source: &Path, destination: &Path) -> std::io::Result<u64> {
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let result = std::io::copy(&mut input, &mut output);
    if result.is_err() {
        drop(output);
        let _ = fs::remove_file(destination);
    } else {
        fs::set_permissions(destination, input.metadata()?.permissions())?;
    }
    result
}
