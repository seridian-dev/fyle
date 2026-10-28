//! Pure desktop/session policy. Keep environment reads in the OS boundary.

pub mod dnd;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEnvironment {
    Plasma,
    Gnome,
    Hyprland,
    Sway,
    Xfce,
    Cinnamon,
    Mate,
    Lxqt,
    Cosmic,
    Other,
}

impl DesktopEnvironment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plasma => "plasma",
            Self::Gnome => "gnome",
            Self::Hyprland => "hyprland",
            Self::Sway => "sway",
            Self::Xfce => "xfce",
            Self::Cinnamon => "cinnamon",
            Self::Mate => "mate",
            Self::Lxqt => "lxqt",
            Self::Cosmic => "cosmic",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayBackend {
    Wayland,
    X11,
    Unknown,
}

impl DisplayBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wayland => "wayland",
            Self::X11 => "x11",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopSession {
    pub environment: DesktopEnvironment,
    pub display: DisplayBackend,
}

/// Identify a desktop from standard XDG values, with compositor sockets as fallback.
/// XDG_CURRENT_DESKTOP commonly contains colon-separated vendor and desktop names.
pub fn detect_desktop(
    current_desktop: &str,
    session_desktop: &str,
    hyprland_running: bool,
    sway_running: bool,
) -> DesktopEnvironment {
    let hints = [current_desktop, session_desktop];
    let matches = |names: &[&str]| {
        hints.iter().any(|hint| {
            hint.split([':', ';', ','])
                .map(|part| part.trim().to_ascii_lowercase())
                .any(|part| names.contains(&part.as_str()))
        })
    };

    if matches(&["kde", "plasma"]) {
        DesktopEnvironment::Plasma
    } else if matches(&["gnome", "ubuntu"]) {
        DesktopEnvironment::Gnome
    } else if matches(&["hyprland"]) || hyprland_running {
        DesktopEnvironment::Hyprland
    } else if matches(&["sway"]) || sway_running {
        DesktopEnvironment::Sway
    } else if matches(&["xfce", "xfce4"]) {
        DesktopEnvironment::Xfce
    } else if matches(&["cinnamon", "x-cinnamon"]) {
        DesktopEnvironment::Cinnamon
    } else if matches(&["mate", "mate-session"]) {
        DesktopEnvironment::Mate
    } else if matches(&["lxqt"]) {
        DesktopEnvironment::Lxqt
    } else if matches(&["cosmic"]) {
        DesktopEnvironment::Cosmic
    } else {
        DesktopEnvironment::Other
    }
}

/// Prefer the explicit XDG session type, then infer from display socket presence.
pub fn detect_display_backend(
    session_type: &str,
    wayland_display_present: bool,
    x_display_present: bool,
) -> DisplayBackend {
    match session_type.trim().to_ascii_lowercase().as_str() {
        "wayland" => DisplayBackend::Wayland,
        "x11" | "xorg" => DisplayBackend::X11,
        _ if wayland_display_present => DisplayBackend::Wayland,
        _ if x_display_present => DisplayBackend::X11,
        _ => DisplayBackend::Unknown,
    }
}

pub fn detect_session(
    current_desktop: &str,
    session_desktop: &str,
    session_type: &str,
    hyprland_running: bool,
    sway_running: bool,
    wayland_display_present: bool,
    x_display_present: bool,
) -> DesktopSession {
    DesktopSession {
        environment: detect_desktop(
            current_desktop,
            session_desktop,
            hyprland_running,
            sway_running,
        ),
        display: detect_display_backend(session_type, wayland_display_present, x_display_present),
    }
}

/// Pick a suitable installed terminal for the active desktop session.
pub fn select_terminal_for_session(
    session: DesktopSession,
    available: impl Fn(&str) -> bool,
) -> Option<&'static str> {
    let preferred: &[&str] = match session.environment {
        DesktopEnvironment::Plasma => &["konsole"],
        DesktopEnvironment::Gnome => &["kgx", "gnome-terminal"],
        DesktopEnvironment::Hyprland | DesktopEnvironment::Sway => &["foot"],
        DesktopEnvironment::Xfce => &["xfce4-terminal"],
        DesktopEnvironment::Cinnamon => &["gnome-terminal", "kgx"],
        DesktopEnvironment::Mate => &["mate-terminal"],
        DesktopEnvironment::Lxqt => &["qterminal"],
        DesktopEnvironment::Cosmic => &["cosmic-term"],
        DesktopEnvironment::Other => &[],
    };
    preferred
        .iter()
        .copied()
        .chain([
            "x-terminal-emulator",
            "konsole",
            "kgx",
            "gnome-terminal",
            "xfce4-terminal",
            "foot",
            "kitty",
            "alacritty",
            "wezterm",
            "xterm",
        ])
        .find(|app| available(app))
}

/// Backwards-compatible selector for callers that only have a desktop hint.
pub fn select_terminal(desktop: &str, available: impl Fn(&str) -> bool) -> Option<&'static str> {
    let session = detect_session(desktop, "", "", false, false, false, false);
    select_terminal_for_session(session, available)
}

/// Classify the init manager from the OpenRC marker or PID 1's command name.
pub fn detect_init_system(openrc_marker: bool, pid1_name: &str) -> &'static str {
    if openrc_marker {
        "openrc"
    } else if pid1_name.trim().eq_ignore_ascii_case("systemd") {
        "systemd"
    } else {
        "other"
    }
}

/// Launch with a child-local working directory, without terminal-specific flags.
/// TERMINAL is an executable name/path, never a shell command.
pub fn terminal_command(terminal: &str, path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new(terminal);
    command.current_dir(path);
    #[cfg(target_os = "macos")]
    command.arg("-a").arg("Terminal").arg(path);
    command
}
