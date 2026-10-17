use fyle::{
    desktop::{
        detect_desktop, detect_display_backend, detect_init_system, detect_session,
        select_terminal, select_terminal_for_session, DesktopEnvironment, DisplayBackend,
    },
    startup::{resolve_location, resolve_target},
};

#[test]
fn desktop_policy_is_independent_of_init_system() {
    for (desktop, expected) in [
        ("KDE", "konsole"),
        ("plasma", "konsole"),
        ("ubuntu:GNOME", "kgx"),
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
fn desktop_and_session_detection_handles_common_wayland_and_x11_sessions() {
    let cases = [
        (
            "KDE",
            "",
            "wayland",
            DesktopEnvironment::Plasma,
            DisplayBackend::Wayland,
        ),
        (
            "ubuntu:GNOME",
            "",
            "wayland",
            DesktopEnvironment::Gnome,
            DisplayBackend::Wayland,
        ),
        (
            "",
            "plasma",
            "x11",
            DesktopEnvironment::Plasma,
            DisplayBackend::X11,
        ),
        (
            "",
            "",
            "",
            DesktopEnvironment::Hyprland,
            DisplayBackend::Wayland,
        ),
        ("", "", "", DesktopEnvironment::Sway, DisplayBackend::X11),
        (
            "XFCE",
            "",
            "",
            DesktopEnvironment::Xfce,
            DisplayBackend::Unknown,
        ),
        (
            "COSMIC",
            "",
            "",
            DesktopEnvironment::Cosmic,
            DisplayBackend::Unknown,
        ),
    ];
    for (desktop, session_desktop, session_type, expected_desktop, expected_display) in cases {
        let session = detect_session(
            desktop,
            session_desktop,
            session_type,
            desktop.is_empty() && expected_desktop == DesktopEnvironment::Hyprland,
            desktop.is_empty() && expected_desktop == DesktopEnvironment::Sway,
            expected_display == DisplayBackend::Wayland,
            expected_display == DisplayBackend::X11,
        );
        assert_eq!(session.environment, expected_desktop);
        assert_eq!(session.display, expected_display);
    }

    assert_eq!(
        detect_display_backend("wayland", false, true),
        DisplayBackend::Wayland
    );
    assert_eq!(
        detect_desktop("KDE:GNOME", "", false, false),
        DesktopEnvironment::Plasma
    );
}

#[test]
fn terminal_preferences_follow_desktop_even_with_generic_xdg_sessions() {
    let plasma = detect_session("", "plasma", "wayland", false, false, true, false);
    assert_eq!(
        select_terminal_for_session(plasma, |app| app == "konsole"),
        Some("konsole")
    );

    let gnome = detect_session("ubuntu:GNOME", "", "wayland", false, false, true, false);
    assert_eq!(
        select_terminal_for_session(gnome, |app| app == "kgx"),
        Some("kgx")
    );

    let hyprland = detect_session("", "", "wayland", true, false, true, false);
    assert_eq!(
        select_terminal_for_session(hyprland, |app| app == "foot"),
        Some("foot")
    );
    assert_eq!(
        select_terminal_for_session(hyprland, |app| app == "kitty"),
        Some("kitty")
    );
}

#[test]
fn init_detection_distinguishes_openrc_systemd_and_other_without_affecting_desktop() {
    assert_eq!(detect_init_system(true, "systemd"), "openrc");
    assert_eq!(detect_init_system(false, "systemd"), "systemd");
    assert_eq!(detect_init_system(false, "runit"), "other");
    assert_eq!(select_terminal("Hyprland", |_| true), Some("foot"));
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
    let target = resolve_target(url.as_str().as_ref()).unwrap();
    assert_eq!(target.directory, dir.canonicalize().unwrap());
    assert_eq!(target.file, Some(file.canonicalize().unwrap()));
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
