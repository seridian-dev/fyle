use fyle::core::{copy_files, create_directory, create_file, move_files, rename_file};
use std::fs;

#[test]
fn creating_a_file_never_truncates_an_existing_file() {
    let temp = tempfile::tempdir().unwrap();
    let existing = temp.path().join("notes");
    fs::write(&existing, "valuable").unwrap();
    assert!(create_file(temp.path(), "notes").is_err());
    assert_eq!(fs::read_to_string(existing).unwrap(), "valuable");
}

#[test]
fn create_and_rename_accept_only_single_normal_names() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("folder");
    fs::create_dir(&folder).unwrap();
    let original = folder.join("original");
    fs::write(&original, "valuable").unwrap();
    for name in ["", ".", "..", "../escape", "nested/file", "nested\\file"] {
        assert!(create_file(&folder, name).is_err(), "{name}");
        assert!(create_directory(&folder, name).is_err(), "{name}");
        assert!(rename_file(&original, name).is_err(), "{name}");
    }
    assert!(original.exists());
    assert!(!temp.path().join("escape").exists());
}

#[test]
fn copy_move_and_rename_preserve_existing_destinations() {
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("dest");
    fs::create_dir(&dest).unwrap();
    let source = temp.path().join("notes");
    let existing = dest.join("notes");
    fs::write(&source, "source").unwrap();
    fs::write(&existing, "valuable").unwrap();
    assert!(copy_files(std::slice::from_ref(&source), &dest).is_err());
    assert!(move_files(std::slice::from_ref(&source), &dest).is_err());
    let other = temp.path().join("other");
    fs::write(&other, "valuable").unwrap();
    assert!(rename_file(&source, "other").is_err());
    assert_eq!(fs::read_to_string(existing).unwrap(), "valuable");
    assert_eq!(fs::read_to_string(source).unwrap(), "source");
}

#[test]
fn batch_copy_preflights_collisions_before_copying_any_item() {
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("dest");
    fs::create_dir(&dest).unwrap();
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    fs::write(&a, "a").unwrap();
    fs::write(&b, "b").unwrap();
    fs::write(dest.join("b"), "valuable").unwrap();
    assert!(copy_files(&[a, b], &dest).is_err());
    assert!(!dest.join("a").exists());
}

#[test]
fn recursive_copy_into_itself_is_rejected_before_creating_output() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("notes"), "valuable").unwrap();
    assert!(copy_files(std::slice::from_ref(&source), &source).is_err());
    assert!(!source.join("source").exists());
}

#[cfg(unix)]
#[test]
fn copying_symlinks_does_not_follow_or_overwrite_targets() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let dest = temp.path().join("dest");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&dest).unwrap();
    let target = temp.path().join("target");
    fs::write(&target, "valuable").unwrap();
    symlink(&target, source.join("link")).unwrap();
    assert!(copy_files(&[source], &dest).is_err());
    assert!(!dest.join("source").exists());
    assert_eq!(fs::read_to_string(target).unwrap(), "valuable");
}

#[test]
fn recursive_copy_preserves_nested_contents_and_empty_folders() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let dest = temp.path().join("dest");
    fs::create_dir_all(source.join("empty")).unwrap();
    fs::create_dir(&dest).unwrap();
    fs::write(source.join("notes"), "valuable").unwrap();
    copy_files(&[source], &dest).unwrap();
    assert!(dest.join("source/empty").is_dir());
    assert_eq!(
        fs::read_to_string(dest.join("source/notes")).unwrap(),
        "valuable"
    );
}

#[test]
fn duplicate_batch_names_are_rejected_before_mutation() {
    let temp = tempfile::tempdir().unwrap();
    for name in ["a", "b", "dest"] {
        fs::create_dir(temp.path().join(name)).unwrap();
    }
    let a = temp.path().join("a/notes");
    let b = temp.path().join("b/notes");
    fs::write(&a, "a").unwrap();
    fs::write(&b, "b").unwrap();
    assert!(copy_files(&[a, b], &temp.path().join("dest")).is_err());
    assert!(!temp.path().join("dest/notes").exists());
}

#[cfg(unix)]
#[test]
fn permanent_delete_unlinks_a_directory_symlink_without_deleting_its_target() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("notes"), "valuable").unwrap();
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    fyle::core::delete_files(std::slice::from_ref(&link)).unwrap();
    assert!(!link.exists());
    assert_eq!(
        fs::read_to_string(target.join("notes")).unwrap(),
        "valuable"
    );
}
