//! Bounded, read-only Git status and diff queries for the in-app review view.
use anyhow::{bail, ensure, Context, Result};
use std::{
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
};

const MAX_STATUS_BYTES: usize = 4 * 1024 * 1024;
const MAX_DIFF_BYTES: usize = 4 * 1024 * 1024;
const MAX_UNTRACKED_BYTES: usize = 1024 * 1024;
const MAX_CHANGES: usize = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

impl ChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Modified => "Modified",
            Self::Deleted => "Deleted",
            Self::Renamed => "Renamed",
            Self::Untracked => "Untracked",
            Self::Conflicted => "Conflict",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Change {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub kind: ChangeKind,
    pub staged: bool,
    pub unstaged: bool,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub root: PathBuf,
    pub changes: Vec<Change>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DiffLayer {
    Staged,
    #[default]
    Working,
}

impl DiffLayer {
    pub fn label(self) -> &'static str {
        match self {
            Self::Staged => "Staged",
            Self::Working => "Working",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Diff {
    pub text: String,
    pub truncated: bool,
}

pub struct GitReview;

pub enum ReviewTask {
    Snapshot(Result<Snapshot>),
    Diff {
        path: PathBuf,
        layer: DiffLayer,
        result: Result<Diff>,
    },
}

enum ReviewRequest {
    Snapshot(PathBuf),
    Diff(PathBuf, PathBuf, DiffLayer),
}

/// Owns Git process startup and bounded output reads away from the render thread.
pub struct GitReviewWorker {
    requests: mpsc::SyncSender<ReviewRequest>,
    results: mpsc::Receiver<ReviewTask>,
}

impl Default for GitReviewWorker {
    fn default() -> Self {
        let (requests, incoming) = mpsc::sync_channel::<ReviewRequest>(8);
        let (outgoing, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(request) = incoming.recv() {
                let task = match request {
                    ReviewRequest::Snapshot(directory) => {
                        ReviewTask::Snapshot(GitReview::snapshot(&directory))
                    }
                    ReviewRequest::Diff(root, path, layer) => ReviewTask::Diff {
                        result: GitReview::diff(&root, &path, layer),
                        path,
                        layer,
                    },
                };
                if outgoing.send(task).is_err() {
                    break;
                }
            }
        });
        Self { requests, results }
    }
}

impl GitReviewWorker {
    pub fn refresh(&self, directory: PathBuf) -> Result<()> {
        self.requests
            .try_send(ReviewRequest::Snapshot(directory))
            .map_err(|error| anyhow::anyhow!("Cannot queue Git status request: {error}"))
    }
    pub fn diff(&self, root: PathBuf, path: PathBuf, layer: DiffLayer) -> Result<()> {
        self.requests
            .try_send(ReviewRequest::Diff(root, path, layer))
            .map_err(|error| anyhow::anyhow!("Cannot queue Git diff request: {error}"))
    }
    pub fn poll(&self) -> Vec<ReviewTask> {
        self.results.try_iter().collect()
    }
}

impl GitReview {
    /// Read status from the repository containing `directory`.
    pub fn snapshot(directory: &Path) -> Result<Snapshot> {
        let root = git_output(directory, &["rev-parse", "--show-toplevel"], 64 * 1024)?;
        let root = PathBuf::from(
            String::from_utf8(root)
                .context("Git returned a non-UTF-8 repository path")?
                .trim_end_matches(['\r', '\n']),
        );
        ensure!(
            root.is_absolute(),
            "Git returned a non-absolute worktree path"
        );
        let output = git_output(
            &root,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--renames",
            ],
            MAX_STATUS_BYTES,
        )?;
        let changes = parse_status(&output)?;
        Ok(Snapshot { root, changes })
    }

    /// Return a single file's staged or working-tree diff. No pager, textconv,
    /// external diff program, hooks, shell, or repository mutation is used.
    pub fn diff(root: &Path, path: &Path, layer: DiffLayer) -> Result<Diff> {
        let relative = safe_relative_path(path)?;
        if layer == DiffLayer::Working && is_untracked(root, &relative)? {
            return untracked_diff(root, &relative);
        }
        let path_arg = relative
            .to_str()
            .context("Git review currently requires a UTF-8 file name")?;
        let mut args = vec![
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--unified=3",
        ];
        if layer == DiffLayer::Staged {
            args.push("--cached");
        }
        args.extend(["--", path_arg]);
        let bytes = git_output(root, &args, MAX_DIFF_BYTES)?;
        Ok(Diff {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            truncated: false,
        })
    }
}

fn safe_relative_path(path: &Path) -> Result<PathBuf> {
    ensure!(
        !path.as_os_str().is_empty(),
        "A changed file path is required"
    );
    ensure!(
        !path.is_absolute(),
        "Git review paths must be relative to the repository"
    );
    ensure!(
        path.components()
            .all(|part| matches!(part, Component::Normal(_))),
        "Git review path escapes the repository"
    );
    ensure!(
        path.to_str().is_some(),
        "Git review currently requires a UTF-8 file name"
    );
    Ok(path.to_path_buf())
}

fn is_untracked(root: &Path, relative: &Path) -> Result<bool> {
    let path = relative.to_str().context("Non-UTF-8 Git path")?;
    let output = git_output(
        root,
        &["ls-files", "--others", "--exclude-standard", "--", path],
        64 * 1024,
    )?;
    Ok(!output.is_empty())
}

fn untracked_diff(root: &Path, relative: &Path) -> Result<Diff> {
    let target = root.join(relative);
    let metadata = std::fs::symlink_metadata(&target).context("Cannot inspect untracked file")?;
    ensure!(
        metadata.file_type().is_file(),
        "Untracked symlinks and special files are not previewed"
    );
    ensure!(
        metadata.len() as usize <= MAX_UNTRACKED_BYTES,
        "Untracked file exceeds the 1 MiB review limit"
    );
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(&target)
        .context("Cannot safely open untracked file")?;
    let opened_metadata = file.metadata()?;
    ensure!(
        opened_metadata.is_file(),
        "Untracked entry is not a regular file"
    );
    ensure!(
        opened_metadata.len() as usize <= MAX_UNTRACKED_BYTES,
        "Untracked file exceeds the 1 MiB review limit"
    );
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take((MAX_UNTRACKED_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_UNTRACKED_BYTES,
        "Untracked file exceeds the 1 MiB review limit"
    );
    let text = String::from_utf8(bytes).context("Untracked file is not UTF-8 text")?;
    ensure!(
        !text.as_bytes().contains(&0),
        "Untracked file appears to be binary"
    );
    let name = relative.to_string_lossy();
    let name = if name
        .chars()
        .any(|character| character.is_control() || matches!(character, '\\' | '"'))
    {
        format!("{name:?}")
    } else {
        name.into_owned()
    };
    let line_count = text.lines().count();
    let mut diff = format!("--- /dev/null\n+++ b/{name}\n@@ -0,0 +1,{line_count} @@\n");
    for line in text.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    if !text.is_empty() && !text.ends_with('\n') {
        diff.push_str("\\ No newline at end of file\n");
    }
    Ok(Diff {
        text: diff,
        truncated: false,
    })
}

fn parse_status(bytes: &[u8]) -> Result<Vec<Change>> {
    let records: Vec<&[u8]> = bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect();
    let mut changes = Vec::with_capacity(records.len().min(MAX_CHANGES));
    let mut index = 0;
    while index < records.len() {
        ensure!(
            changes.len() < MAX_CHANGES,
            "Repository has more than 10,000 changed files"
        );
        let record = records[index];
        ensure!(
            record.len() >= 4 && record[2] == b' ',
            "Git returned malformed status data"
        );
        let x = record[0];
        let y = record[1];
        let path =
            PathBuf::from(std::str::from_utf8(&record[3..]).context("Changed path is not UTF-8")?);
        let renamed = x == b'R' || y == b'R' || x == b'C' || y == b'C';
        let old_path = if renamed {
            index += 1;
            Some(PathBuf::from(
                std::str::from_utf8(
                    records
                        .get(index)
                        .context("Git omitted rename source path")?,
                )
                .context("Rename source path is not UTF-8")?,
            ))
        } else {
            None
        };
        let untracked = x == b'?' && y == b'?';
        let conflicted = x == b'U' || y == b'U' || matches!((x, y), (b'D', b'D') | (b'A', b'A'));
        let kind = if conflicted {
            ChangeKind::Conflicted
        } else if untracked {
            ChangeKind::Untracked
        } else if renamed {
            ChangeKind::Renamed
        } else if x == b'D' || y == b'D' {
            ChangeKind::Deleted
        } else if x == b'A' || y == b'A' {
            ChangeKind::Added
        } else {
            ChangeKind::Modified
        };
        changes.push(Change {
            path,
            old_path,
            kind,
            staged: !untracked && x != b' ',
            unstaged: !untracked && y != b' ',
        });
        index += 1;
    }
    Ok(changes)
}

fn git_output(root: &Path, args: &[&str], limit: usize) -> Result<Vec<u8>> {
    let mut child = Command::new("git")
        .args([
            "--no-pager",
            "--literal-pathspecs",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-C",
        ])
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .context("Cannot start Git; install Git to use code review")?;
    let mut output = Vec::with_capacity(limit.min(64 * 1024));
    child
        .stdout
        .take()
        .context("Git output was unavailable")?
        .take((limit + 1) as u64)
        .read_to_end(&mut output)?;
    if output.len() > limit {
        let _ = child.kill();
        let _ = child.wait();
        bail!(
            "Git review output exceeded the {} MiB safety limit",
            limit / (1024 * 1024)
        );
    }
    let status = child.wait()?;
    ensure!(
        status.success(),
        "Git {} failed",
        args.first().copied().unwrap_or("command")
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_parser_preserves_rename_source_and_each_change_layer() {
        let parsed =
            parse_status(b"M  staged\0 M unstaged\0R  new-name\0old-name\0?? new-file\0").unwrap();
        assert_eq!(parsed.len(), 4);
        assert!(parsed[0].staged && !parsed[0].unstaged);
        assert!(!parsed[1].staged && parsed[1].unstaged);
        assert_eq!(parsed[2].old_path.as_deref(), Some(Path::new("old-name")));
        assert_eq!(parsed[3].kind, ChangeKind::Untracked);
    }

    #[test]
    fn porcelain_parser_recognizes_all_unmerged_conflict_shapes() {
        for status in [
            b"DD conflict\0".as_slice(),
            b"AU conflict\0",
            b"UD conflict\0",
            b"UA conflict\0",
            b"DU conflict\0",
            b"AA conflict\0",
            b"UU conflict\0",
        ] {
            assert_eq!(
                parse_status(status).unwrap()[0].kind,
                ChangeKind::Conflicted
            );
        }
        assert_eq!(
            parse_status(b"A  added\0").unwrap()[0].kind,
            ChangeKind::Added
        );
    }
}
