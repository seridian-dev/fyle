//! Drag-and-drop interop contract. Fake desktop payloads (what other
//! applications actually place on the drag clipboard) must parse into paths
//! and route through the same safe-operation path as in-app copy/move.

use fyle::core::job::{FileOperation, OperationJob};
use fyle::desktop::dnd::{self, DragEffect};
use std::path::PathBuf;
use std::time::Duration;

/// Drive an operation job to completion without a GUI event loop.
fn outcome(job: OperationJob) -> anyhow::Result<()> {
    loop {
        if let Some(result) = job.poll() {
            return result;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn uri_list_drops_parse_percent_encoded_paths_and_ignore_non_files() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("space #dir");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("report one.txt");
    std::fs::write(&file, "body").unwrap();

    let payload = format!(
        "# dragged from another application\r\n\
         https://mail.seridian.dev/compose\r\n\
         mailto:team@seridian.dev\r\n\
         {}\r\n\
         file://remote-host/share/secret.txt\r\n\
         \r\n",
        url::Url::from_file_path(&file).unwrap()
    );
    assert_eq!(dnd::parse_uri_list(&payload), vec![file]);
}

#[test]
fn cf_hdrop_payloads_decode_wide_paths_and_ignore_malformed_blobs() {
    let mut payload = vec![0u8; 20];
    payload[0..4].copy_from_slice(&20u32.to_le_bytes());
    payload[16..20].copy_from_slice(&1i32.to_le_bytes());
    for path in [r"C:\Users\Dee\notes.txt", r"D:\報告"] {
        for unit in path.encode_utf16() {
            payload.extend_from_slice(&unit.to_le_bytes());
        }
        payload.extend_from_slice(&0u16.to_le_bytes());
    }
    payload.extend_from_slice(&0u16.to_le_bytes());

    assert_eq!(
        dnd::parse_cf_hdrop(&payload),
        vec![
            PathBuf::from(r"C:\Users\Dee\notes.txt"),
            PathBuf::from(r"D:\報告"),
        ]
    );
    assert!(dnd::parse_cf_hdrop(&[]).is_empty());
    assert!(dnd::parse_cf_hdrop(&payload[..12]).is_empty());
    assert!(dnd::parse_cf_hdrop(&[0u8; 32]).is_empty());
    // A blob cut mid-item yields the complete leading entries, not garbage.
    let cut = payload.len() - 6;
    assert_eq!(
        dnd::parse_cf_hdrop(&payload[..cut]),
        vec![PathBuf::from(r"C:\Users\Dee\notes.txt")]
    );
}

#[test]
fn outgoing_drag_payload_carries_every_selected_item_and_round_trips() {
    let temp = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = ["one.txt", "two bytes.bin", "日本語.md"]
        .iter()
        .map(|name| {
            let path = temp.path().join(name);
            std::fs::write(&path, name).unwrap();
            path
        })
        .collect();

    let payload = dnd::build_drag_payload(&paths).unwrap();
    assert_eq!(payload.count, 3);
    assert_eq!(dnd::parse_uri_list(&payload.uri_list), paths);
    assert_eq!(dnd::parse_cf_hdrop(&payload.cf_hdrop), paths);

    assert!(dnd::build_drag_payload(&[]).is_none());
    assert!(dnd::build_drag_payload(&[PathBuf::from("relative.txt")]).is_none());
}

#[test]
fn incoming_drop_flows_through_confirm_copy_and_operation_job() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("attachment.pdf");
    std::fs::write(&source, "invoice").unwrap();
    let uri_list = format!("{}\r\n", url::Url::from_file_path(&source).unwrap());

    let paths = dnd::parse_uri_list(&uri_list);
    let destination = temp.path().join("inbox");
    std::fs::create_dir(&destination).unwrap();
    let intent = dnd::incoming_intent(paths, &destination, dnd::drag_effect(false, false))
        .expect("copy intent for external drop");
    assert_eq!(intent.effect, DragEffect::Copy);

    // The ConfirmCopy dialog turns the same intent into this operation.
    let job = OperationJob::start(FileOperation::Copy(intent.paths, destination.clone()));
    outcome(job).unwrap();
    assert_eq!(
        std::fs::read_to_string(destination.join("attachment.pdf")).unwrap(),
        "invoice"
    );
}

#[test]
fn drop_copy_surfaces_existing_destination_collision() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("notes.txt");
    std::fs::write(&source, "new bytes").unwrap();
    let destination = temp.path().join("here");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("notes.txt"), "old bytes").unwrap();

    let intent =
        dnd::incoming_intent(vec![source.clone()], &destination, DragEffect::Copy).unwrap();
    let job = OperationJob::start(FileOperation::Copy(intent.paths, destination.clone()));
    let error = outcome(job).unwrap_err();
    assert!(format!("{error:#}").contains("already exists"));
    assert_eq!(
        std::fs::read_to_string(destination.join("notes.txt")).unwrap(),
        "old bytes"
    );
}

#[test]
fn move_modifier_drop_moves_instead_of_copies() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("moved.txt");
    std::fs::write(&source, "carry me").unwrap();
    let destination = temp.path().join("elsewhere");
    std::fs::create_dir(&destination).unwrap();

    let intent = dnd::incoming_intent(
        vec![source.clone()],
        &destination,
        dnd::drag_effect(true, false),
    )
    .unwrap();
    assert_eq!(intent.effect, DragEffect::Move);
    let job = OperationJob::start(FileOperation::Move(intent.paths, destination.clone()));
    outcome(job).unwrap();
    assert!(!source.exists());
    assert_eq!(
        std::fs::read_to_string(destination.join("moved.txt")).unwrap(),
        "carry me"
    );
}
