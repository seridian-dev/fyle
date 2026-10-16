use fyle::core::editor::{Document, MAX_DOCUMENT_BYTES};
use std::{fs, path::Path};
use tempfile::tempdir;

#[test]
fn loads_utf8_text_tracks_dirty_state_and_saves_edits() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("sample.rs");
    fs::write(&path, "fn main() {}\n").unwrap();

    let mut document = Document::load(&path).unwrap();
    assert_eq!(document.text(), "fn main() {}\n");
    assert!(!document.is_dirty());
    document.set_text("fn main() { println!(\"fyle\"); }\n".into());
    assert!(document.is_dirty());

    document.save().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), document.text());
    assert!(!document.is_dirty());
}

#[test]
fn rejects_oversized_binary_symlink_and_non_regular_inputs() {
    let dir = tempdir().unwrap();
    let binary = dir.path().join("image.png");
    fs::write(&binary, [0, 1, 2]).unwrap();
    assert!(Document::load(&binary).is_err());

    let huge = dir.path().join("huge.txt");
    let file = fs::File::create(&huge).unwrap();
    file.set_len((MAX_DOCUMENT_BYTES + 1) as u64).unwrap();
    assert!(Document::load(&huge).is_err());

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let link = dir.path().join("link.rs");
        symlink(Path::new("sample.rs"), &link).unwrap();
        assert!(Document::load(&link).is_err());
    }
}

#[test]
fn refuses_to_overwrite_changes_made_by_another_application() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("shared.txt");
    fs::write(&path, "original\n").unwrap();
    let mut document = Document::load(&path).unwrap();
    document.set_text("my edit\n".into());
    fs::write(&path, "external edit\n").unwrap();

    let error = document.save().unwrap_err().to_string();
    assert!(error.contains("changed on disk"));
    assert_eq!(fs::read_to_string(path).unwrap(), "external edit\n");
}

#[test]
fn routes_source_and_plain_text_extensions_to_the_native_editor() {
    for name in ["main.rs", "theme.css", "notes.md", "Makefile", ".gitignore"] {
        assert!(
            fyle::core::editor::is_text_candidate(Path::new(name)),
            "{name}"
        );
    }
    for name in ["photo.png", "archive.zip", "movie.mp4"] {
        assert!(
            !fyle::core::editor::is_text_candidate(Path::new(name)),
            "{name}"
        );
    }
}

#[test]
fn editor_worker_loads_and_saves_without_blocking_the_requesting_thread() {
    use fyle::core::editor::{EditorTask, EditorWorker};
    use std::time::{Duration, Instant};

    let dir = tempdir().unwrap();
    let path = dir.path().join("worker.rs");
    fs::write(&path, "fn main() {}\n").unwrap();
    let worker = EditorWorker::default();
    worker.load(path.clone()).unwrap();

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut document = loop {
        if let Some(EditorTask::Loaded { result, .. }) = worker.poll().into_iter().next() {
            break result.unwrap();
        }
        assert!(Instant::now() < deadline, "document load did not finish");
        std::thread::sleep(Duration::from_millis(5));
    };
    document.set_text("fn main() { /* saved in worker */ }\n".into());
    worker.save(document).unwrap();
    loop {
        if let Some(EditorTask::Saved { result, .. }) = worker.poll().into_iter().next() {
            assert!(!result.unwrap().is_dirty());
            break;
        }
        assert!(Instant::now() < deadline, "document save did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(fs::read_to_string(path)
        .unwrap()
        .contains("saved in worker"));
}
