//! Safe local archive operations used by file views and the CLI.

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

static STAGING_ID: AtomicU64 = AtomicU64::new(0);

/// Create a ZIP archive containing each source and its contents.
///
/// The destination is created exclusively; existing files are never replaced.
/// Symlinks and special files are rejected so the archive has predictable
/// contents across Linux desktops.
pub fn create_zip(sources: &[PathBuf], archive_path: &Path) -> Result<()> {
    anyhow::ensure!(!sources.is_empty(), "Select at least one item to archive");
    let archive_parent = archive_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .context("Cannot resolve archive destination folder")?;
    anyhow::ensure!(
        archive_parent.is_dir(),
        "Archive destination must be a folder"
    );
    let archive_name = archive_path
        .file_name()
        .context("Archive destination must have a file name")?;
    let destination = archive_parent.join(archive_name);
    let sources = validate_sources(sources, &destination)?;

    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .with_context(|| format!("Cannot create archive {}", destination.display()))?;
    let result = write_zip(file, &sources);
    if result.is_err() {
        let _ = fs::remove_file(&destination);
    }
    result.with_context(|| format!("Failed to create archive {}", destination.display()))
}

/// Extract a ZIP or tar/tar.gz archive into a new sibling folder.
///
/// Extraction is staged in a private sibling directory and only published
/// after every entry succeeds. Existing destinations are preserved. Archive
/// paths must be relative and links/special files are rejected.
pub fn extract_archive(archive_path: &Path, destination: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(archive_path)
        .with_context(|| format!("Cannot read archive {}", archive_path.display()))?;
    anyhow::ensure!(metadata.is_file(), "Archive must be a regular file");
    let destination = destination
        .canonicalize()
        .context("Cannot resolve extraction folder")?;
    anyhow::ensure!(
        destination.is_dir(),
        "Extraction destination must be a folder"
    );
    let output_name = extracted_folder_name(archive_path)?;
    let output = destination.join(output_name);
    ensure_absent(&output)?;
    let staging = create_staging_directory(&destination)?;

    let result = (|| match archive_kind(archive_path) {
        ArchiveKind::Zip => extract_zip(archive_path, &staging),
        ArchiveKind::Tar => extract_tar(File::open(archive_path)?, &staging),
        ArchiveKind::TarGzip => extract_tar(GzDecoder::new(File::open(archive_path)?), &staging),
        ArchiveKind::Unsupported => bail!(
            "Unsupported archive format: {} (supported: .zip, .tar, .tar.gz, .tgz)",
            archive_path.display()
        ),
    })();

    if let Err(error) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error).with_context(|| format!("Could not extract {}", archive_path.display()));
    }
    if let Err(error) = fs::rename(&staging, &output) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error)
            .with_context(|| format!("Cannot publish extracted folder {}", output.display()));
    }
    Ok(output)
}

/// Whether Fyle has a native extraction path for this extension.
#[must_use]
pub fn is_supported_archive(path: &Path) -> bool {
    !matches!(archive_kind(path), ArchiveKind::Unsupported)
}

fn write_zip(file: File, sources: &[PathBuf]) -> Result<()> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(6));
    let mut writer = ZipWriter::new(file);
    for source in sources {
        let parent = source
            .parent()
            .context("Selected item has no parent folder")?;
        let root_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .context("ZIP item names must be valid UTF-8")?;
        if source.is_dir() {
            let root = source.strip_prefix(parent)?;
            writer.add_directory(zip_name(root, true)?, options)?;
            for entry in WalkDir::new(source).min_depth(1).sort_by_file_name() {
                let entry = entry?;
                let path = entry.path();
                let relative = path.strip_prefix(parent)?;
                if entry.file_type().is_symlink()
                    || (!entry.file_type().is_file() && !entry.file_type().is_dir())
                {
                    bail!("Cannot archive links or special files: {}", path.display());
                }
                let name = zip_name(relative, entry.file_type().is_dir())?;
                if entry.file_type().is_dir() {
                    writer.add_directory(name, options)?;
                } else {
                    writer.start_file(name, options)?;
                    io::copy(&mut File::open(path)?, &mut writer)?;
                }
            }
        } else {
            writer.start_file(root_name, options)?;
            io::copy(&mut File::open(source)?, &mut writer)?;
        }
    }
    writer.finish()?;
    Ok(())
}

fn validate_sources(sources: &[PathBuf], destination: &Path) -> Result<Vec<PathBuf>> {
    let mut names = std::collections::HashSet::new();
    let mut validated = Vec::with_capacity(sources.len());
    for source in sources {
        let metadata = fs::symlink_metadata(source)
            .with_context(|| format!("Cannot read selected item {}", source.display()))?;
        anyhow::ensure!(
            metadata.is_file() || metadata.is_dir(),
            "Cannot archive links or special files: {}",
            source.display()
        );
        let name = source
            .file_name()
            .context("Selected item has no file name")?;
        anyhow::ensure!(
            names.insert(name.to_os_string()),
            "Selected items have duplicate names"
        );
        let canonical = source.canonicalize()?;
        anyhow::ensure!(
            !destination.starts_with(&canonical) || !canonical.is_dir(),
            "Cannot create an archive inside a folder being archived"
        );
        if canonical.is_dir() {
            for entry in WalkDir::new(&canonical) {
                let entry = entry?;
                anyhow::ensure!(
                    entry.file_type().is_dir() || entry.file_type().is_file(),
                    "Cannot archive links or special files: {}",
                    entry.path().display()
                );
                anyhow::ensure!(
                    entry.file_name().to_str().is_some(),
                    "ZIP item names must be valid UTF-8"
                );
            }
        } else {
            anyhow::ensure!(
                name.to_str().is_some(),
                "ZIP item names must be valid UTF-8"
            );
        }
        validated.push(canonical);
    }
    Ok(validated)
}

