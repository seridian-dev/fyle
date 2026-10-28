//! Headless folder-size scanning shared by the GUI and CLI.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

const MAX_ROOT_ENTRIES: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageEntryKind {
    File,
    Directory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StorageEntry {
    pub path: PathBuf,
    pub name: String,
    pub kind: StorageEntryKind,
    pub apparent_bytes: u64,
    pub file_count: u64,
    pub directory_count: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct StorageProgress {
    pub visited_entries: u64,
    pub apparent_bytes: u64,
    pub unreadable_entries: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct StorageSummary {
    pub root: PathBuf,
    pub apparent_bytes: u64,
    pub file_count: u64,
    pub directory_count: u64,
    pub unreadable_entries: u64,
    pub omitted_root_entries: u64,
    pub entries: Vec<StorageEntry>,
}

/// Scan a folder without following symlinks. Results report apparent file
/// sizes and one level of children so the UI can draw a bounded size map.
pub fn scan_folder(
    root: &Path,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(StorageProgress),
) -> Result<StorageSummary> {
    let root_metadata = std::fs::symlink_metadata(root)
        .with_context(|| format!("Cannot inspect scan root {}", root.display()))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        bail!("Storage scans require a real directory, not a symlink or file");
    }

    let mut summary = StorageSummary {
        root: root.to_path_buf(),
        ..StorageSummary::default()
    };
    let mut children: HashMap<PathBuf, StorageEntry> = HashMap::new();
    let mut stats = StorageProgress::default();

    for result in walkdir::WalkDir::new(root).follow_links(false).min_depth(1) {
        if cancelled.load(Ordering::Relaxed) {
            bail!("Storage scan cancelled");
        }
        stats.visited_entries = stats.visited_entries.saturating_add(1);
        let entry = match result {
            Ok(entry) => entry,
            Err(_) => {
                stats.unreadable_entries = stats.unreadable_entries.saturating_add(1);
                if stats.visited_entries % 128 == 0 {
                    progress(stats);
                }
                continue;
            }
        };
        let file_type = entry.file_type();
        if file_type.is_symlink() || (!file_type.is_file() && !file_type.is_dir()) {
            if stats.visited_entries % 128 == 0 {
                progress(stats);
            }
            continue;
        }

        let relative = match entry.path().strip_prefix(root) {
            Ok(relative) => relative,
            Err(_) => continue,
        };
        let mut components = relative.components();
        let Some(Component::Normal(first_component)) = components.next() else {
            continue;
        };
        let child_path = root.join(first_component);
        if !children.contains_key(&child_path) && entry.depth() == 1 {
            if children.len() >= MAX_ROOT_ENTRIES {
                summary.omitted_root_entries = summary.omitted_root_entries.saturating_add(1);
            } else {
                let kind = if file_type.is_dir() {
                    StorageEntryKind::Directory
                } else {
                    StorageEntryKind::File
                };
                children.insert(
                    child_path.clone(),
                    StorageEntry {
                        path: child_path.clone(),
                        name: first_component.to_string_lossy().into_owned(),
                        kind,
                        apparent_bytes: 0,
                        file_count: 0,
                        directory_count: 0,
                    },
                );
            }
        }

        if file_type.is_dir() {
            summary.directory_count = summary.directory_count.saturating_add(1);
            if entry.depth() > 1 {
                if let Some(child) = children.get_mut(&child_path) {
                    child.directory_count = child.directory_count.saturating_add(1);
                }
            }
        } else {
            let length = match entry.metadata() {
                Ok(metadata) => metadata.len(),
                Err(_) => {
                    stats.unreadable_entries = stats.unreadable_entries.saturating_add(1);
                    if stats.visited_entries % 128 == 0 {
                        progress(stats);
                    }
                    continue;
                }
            };
            summary.apparent_bytes = summary.apparent_bytes.saturating_add(length);
            summary.file_count = summary.file_count.saturating_add(1);
            stats.apparent_bytes = stats.apparent_bytes.saturating_add(length);
            if let Some(child) = children.get_mut(&child_path) {
                child.apparent_bytes = child.apparent_bytes.saturating_add(length);
                child.file_count = child.file_count.saturating_add(1);
            }
        }

        if stats.visited_entries % 128 == 0 {
            progress(stats);
        }
    }

    summary.unreadable_entries = stats.unreadable_entries;
    progress(stats);
    summary.entries = children.into_values().collect();
    summary.entries.sort_by(|left, right| {
        right
            .apparent_bytes
            .cmp(&left.apparent_bytes)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(summary)
}

/// Runs a folder scan off the UI thread and reports periodic progress.
pub struct StorageScanJob {
    progress: mpsc::Receiver<StorageProgress>,
    result: mpsc::Receiver<Result<StorageSummary>>,
    cancelled: Arc<AtomicBool>,
}

impl StorageScanJob {
    #[must_use]
    pub fn start(root: PathBuf) -> Self {
        let (progress_tx, progress) = mpsc::channel();
        let (result_tx, result) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        thread::spawn(move || {
            let outcome = scan_folder(&root, &worker_cancelled, |update| {
                let _ = progress_tx.send(update);
            });
            let _ = result_tx.send(outcome);
        });
        Self {
            progress,
            result,
            cancelled,
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn poll_progress(&self) -> Option<StorageProgress> {
        let mut latest = None;
        while let Ok(progress) = self.progress.try_recv() {
            latest = Some(progress);
        }
        latest
    }

    pub fn poll_result(&self) -> Option<Result<StorageSummary>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(anyhow::anyhow!(
                "Storage scan worker stopped unexpectedly"
            ))),
        }
    }
}

impl Drop for StorageScanJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn scan_reports_apparent_sizes_and_direct_children() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(root.join("alpha.txt"), "12345").unwrap();
        std::fs::create_dir_all(root.join("projects/build")).unwrap();
        std::fs::write(root.join("projects/a.rs"), "1234567").unwrap();
        std::fs::write(root.join("projects/build/out.bin"), "12345678901").unwrap();
        std::fs::create_dir(root.join("empty")).unwrap();

        let summary = scan_folder(root, &AtomicBool::new(false), |_| {}).unwrap();

        assert_eq!(summary.apparent_bytes, 23);
        assert_eq!(summary.file_count, 3);
        assert_eq!(summary.directory_count, 3);
        assert_eq!(summary.entries.len(), 3);
        assert_eq!(summary.entries[0].path, root.join("projects"));
        assert_eq!(summary.entries[0].apparent_bytes, 18);
        assert_eq!(summary.entries[0].file_count, 2);
        assert_eq!(summary.entries[0].directory_count, 1);
        assert_eq!(summary.entries[1].path, root.join("alpha.txt"));
        assert_eq!(summary.entries[1].apparent_bytes, 5);
        assert_eq!(summary.entries[2].path, root.join("empty"));
        assert_eq!(summary.entries[2].apparent_bytes, 0);
    }

    #[cfg(unix)]
    #[test]
    fn scan_does_not_follow_symlinked_directories_or_files() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret.bin"), "secret data").unwrap();
        symlink(&outside, root.join("linked-folder")).unwrap();
        symlink(outside.join("secret.bin"), root.join("linked-file")).unwrap();

        let summary = scan_folder(&root, &AtomicBool::new(false), |_| {}).unwrap();

        assert_eq!(summary.apparent_bytes, 0);
        assert_eq!(summary.file_count, 0);
        assert!(summary.entries.is_empty());
    }

    #[test]
    fn scan_rejects_a_symlink_as_its_root() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let temp = tempfile::tempdir().unwrap();
            let real = temp.path().join("real");
            std::fs::create_dir(&real).unwrap();
            let link = temp.path().join("link");
            symlink(&real, &link).unwrap();
            assert!(scan_folder(&link, &AtomicBool::new(false), |_| {}).is_err());
        }
    }
}
