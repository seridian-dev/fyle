use fyle::core::{create_zip, extract_archive};
use std::fs;
use std::io::Write;

#[test]
fn zip_creation_and_extraction_round_trip_files_and_directories() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("mission files");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("readme.txt"), "hello from Fyle").unwrap();
    fs::write(source.join("nested/data.bin"), [0, 1, 2, 255]).unwrap();
    let archive = temp.path().join("mission.zip");

    create_zip(std::slice::from_ref(&source), &archive).unwrap();
    let extracted = extract_archive(&archive, temp.path()).unwrap();

    assert!(extracted.ends_with("mission-extracted"));
    assert_eq!(
        fs::read_to_string(extracted.join("mission files/readme.txt")).unwrap(),
        "hello from Fyle"
    );
    assert_eq!(
        fs::read(extracted.join("mission files/nested/data.bin")).unwrap(),
        [0, 1, 2, 255]
    );
}

#[test]
fn extraction_rejects_zip_slip_and_does_not_publish_partial_output() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("escape.zip");
    let file = fs::File::create(&archive).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    writer
        .start_file("../outside.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"escape").unwrap();
    writer.finish().unwrap();

    assert!(extract_archive(&archive, temp.path()).is_err());
    assert!(!temp.path().join("outside.txt").exists());
    assert!(!temp.path().join("escape").exists());
}

#[test]
fn extraction_preserves_existing_destination_without_overwriting() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("package.zip");
    let file = fs::File::create(&archive).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    writer
        .start_file("new.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"archive data").unwrap();
    writer.finish().unwrap();
    let existing = temp.path().join("package-extracted");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("keep.txt"), "keep me").unwrap();

    assert!(extract_archive(&archive, temp.path()).is_err());
    assert_eq!(
        fs::read_to_string(existing.join("keep.txt")).unwrap(),
        "keep me"
    );
    assert!(!existing.join("new.txt").exists());
}

#[test]
fn malformed_archive_fails_without_creating_output() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("broken.zip");
    fs::write(&archive, b"not a zip file").unwrap();

    assert!(extract_archive(&archive, temp.path()).is_err());
    assert!(!temp.path().join("broken").exists());
}

#[test]
fn tar_gzip_archive_extracts_into_a_new_folder() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("logs.tgz");
    let output = fs::File::create(&archive).unwrap();
    let encoder = flate2::write::GzEncoder::new(output, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let contents = b"compressed log data";
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, "logs/app.txt", &contents[..])
        .unwrap();
    builder.into_inner().unwrap().finish().unwrap();

    let extracted = extract_archive(&archive, temp.path()).unwrap();

    assert_eq!(fs::read(extracted.join("logs/app.txt")).unwrap(), contents);
}

#[test]
fn extraction_rejects_zip_symlinks() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("links.zip");
    let file = fs::File::create(&archive).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    writer
        .add_symlink(
            "link",
            "../../outside",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer.finish().unwrap();

    assert!(extract_archive(&archive, temp.path()).is_err());
    assert!(!temp.path().join("outside").exists());
    assert!(!temp.path().join("links-extracted").exists());
}

#[test]
fn extraction_rejects_tar_path_traversal() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("escape.tar");
    let file = fs::File::create(&archive).unwrap();
    let mut builder = tar::Builder::new(file);
    let contents = b"escape";
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o644);
    header.as_mut_bytes()[..14].copy_from_slice(b"../outside.txt");
    header.set_cksum();
    builder.append(&header, &contents[..]).unwrap();
    builder.finish().unwrap();

    assert!(extract_archive(&archive, temp.path()).is_err());
    assert!(!temp.path().join("outside.txt").exists());
    assert!(!temp.path().join("escape-extracted").exists());
}

#[test]
fn zip_creation_never_overwrites_an_existing_archive() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("notes.txt");
    let archive = temp.path().join("notes.zip");
    fs::write(&source, "source data").unwrap();
    fs::write(&archive, "preserve this existing file").unwrap();

    assert!(create_zip(std::slice::from_ref(&source), &archive).is_err());
    assert_eq!(
        fs::read_to_string(archive).unwrap(),
        "preserve this existing file"
    );
}