fn zip_name(path: &Path, directory: bool) -> Result<String> {
    let parts = path
        .components()
        .map(|component| match component {
            Component::Normal(part) => part.to_str().map(str::to_owned),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .context("Archive path must be relative")?;
    let mut name = parts.join("/");
    anyhow::ensure!(!name.is_empty(), "Archive entry has an empty name");
    if directory {
        name.push('/');
    }
    Ok(name)
}

fn extract_zip(archive_path: &Path, staging: &Path) -> Result<()> {
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file).context("Invalid ZIP archive")?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.name().contains(['\\', '\0']) {
            bail!("Unsafe archive entry name: {}", entry.name());
        }
        if let Some(mode) = entry.unix_mode() {
            let file_type = mode & 0o170000;
            if file_type != 0 && file_type != 0o100000 && file_type != 0o040000 {
                bail!(
                    "Archive links and special files are not supported: {}",
                    entry.name()
                );
            }
        }
        let relative = entry
            .enclosed_name()
            .context("Archive contains an absolute or escaping path")?;
        validate_relative_path(&relative)?;
        let output = staging.join(relative);
        if entry.is_dir()
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o040000)
        {
            fs::create_dir_all(&output)?;
        } else {
            create_archive_file(&output, &mut entry)?;
        }
    }
    Ok(())
}

fn extract_tar(reader: impl Read, staging: &Path) -> Result<()> {
    let mut archive = tar::Archive::new(reader);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let kind = entry.header().entry_type();
        let path = entry.path()?.into_owned();
        let relative = normalize_tar_path(&path)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let output = staging.join(relative);
        if kind.is_dir() {
            fs::create_dir_all(output)?;
        } else if kind.is_file() {
            create_archive_file(&output, &mut entry)?;
        } else {
            bail!(
                "Archive links and special files are not supported: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn create_archive_file(output: &Path, reader: &mut impl Read) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .with_context(|| format!("Cannot create extracted file {}", output.display()))?;
    io::copy(reader, &mut file)?;
    file.flush()?;
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<()> {
    anyhow::ensure!(
        !path.as_os_str().is_empty(),
        "Archive entry has an empty name"
    );
    anyhow::ensure!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "Archive contains an absolute or escaping path"
    );
    Ok(())
}

fn normalize_tar_path(path: &Path) -> Result<PathBuf> {
    if path.to_string_lossy().contains(['\\', '\0']) {
        bail!("Archive contains an unsafe path");
    }
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            _ => bail!("Archive contains an absolute or escaping path"),
        }
    }
    Ok(relative)
}

fn create_staging_directory(parent: &Path) -> Result<PathBuf> {
    for _ in 0..32 {
        let id = STAGING_ID.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".fyle-extract-{}-{id}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("Could not reserve an extraction staging folder")
}

fn ensure_absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => bail!("Destination already exists: {}", path.display()),
    }
}

fn extracted_folder_name(path: &Path) -> Result<String> {
    let name = path.file_name().context("Archive has no file name")?;
    let lower = name.to_string_lossy().to_ascii_lowercase();
    let suffix = [".tar.gz", ".tgz", ".tar", ".zip"]
        .into_iter()
        .find(|suffix| lower.ends_with(suffix))
        .context("Unsupported archive format (supported: .zip, .tar, .tar.gz, .tgz)")?;
    let name = name.to_string_lossy();
    let stem = &name[..name.len() - suffix.len()];
    anyhow::ensure!(
        !stem.is_empty() && stem != "." && stem != "..",
        "Archive needs a valid base name"
    );
    Ok(format!("{stem}-extracted"))
}

#[derive(Clone, Copy)]
enum ArchiveKind {
    Zip,
    Tar,
    TarGzip,
    Unsupported,
}

fn archive_kind(path: &Path) -> ArchiveKind {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    if name.ends_with(".zip") {
        ArchiveKind::Zip
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        ArchiveKind::TarGzip
    } else if name.ends_with(".tar") {
        ArchiveKind::Tar
    } else {
        ArchiveKind::Unsupported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_archive_detection_is_case_insensitive() {
        for path in ["backup.ZIP", "backup.tar", "backup.TAR.GZ", "backup.tgz"] {
            assert!(is_supported_archive(Path::new(path)), "{path}");
        }
        assert!(!is_supported_archive(Path::new("backup.7z")));
        assert!(!is_supported_archive(Path::new("backup.rar")));
    }
}
