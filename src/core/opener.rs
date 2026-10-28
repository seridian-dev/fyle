//! Cross-platform file opening functionality.

use crate::types::FileManagerError;
use anyhow::Result;
use std::path::Path;
use std::process::Command;

/// Open a file with the system's default application.
///
/// This function uses the appropriate command for each platform:
/// - Linux/BSD: `xdg-open`
/// - macOS: `open`
/// - Windows: `explorer`
///
/// # Errors
/// Returns an error if the process cannot be spawned.
#[allow(dead_code)] // Public API for library use
pub fn open_file(path: &Path) -> Result<()> {
    let opener = get_system_opener();

    Command::new(opener)
        .arg(path)
        .spawn()
        .map_err(|e| FileManagerError::OpenFailed(format!("{}: {}", path.display(), e)))?;

    Ok(())
}

/// Open a file with a specific application.
///
/// # Errors
/// Returns an error if the process cannot be spawned.
pub fn open_with(path: &Path, application: &str) -> Result<()> {
    Command::new(application)
        .arg(path)
        .spawn()
        .map_err(|e| FileManagerError::OpenFailed(format!("{}: {}", path.display(), e)))?;

    Ok(())
}

/// Open a terminal in the given directory.
///
/// # Errors
/// Returns an error if no terminal is found or spawning fails.
pub fn open_terminal(path: &Path) -> Result<()> {
    let terminal = std::env::var("TERMINAL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| get_system_terminal().to_owned());
    let mut cmd = crate::desktop::terminal_command(&terminal, path);
    cmd.spawn().map_err(|e| {
        FileManagerError::OpenFailed(format!("Failed to spawn terminal {terminal}: {e}"))
    })?;
    Ok(())
}

/// Select an installed terminal for the active desktop, independent of init system.
#[must_use]
pub fn get_system_terminal() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "open"
    }
    #[cfg(target_os = "windows")]
    {
        "cmd"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let present = |key| std::env::var_os(key).is_some_and(|value| !value.is_empty());
        let session = crate::desktop::detect_session(
            &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            &std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default(),
            &std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
            present("HYPRLAND_INSTANCE_SIGNATURE"),
            present("SWAYSOCK"),
            present("WAYLAND_DISPLAY"),
            present("DISPLAY"),
        );
        crate::desktop::select_terminal_for_session(session, is_application_available)
            .unwrap_or("xterm")
    }
}

/// Get the system's default file opener command.
#[must_use]
pub fn get_system_opener() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "xdg-open"
    }

    #[cfg(target_os = "macos")]
    {
        "open"
    }

    #[cfg(target_os = "windows")]
    {
        "explorer"
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        // Fallback for BSD and other Unix-like systems
        "xdg-open"
    }
}

/// Check if a given application is available in the system PATH.
#[must_use]
pub fn is_application_available(app: &str) -> bool {
    let executable = |path: &std::path::Path| {
        if !path.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            path.metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            true
        }
    };
    if Path::new(app).components().count() > 1 {
        return executable(Path::new(app));
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| executable(&dir.join(app))))
        .unwrap_or(false)
}

/// Get a recommended application for a file based on its extension.
///
/// This returns the best available application, falling back to the system default.
#[must_use]
pub fn get_recommended_app(extension: &str) -> &'static str {
    let ext = extension.to_lowercase();

    match ext.as_str() {
        // Images - prefer common image viewers
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" => {
            get_system_opener() // Use system default for images
        }

        // Videos - prefer mpv if available, otherwise system default
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "m4v" => {
            if is_application_available("mpv") {
                "mpv"
            } else if is_application_available("vlc") {
                "vlc"
            } else {
                get_system_opener()
            }
        }

        // Audio - prefer mpv if available
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "opus" | "wma" => {
            if is_application_available("mpv") {
                "mpv"
            } else if is_application_available("vlc") {
                "vlc"
            } else {
                get_system_opener()
            }
        }

        // Text editors
        "txt" | "md" | "rst" | "org" | "cfg" | "ini" | "conf" => get_system_opener(),

        // Default - use system opener
        _ => get_system_opener(),
    }
}

/// Open a file with the recommended application based on its extension.
///
/// # Errors
/// Returns an error if the process cannot be spawned.
pub fn open_with_recommended(path: &Path) -> Result<()> {
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

    let app = get_recommended_app(extension);
    open_with(path, app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_system_opener() {
        let opener = get_system_opener();

        #[cfg(target_os = "linux")]
        assert_eq!(opener, "xdg-open");

        #[cfg(target_os = "macos")]
        assert_eq!(opener, "open");

        #[cfg(target_os = "windows")]
        assert_eq!(opener, "explorer");
    }

    #[test]
    fn test_get_recommended_app_video() {
        let app = get_recommended_app("mp4");
        // Should return either mpv, vlc, or system opener
        assert!(!app.is_empty());
    }

    #[test]
    fn test_get_recommended_app_unknown() {
        let app = get_recommended_app("unknown_extension");
        assert_eq!(app, get_system_opener());
    }

    #[test]
    fn test_get_recommended_app_case_insensitive() {
        let app_lower = get_recommended_app("mp4");
        let app_upper = get_recommended_app("MP4");
        assert_eq!(app_lower, app_upper);
    }

    #[test]
    fn test_is_application_available() {
        // 'ls' should be available on most Unix systems
        #[cfg(not(target_os = "windows"))]
        {
            // This test is informational - we don't assert because
            // it depends on the system configuration
            let _ls_available = is_application_available("ls");
        }
    }
}
