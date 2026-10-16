use fyle::core::git_review::{ChangeKind, DiffLayer, GitReview};
use std::{fs, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "-q"]);
    git(root.path(), &["config", "user.name", "Fyle Test"]);
    git(
        root.path(),
        &["config", "user.email", "fyle@example.invalid"],
    );
    fs::write(root.path().join("base.rs"), "fn base() {}\n").unwrap();
    fs::write(root.path().join("delete-me.rs"), "fn delete_me() {}\n").unwrap();
    fs::write(root.path().join("rename-me.rs"), "fn moved() {}\n").unwrap();
    git(root.path(), &["add", "base.rs"]);
    git(root.path(), &["add", "delete-me.rs"]);
    git(root.path(), &["add", "rename-me.rs"]);
    git(root.path(), &["commit", "-qm", "base"]);
    root
}

#[test]
fn lists_staged_unstaged_untracked_unicode_rename_and_delete_changes() {
    let root = repository();
    fs::write(root.path().join("base.rs"), "fn base() { true }\n").unwrap();
    git(root.path(), &["add", "base.rs"]);
    fs::write(root.path().join("base.rs"), "fn base() { false }\n").unwrap();
    fs::write(root.path().join("新規.rs"), "fn new_file() {}\n").unwrap();
    git(root.path(), &["mv", "rename-me.rs", "renamed.rs"]);
    git(root.path(), &["rm", "delete-me.rs"]);

    let snapshot = GitReview::snapshot(root.path()).unwrap();
    let find = |name: &str| {
        snapshot
            .changes
            .iter()
            .find(|change| change.path == Path::new(name))
            .unwrap()
    };
    assert!(find("base.rs").staged);
    assert!(find("base.rs").unstaged);
    assert_eq!(find("delete-me.rs").kind, ChangeKind::Deleted);
    assert_eq!(find("新規.rs").kind, ChangeKind::Untracked);
    assert_eq!(find("renamed.rs").kind, ChangeKind::Renamed);
    assert_eq!(
        find("renamed.rs").old_path.as_deref(),
        Some(Path::new("rename-me.rs"))
    );
    assert_eq!(
        GitReview::diff(root.path(), Path::new("新規.rs"), DiffLayer::Working)
            .unwrap()
            .text
            .lines()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .count(),
        1
    );
}

#[test]
fn renders_bounded_read_only_staged_and_working_diffs() {
    let root = repository();
    fs::write(root.path().join("base.rs"), "fn staged() {}\n").unwrap();
    git(root.path(), &["add", "base.rs"]);
    fs::write(root.path().join("base.rs"), "fn working() {}\n").unwrap();
    let staged = GitReview::diff(root.path(), Path::new("base.rs"), DiffLayer::Staged).unwrap();
    let working = GitReview::diff(root.path(), Path::new("base.rs"), DiffLayer::Working).unwrap();
    assert!(staged.text.contains("+fn staged() {}"));
    assert!(working.text.contains("+fn working() {}"));
    assert_eq!(
        fs::read_to_string(root.path().join("base.rs")).unwrap(),
        "fn working() {}\n"
    );
}

#[test]
fn non_repository_is_reported_and_paths_cannot_escape_the_worktree() {
    let outside = tempfile::tempdir().unwrap();
    assert!(GitReview::snapshot(outside.path()).is_err());
    let root = repository();
    assert!(GitReview::diff(root.path(), Path::new("../outside"), DiffLayer::Working).is_err());
}

#[test]
fn refuses_binary_and_symlinked_untracked_files() {
    let root = repository();
    fs::write(root.path().join("binary.bin"), [0, 1, 2]).unwrap();
    assert!(GitReview::diff(root.path(), Path::new("binary.bin"), DiffLayer::Working).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.path().join("base.rs"), root.path().join("linked.rs"))
            .unwrap();
        assert!(GitReview::diff(root.path(), Path::new("linked.rs"), DiffLayer::Working).is_err());
    }
}
