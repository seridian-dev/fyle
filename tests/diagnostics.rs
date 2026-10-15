use fyle::diagnostics::{write_bundle, DiagnosticsStore, RollingWriter};
use std::io::Write;

#[test]
fn crash_reports_are_structured_and_retained_with_a_limit() {
    let temp = tempfile::tempdir().unwrap();
    let store = DiagnosticsStore::new(temp.path().into());
    for i in 0..25 {
        store
            .record_crash("panic", &format!("failure {i}"), "stack trace")
            .unwrap();
    }
    let reports = store.crashes().unwrap();
    assert_eq!(reports.len(), 20);
    assert_eq!(reports[0]["kind"], "panic");
    assert!(reports[0]["version"].is_string());
    assert!(reports[0]["backtrace"].is_string());
}

#[test]
fn debug_bundle_redacts_home_and_credential_lines() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("private-home");
    let store = DiagnosticsStore::new(temp.path().join("diagnostics"));
    store
        .record_crash(
            "panic",
            &format!("failed at {}/notes", home.display()),
            "password=hunter2",
        )
        .unwrap();
    let encoded = serde_json::to_string(&store.bundle(Some(&home)).unwrap()).unwrap();
    assert!(!encoded.contains(&home.to_string_lossy().to_string()));
    assert!(!encoded.contains("hunter2"));
    assert!(encoded.contains("$HOME"));
    assert!(encoded.contains("doctor"));
}

#[test]
fn logs_rotate_and_bundle_reads_only_bounded_tails() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("gui-session.log");
    let mut writer = RollingWriter::new(path.clone(), 64).unwrap();
    writer.write_all(b"first entry\n").unwrap();
    writer.write_all(&[b'x'; 1000]).unwrap();
    writer.flush().unwrap();
    assert!(path.metadata().unwrap().len() <= 64);
    let store = DiagnosticsStore::new(temp.path().into());
    assert!(!store.logs(10).unwrap().is_empty());
}

#[test]
fn bundle_never_overwrites_an_existing_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("bundle.json");
    std::fs::write(&path, "valuable").unwrap();
    assert!(write_bundle(&path, &serde_json::json!({"test": true})).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "valuable");
}

#[test]
fn malformed_reports_do_not_hide_other_crashes() {
    let temp = tempfile::tempdir().unwrap();
    let store = DiagnosticsStore::new(temp.path().into());
    store
        .record_crash("startup", "renderer failed", "")
        .unwrap();
    std::fs::write(temp.path().join("crash-broken.json"), "not json").unwrap();
    assert_eq!(store.crashes().unwrap().len(), 1);
}

#[test]
#[ignore = "invoked in a child process by panic_capture_works_in_a_real_process"]
fn panic_fixture() {
    fyle::diagnostics::init_gui();
    panic!("diagnostic fixture panic");
}

#[test]
fn panic_capture_works_in_a_real_process() {
    let temp = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "panic_fixture", "--nocapture"])
        .env("FYLE_DIAGNOSTICS_DIR", temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let reports = DiagnosticsStore::new(temp.path().into()).crashes().unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["kind"], "panic");
    assert!(reports[0]["message"]
        .as_str()
        .unwrap()
        .contains("diagnostic fixture panic"));
    assert!(!reports[0]["backtrace"].as_str().unwrap().is_empty());
}

#[test]
fn cli_exports_json_and_refuses_to_overwrite_bundles() {
    let temp = tempfile::tempdir().unwrap();
    let cli = env!("CARGO_BIN_EXE_fyle-cli");
    let output = std::process::Command::new(cli)
        .args(["doctor", "--json"])
        .env("FYLE_DIAGNOSTICS_DIR", temp.path())
        .env("FYLE_TEST_SECRET", "do-not-collect-this")
        .output()
        .unwrap();
    assert!(output.status.success());
    let info: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(info["version"].is_string());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("do-not-collect-this"));
    let bundle = temp.path().join("debug bundle.json");
    let first = std::process::Command::new(cli)
        .args(["debug", "bundle", "--output"])
        .arg(&bundle)
        .env("FYLE_DIAGNOSTICS_DIR", temp.path())
        .output()
        .unwrap();
    assert!(first.status.success());
    let original = std::fs::read(&bundle).unwrap();
    let second = std::process::Command::new(cli)
        .args(["debug", "bundle", "--output"])
        .arg(&bundle)
        .env("FYLE_DIAGNOSTICS_DIR", temp.path())
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert_eq!(std::fs::read(bundle).unwrap(), original);
}

#[test]
fn large_unicode_and_control_messages_stay_readable_and_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let store = DiagnosticsStore::new(temp.path().into());
    let path = store
        .record_crash("panic", &"日本語".repeat(20_000), &"\0".repeat(30_000))
        .unwrap();
    assert!(path.metadata().unwrap().len() < 256 * 1024);
    assert_eq!(store.crashes().unwrap().len(), 1);
}

#[test]
fn newest_logs_win_over_lexicographic_file_names() {
    let temp = tempfile::tempdir().unwrap();
    for index in 0..6 {
        let path = temp.path().join(format!("z-old-{index}.log"));
        std::fs::write(&path, "old").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(100)),
            )
            .unwrap();
    }
    std::fs::write(temp.path().join("a-new.log"), "latest").unwrap();
    let logs = DiagnosticsStore::new(temp.path().into()).logs(10).unwrap();
    assert_eq!(logs[0].text, "latest");
}

#[test]
fn latest_crash_json_is_null_when_no_reports_exist() {
    let temp = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_fyle-cli"))
        .args(["crashes", "--latest", "--json"])
        .env("FYLE_DIAGNOSTICS_DIR", temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::Value::Null
    );
}
