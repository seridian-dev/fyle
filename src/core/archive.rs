//! Safe local archive operations used by file views and the CLI.

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

static STAGING_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum ArchiveOutput {
    Zip,
    SevenZip,
}

/// Create a ZIP archive containing each source and its contents.
///
/// The destination is created exclusively; existing files are never replaced.
/// Symlinks and special files are rejected so the archive has predictable
/// contents across Linux desktops.
pub fn create_zip(sources: &[PathBuf], archive_path: &Path) -> Result<()> {
    create_archive(sources, archive_path, ArchiveOutput::Zip)
}

/// Create a native 7z archive. Existing files are never replaced.
pub fn create_7z(sources: &[PathBuf], archive_path: &Path) -> Result<()> {
    create_archive(sources, archive_path, ArchiveOutput::SevenZip)
}

fn create_archive(sources: &[PathBuf], archive_path: &Path, output: ArchiveOutput) -> Result<()> {
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
    let result = match output {
        ArchiveOutput::Zip => write_zip(file, &sources),
        ArchiveOutput::SevenZip => write_7z(file, &sources),
    };
    if result.is_err() {
        let _ = fs::remove_file(&destination);
    }
    result.with_context(|| format!("Failed to create archive {}", destination.display()))
}

/// Extract a ZIP, 7z or tar/tar.gz archive into a new sibling folder.
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
        ArchiveKind::SevenZip => extract_7z(archive_path, &staging),
        ArchiveKind::Rar => extract_rar(archive_path, &staging),
        ArchiveKind::Tar => extract_tar(File::open(archive_path)?, &staging),
        ArchiveKind::TarGzip => extract_tar(GzDecoder::new(File::open(archive_path)?), &staging),
        ArchiveKind::Unsupported => bail!(
            "Unsupported archive format: {} (supported: .zip, .7z, .rar, .tar, .tar.gz, .tgz)",
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

fn write_7z(file: File, sources: &[PathBuf]) -> Result<()> {
    let mut writer =
        sevenz_rust2::ArchiveWriter::new(file).context("Cannot initialize 7z writer")?;
    for source in sources {
        let parent = source
            .parent()
            .context("Selected item has no parent folder")?;
        for entry in WalkDir::new(source).sort_by_file_name() {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type().is_symlink()
                || (!entry.file_type().is_file() && !entry.file_type().is_dir())
            {
                bail!("Cannot archive links or special files: {}", path.display());
            }
            let relative = path.strip_prefix(parent)?;
            let name = zip_name(relative, entry.file_type().is_dir())?;
            let archive_entry = if entry.file_type().is_dir() {
                sevenz_rust2::ArchiveEntry::new_directory(&name)
            } else {
                sevenz_rust2::ArchiveEntry::from_path(path, name)
            };
            let reader = if entry.file_type().is_file() {
                Some(File::open(path)?)
            } else {
                None
            };
            writer
                .push_archive_entry(archive_entry, reader)
                .with_context(|| format!("Cannot add {} to 7z archive", path.display()))?;
        }
    }
    writer.finish().context("Cannot finish 7z archive")?;
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

fn extract_7z(archive_path: &Path, staging: &Path) -> Result<()> {
    sevenz_rust2::decompress_file_with_extract_fn(archive_path, staging, |entry, reader, _| {
        if entry.is_anti_item {
            return Err(sevenz_error("7z anti-items are not supported"));
        }
        let relative = normalize_seven_zip_path(&entry.name)
            .map_err(|error| sevenz_error(error.to_string()))?;
        let output = staging.join(relative);
        if entry.is_directory {
            fs::create_dir_all(output).map_err(sevenz_rust2::Error::from)?;
        } else {
            create_archive_file(&output, reader)
                .map_err(|error| sevenz_error(error.to_string()))?;
        }
        Ok(true)
    })
    .context("Invalid or unsupported 7z archive")
}

fn sevenz_error(message: impl Into<String>) -> sevenz_rust2::Error {
    sevenz_rust2::Error::from(io::Error::new(io::ErrorKind::InvalidData, message.into()))
}

fn normalize_seven_zip_path(name: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        !name.is_empty() && !name.contains(['\\', '\0', ':']),
        "7z archive contains an unsafe path"
    );
    let path = Path::new(name);
    validate_relative_path(path)?;
    Ok(path.to_owned())
}

fn extract_rar(archive_path: &Path, staging: &Path) -> Result<()> {
    if command_exists("bsdtar") {
        extract_rar_with_bsdtar(archive_path, staging)
    } else if let Some(seven_zip) = ["7zz", "7z", "7za"]
        .into_iter()
        .find(|program| command_exists(program))
    {
        extract_rar_with_7zip(seven_zip, archive_path, staging)
    } else {
        bail!("RAR extraction needs bsdtar (libarchive) or 7-Zip (7z/7zz)")
    }
}

fn extract_rar_with_bsdtar(archive_path: &Path, staging: &Path) -> Result<()> {
    let listing = Command::new("bsdtar")
        .args(["-tf"])
        .arg(archive_path)
        .stdin(Stdio::null())
        .output()
        .context("Could not start bsdtar to inspect the RAR archive")?;
    anyhow::ensure!(
        listing.status.success(),
        "bsdtar could not read the RAR archive: {}",
        String::from_utf8_lossy(&listing.stderr).trim()
    );
    for path in String::from_utf8_lossy(&listing.stdout).lines() {
        let _ = normalize_seven_zip_path(path)?;
    }

    let details = Command::new("bsdtar")
        .args(["-tvf"])
        .arg(archive_path)
        .stdin(Stdio::null())
        .output()
        .context("Could not inspect RAR entry types")?;
    anyhow::ensure!(
        details.status.success(),
        "bsdtar could not inspect RAR entries: {}",
        String::from_utf8_lossy(&details.stderr).trim()
    );
    for line in String::from_utf8_lossy(&details.stdout).lines() {
        let file_type = line.chars().next().unwrap_or(' ');
        anyhow::ensure!(
            file_type == '-' || file_type == 'd',
            "RAR archives containing links or special files are not supported"
        );
    }

    let output = Command::new("bsdtar")
        .args(["-xf"])
        .arg(archive_path)
        .arg("-C")
        .arg(staging)
        .stdin(Stdio::null())
        .output()
        .context("Could not start bsdtar to extract the RAR archive")?;
    anyhow::ensure!(
        output.status.success(),
        "bsdtar could not extract the RAR archive: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    validate_extracted_tree(staging)
}

fn extract_rar_with_7zip(seven_zip: &str, archive_path: &Path, staging: &Path) -> Result<()> {
    let listing = Command::new(seven_zip)
        .args(["l", "-slt"])
        .arg(archive_path)
        .stdin(Stdio::null())
        .output()
        .context("Could not start 7-Zip to inspect the RAR archive")?;
    anyhow::ensure!(
        listing.status.success(),
        "7-Zip could not read the RAR archive: {}",
        String::from_utf8_lossy(&listing.stderr).trim()
    );
    validate_7z_listing(&String::from_utf8_lossy(&listing.stdout))?;

    let output = Command::new(seven_zip)
        .arg("x")
        .arg("-y")
        .arg(format!("-o{}", staging.display()))
        .arg(archive_path)
        .stdin(Stdio::null())
        .output()
        .context("Could not start 7-Zip to extract the RAR archive")?;
    anyhow::ensure!(
        output.status.success(),
        "7-Zip could not extract the RAR archive: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    validate_extracted_tree(staging)
}

fn command_exists(program: &str) -> bool {
    Command::new(program)
        .arg("--help")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn validate_7z_listing(listing: &str) -> Result<()> {
    let mut in_entries = false;
    let mut entry = Vec::new();
    for line in listing.lines().chain(std::iter::once("")) {
        if line.starts_with("----------") {
            in_entries = true;
            entry.clear();
            continue;
        }
        if !in_entries {
            continue;
        }
        if line.is_empty() {
            validate_7z_listing_entry(&entry)?;
            entry.clear();
        } else {
            entry.push(line);
        }
    }
    Ok(())
}

fn validate_7z_listing_entry(lines: &[&str]) -> Result<()> {
    for line in lines {
        if let Some(path) = line.strip_prefix("Path = ") {
            let _ = normalize_seven_zip_path(path)?;
        }
        if let Some(attributes) = line.strip_prefix("Attributes = ") {
            anyhow::ensure!(
                !attributes
                    .split_whitespace()
                    .any(|attribute| attribute.starts_with('l') || attribute.contains("LINK")),
                "RAR archives containing symbolic links are not supported"
            );
        }
        anyhow::ensure!(
            !line.starts_with("Symbolic Link = ") && !line.starts_with("Hard Link = "),
            "RAR archives containing links are not supported"
        );
    }
    Ok(())
}

fn validate_extracted_tree(root: &Path) -> Result<()> {
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        anyhow::ensure!(
            entry.file_type().is_file() || entry.file_type().is_dir(),
            "RAR archives containing links or special files are not supported"
        );
    }
    Ok(())
}

fn create_archive_file(output: &Path, reader: &mut (impl Read + ?Sized)) -> Result<()> {
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
    let suffix = [".tar.gz", ".tgz", ".tar", ".7z", ".rar", ".zip"]
        .into_iter()
        .find(|suffix| lower.ends_with(suffix))
        .context("Unsupported archive format (supported: .zip, .7z, .rar, .tar, .tar.gz, .tgz)")?;
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
    SevenZip,
    Rar,
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
    } else if name.ends_with(".7z") {
        ArchiveKind::SevenZip
    } else if name.ends_with(".rar") {
        ArchiveKind::Rar
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
        for path in [
            "backup.ZIP",
            "backup.7Z",
            "backup.RAR",
            "backup.tar",
            "backup.TAR.GZ",
            "backup.tgz",
        ] {
            assert!(is_supported_archive(Path::new(path)), "{path}");
        }
        assert!(!is_supported_archive(Path::new("backup.iso")));
    }

    #[test]
    fn rar_listing_rejects_escaping_paths_and_links() {
        let traversal = "----------\nPath = ../outside\nSize = 1\n\n";
        assert!(validate_7z_listing(traversal).is_err());

        let link = "----------\nPath = link\nAttributes = A_ lrwxrwxrwx\n\n";
        assert!(validate_7z_listing(link).is_err());
    }
}
