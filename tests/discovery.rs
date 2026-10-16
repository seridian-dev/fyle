use fyle::core::discovery::{fuzzy_score, Query};
use fyle::core::preview::{read_preview, Preview};
use fyle::types::FileEntry;

fn entry(name: &str, size: u64, is_dir: bool) -> FileEntry {
    FileEntry {
        name: name.into(),
        path: name.into(),
        size,
        is_dir,
        modified: chrono::Local::now(),
        is_hidden: false,
        file_type: String::new(),
        selected: false,
    }
}
#[test]
fn lenses_compose_and_negate_without_touching_the_filesystem() {
    let query = Query::parse("ext:rs size:>10kb -test kind:file").unwrap();
    assert!(query.matches(&entry("engine.rs", 20_000, false)));
    assert!(!query.matches(&entry("engine_test.rs", 20_000, false)));
    assert!(!query.matches(&entry("engine.rs", 100, false)));
    assert!(!query.matches(&entry("engine.txt", 20_000, false)));
    assert!(!query.matches(&entry("engine.rs", 20_000, true)));
    assert!(Query::parse("kind:folder")
        .unwrap()
        .matches(&entry("src", 0, true)));
    assert!(Query::parse("日本")
        .unwrap()
        .matches(&entry("日本語.txt", 0, false)));
    assert!(Query::parse("\"release notes\"").unwrap().matches(&entry(
        "Release Notes.md",
        50,
        false
    )));
    assert!(Query::parse("size:>999999999999999999999gb").is_err());
    assert!(Query::parse("size:banana").is_err());
    assert!(Query::parse("ext:rs,py")
        .unwrap()
        .matches(&entry("script.PY", 50, false)));
    assert!(Query::parse("ext:.").is_err());
    assert!(Query::parse("kind:dragon").is_err());
    assert!(Query::parse("\"unfinished").is_err());
    // Unknown colon words remain literal filenames.
    assert!(Query::parse("draft:final")
        .unwrap()
        .matches(&entry("draft:final.txt", 0, false)));
}
#[test]
fn fuzzy_jump_ranks_exact_then_prefix_then_subsequence_and_handles_unicode() {
    assert!(fuzzy_score("readme", "README") > fuzzy_score("readme", "README.md"));
    assert!(fuzzy_score("read", "readme.md") > fuzzy_score("read", "my_readme.md"));
    assert!(fuzzy_score("rdm", "README.md").is_some());
    assert!(fuzzy_score("xyz", "README.md").is_none());
    assert!(fuzzy_score("日本", "日本語.txt").is_some());
}
#[test]
fn previews_are_bounded_utf8_safe_and_never_render_binary_or_follow_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let text = dir.path().join("notes.txt");
    std::fs::write(&text, "日本語\n".repeat(20_000)).unwrap();
    let Preview::Text {
        text: content,
        truncated,
    } = read_preview(&text).unwrap()
    else {
        panic!("expected text")
    };
    assert!(truncated);
    assert!(content.len() <= 32 * 1024);
    assert!(content.contains("日本語"));
    let binary = dir.path().join("binary");
    std::fs::write(&binary, b"hello\0world").unwrap();
    assert!(matches!(
        read_preview(&binary).unwrap(),
        Preview::Unsupported
    ));
    assert!(read_preview(dir.path()).is_err());
    std::fs::write(&binary, [0xff, 0xfe, 0xfd]).unwrap();
    assert!(matches!(
        read_preview(&binary).unwrap(),
        Preview::Unsupported
    ));
    // Empty files are valid, and a prefix ending inside a UTF-8 character is
    // shortened to its last complete character rather than called binary.
    std::fs::write(&binary, []).unwrap();
    assert!(
        matches!(read_preview(&binary).unwrap(), Preview::Text { text, truncated: false } if text.is_empty())
    );
    #[cfg(unix)]
    {
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&text, &link).unwrap();
        assert!(read_preview(&link).is_err());
    }
}
