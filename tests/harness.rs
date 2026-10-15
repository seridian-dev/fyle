use fyle::harness::{self, Reply, Server};
use serde_json::json;

#[test]
fn session_transport_authenticates_and_delivers_actions() {
    let temp = tempfile::tempdir().unwrap();
    let server = Server::start(temp.path().into(), || {}).unwrap();
    let session = server.session.clone();
    let worker = std::thread::spawn(move || {
        let request = server
            .receive_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        assert_eq!(request.command["action"], "state");
        request
            .respond
            .send(Reply::ok(json!({"path":"/fixture"})))
            .unwrap();
    });
    let response = harness::request(&session, json!({"action":"state"})).unwrap();
    assert_eq!(response.data["path"], "/fixture");
    worker.join().unwrap();
}

#[test]
fn unauthenticated_clients_cannot_dispatch_gui_actions() {
    let temp = tempfile::tempdir().unwrap();
    let server = Server::start(temp.path().into(), || {}).unwrap();
    let mut session = server.session.clone();
    session.token = "wrong-token".into();
    assert!(harness::request(&session, json!({"action":"state"})).is_err());
    assert!(server.try_receive().is_none());
}

#[test]
fn png_encoding_preserves_size_and_pixel_data() {
    let png = harness::encode_png(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut bytes).unwrap();
    assert_eq!((info.width, info.height), (2, 1));
    assert_eq!(
        &bytes[..info.buffer_size()],
        &[255, 0, 0, 255, 0, 255, 0, 255]
    );
}

#[test]
fn dropping_a_session_removes_its_private_descriptor() {
    let temp = tempfile::tempdir().unwrap();
    let server = Server::start(temp.path().into(), || {}).unwrap();
    let path = harness::session_path(temp.path(), &server.session.id).unwrap();
    assert!(path.exists());
    drop(server);
    assert!(!path.exists());
    assert!(harness::session_path(temp.path(), "../../escape").is_err());
}

#[test]
fn automation_requires_an_option_before_the_argument_terminator() {
    let args = |values: &[&str]| {
        values
            .iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>()
    };
    assert!(fyle::startup::automation_requested(args(&[
        "--automation",
        "/tmp"
    ])));
    assert!(!fyle::startup::automation_requested(args(&[
        "--",
        "--automation"
    ])));
}
