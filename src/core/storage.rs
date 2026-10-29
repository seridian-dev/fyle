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
                if stats.visited_entries.is_multiple_of(128) {
                    progress(stats);
                }
                continue;
            }
        };
        let file_type = entry.file_type();
        if file_type.is_symlink() || (!file_type.is_file() && !file_type.is_dir()) {
            if stats.visited_entries.is_multiple_of(128) {
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
                    if stats.visited_entries.is_multiple_of(128) {
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

        if stats.visited_entries.is_multiple_of(128) {
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

/// Depth limit for tree scans. Deep enough that only pathological
/// directory structures hit it; sizes below the cap are not counted.
pub const TREE_MAX_DEPTH: usize = 24;

/// A node in a recursive apparent-size tree. Directory sizes include all
/// descendants; children are sorted by descending size.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DiskNode {
    pub path: PathBuf,
    pub name: String,
    pub kind: StorageEntryKind,
    pub apparent_bytes: u64,
    pub children: Vec<DiskNode>,
}

/// Scan a folder into a nested tree without following symlinks. Sizes are
/// apparent file lengths; directories carry the sum of their descendants.
/// `max_depth` bounds recursion (use [`TREE_MAX_DEPTH`] for full scans).
pub fn scan_tree(
    root: &Path,
    max_depth: usize,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(StorageProgress),
) -> Result<DiskNode> {
    let root_metadata = std::fs::symlink_metadata(root)
        .with_context(|| format!("Cannot inspect scan root {}", root.display()))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        bail!("Storage scans require a real directory, not a symlink or file");
    }
    if cancelled.load(Ordering::Relaxed) {
        bail!("Storage scan cancelled");
    }
    let mut stats = StorageProgress::default();
    let tree = visit_tree_dir(
        root,
        root.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string()),
        0,
        max_depth,
        cancelled,
        &mut stats,
        &mut progress,
    )?;
    progress(stats);
    Ok(tree)
}

fn emit_tree_progress(progress: &mut dyn FnMut(StorageProgress), stats: &StorageProgress) {
    if stats.visited_entries.is_multiple_of(128) {
        progress(*stats);
    }
}

fn visit_tree_dir(
    path: &Path,
    name: String,
    depth: usize,
    max_depth: usize,
    cancelled: &AtomicBool,
    stats: &mut StorageProgress,
    progress: &mut dyn FnMut(StorageProgress),
) -> Result<DiskNode> {
    let mut children: Vec<DiskNode> = Vec::new();
    if depth < max_depth {
        let read = match std::fs::read_dir(path) {
            Ok(read) => read,
            Err(_) => {
                stats.unreadable_entries = stats.unreadable_entries.saturating_add(1);
                emit_tree_progress(progress, stats);
                return Ok(DiskNode {
                    path: path.to_path_buf(),
                    name,
                    kind: StorageEntryKind::Directory,
                    apparent_bytes: 0,
                    children,
                });
            }
        };
        for entry in read {
            if cancelled.load(Ordering::Relaxed) {
                bail!("Storage scan cancelled");
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    stats.unreadable_entries = stats.unreadable_entries.saturating_add(1);
                    emit_tree_progress(progress, stats);
                    continue;
                }
            };
            stats.visited_entries = stats.visited_entries.saturating_add(1);
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => {
                    stats.unreadable_entries = stats.unreadable_entries.saturating_add(1);
                    emit_tree_progress(progress, stats);
                    continue;
                }
            };
            if file_type.is_symlink() || (!file_type.is_file() && !file_type.is_dir()) {
                emit_tree_progress(progress, stats);
                continue;
            }
            let child_name = entry.file_name().to_string_lossy().into_owned();
            if file_type.is_dir() {
                children.push(visit_tree_dir(
                    &entry.path(),
                    child_name,
                    depth + 1,
                    max_depth,
                    cancelled,
                    stats,
                    progress,
                )?);
            } else {
                let length = match entry.metadata() {
                    Ok(metadata) => metadata.len(),
                    Err(_) => {
                        stats.unreadable_entries = stats.unreadable_entries.saturating_add(1);
                        emit_tree_progress(progress, stats);
                        continue;
                    }
                };
                stats.apparent_bytes = stats.apparent_bytes.saturating_add(length);
                children.push(DiskNode {
                    path: entry.path(),
                    name: child_name,
                    kind: StorageEntryKind::File,
                    apparent_bytes: length,
                    children: Vec::new(),
                });
            }
            emit_tree_progress(progress, stats);
        }
    }
    children.sort_by(|left, right| {
        right
            .apparent_bytes
            .cmp(&left.apparent_bytes)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(DiskNode {
        path: path.to_path_buf(),
        name,
        kind: StorageEntryKind::Directory,
        apparent_bytes: children
            .iter()
            .map(|child| child.apparent_bytes)
            .sum::<u64>(),
        children,
    })
}

/// Runs a tree scan off the UI thread and reports periodic progress.
pub struct TreeScanJob {
    progress: mpsc::Receiver<StorageProgress>,
    result: mpsc::Receiver<Result<DiskNode>>,
    cancelled: Arc<AtomicBool>,
}

impl TreeScanJob {
    #[must_use]
    pub fn start(root: PathBuf) -> Self {
        let (progress_tx, progress) = mpsc::channel();
        let (result_tx, result) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        thread::spawn(move || {
            let outcome = scan_tree(&root, TREE_MAX_DEPTH, &worker_cancelled, |update| {
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

    pub fn poll_result(&self) -> Option<Result<DiskNode>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(anyhow::anyhow!(
                "Storage tree scan worker stopped unexpectedly"
            ))),
        }
    }
}

impl Drop for TreeScanJob {
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

    #[test]
    fn tree_reports_nested_sizes_sorted_by_descending_size() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(root.join("alpha.txt"), "12345").unwrap();
        std::fs::create_dir_all(root.join("projects/build")).unwrap();
        std::fs::write(root.join("projects/a.rs"), "1234567").unwrap();
        std::fs::write(root.join("projects/build/out.bin"), "12345678901").unwrap();
        std::fs::create_dir(root.join("empty")).unwrap();

        let tree = scan_tree(root, TREE_MAX_DEPTH, &AtomicBool::new(false), |_| {}).unwrap();

        assert_eq!(tree.apparent_bytes, 23);
        assert_eq!(tree.kind, StorageEntryKind::Directory);
        let names: Vec<&str> = tree.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["projects", "alpha.txt", "empty"]);
        let projects = &tree.children[0];
        assert_eq!(projects.apparent_bytes, 18);
        let build = &projects.children[0];
        assert_eq!(build.name, "build");
        assert_eq!(build.apparent_bytes, 11);
        assert_eq!(build.children[0].name, "out.bin");
        assert_eq!(projects.children[1].name, "a.rs");
        assert_eq!(projects.children[1].apparent_bytes, 7);
    }

    #[cfg(unix)]
    #[test]
    fn tree_does_not_follow_symlinks() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret.bin"), "secret data").unwrap();
        symlink(&outside, root.join("linked-folder")).unwrap();
        symlink(outside.join("secret.bin"), root.join("linked-file")).unwrap();

        let tree = scan_tree(&root, TREE_MAX_DEPTH, &AtomicBool::new(false), |_| {}).unwrap();

        assert_eq!(tree.apparent_bytes, 0);
        assert!(tree.children.is_empty());
    }

    #[test]
    fn tree_respects_max_depth() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(root.join("alpha.txt"), "12345").unwrap();
        std::fs::create_dir_all(root.join("projects/build")).unwrap();
        std::fs::write(root.join("projects/build/out.bin"), "12345678901").unwrap();

        let tree = scan_tree(root, 1, &AtomicBool::new(false), |_| {}).unwrap();

        assert_eq!(tree.apparent_bytes, 5);
        // Sort is size-descending: the counted file outranks the capped folder.
        assert_eq!(tree.children[0].name, "alpha.txt");
        let projects = tree
            .children
            .iter()
            .find(|child| child.name == "projects")
            .unwrap();
        assert!(projects.children.is_empty());
        assert_eq!(projects.apparent_bytes, 0);
    }

    #[cfg(unix)]
    #[test]
    fn tree_rejects_a_symlink_as_its_root() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let real = temp.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = temp.path().join("link");
        symlink(&real, &link).unwrap();
        assert!(scan_tree(&link, TREE_MAX_DEPTH, &AtomicBool::new(false), |_| {}).is_err());
    }

    #[test]
    fn tree_scan_honours_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let cancelled = AtomicBool::new(true);
        assert!(scan_tree(temp.path(), TREE_MAX_DEPTH, &cancelled, |_| {}).is_err());
    }
}
