use fyle::{desktop::select_terminal, startup::resolve_location};

#[test]
fn desktop_policy_is_independent_of_init_system() {
    for (desktop, expected) in [
        ("KDE", "konsole"),
        ("plasma", "konsole"),
        ("ubuntu:GNOME", "gnome-terminal"),
        ("Hyprland", "foot"),
        ("sway", "foot"),
        ("XFCE", "xfce4-terminal"),
    ] {
        assert_eq!(select_terminal(desktop, |_| true), Some(expected));
        assert_eq!(
            select_terminal(desktop, |app| app == "kitty"),
            Some("kitty")
        );
    }
    assert_eq!(select_terminal("unknown", |_| false), None);
}

#[test]
fn file_urls_decode_spaces_and_unicode_and_files_open_parent() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("日本語 space #");
    std::fs::create_dir(&dir).unwrap();
    let file = dir.join("document.txt");
    std::fs::write(&file, "test").unwrap();
    let url = url::Url::from_file_path(&file).unwrap();
    assert_eq!(
        resolve_location(url.as_str().as_ref()).unwrap(),
        dir.canonicalize().unwrap()
    );
    assert_eq!(
        resolve_location(dir.as_os_str()).unwrap(),
        dir.canonicalize().unwrap()
    );
    assert!(resolve_location("https://example.com".as_ref()).is_err());
    assert!(resolve_location("file://remote-host/path".as_ref()).is_err());
}

#[test]
fn terminal_launch_preserves_parent_cwd_and_has_no_foreign_flags() {
    let before = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let command = fyle::desktop::terminal_command("konsole", dir.path());
    assert_eq!(command.get_current_dir(), Some(dir.path()));
    assert_eq!(command.get_program(), "konsole");
    #[cfg(not(target_os = "macos"))]
    assert_eq!(command.get_args().count(), 0);
    assert_eq!(std::env::current_dir().unwrap(), before);
}
