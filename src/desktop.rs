//! Desktop policy is pure: callers supply executable discovery for deterministic tests.
pub fn select_terminal(desktop: &str, available: impl Fn(&str) -> bool) -> Option<&'static str> {
    let desktop = desktop.to_ascii_lowercase();
    let preferred = if desktop.split(':').any(|d| matches!(d, "kde" | "plasma")) {
        "konsole"
    } else if desktop.contains("gnome") || desktop.contains("ubuntu") {
        "gnome-terminal"
    } else if desktop.contains("hyprland") || desktop.contains("sway") {
        "foot"
    } else if desktop.contains("xfce") {
        "xfce4-terminal"
    } else {
        "x-terminal-emulator"
    };
    std::iter::once(preferred)
        .chain([
            "x-terminal-emulator",
            "konsole",
            "gnome-terminal",
            "kgx",
            "xfce4-terminal",
            "foot",
            "kitty",
            "alacritty",
            "wezterm",
            "xterm",
        ])
        .find(|app| available(app))
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
