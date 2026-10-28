//! Stable visual and general display preferences; no GUI dependencies.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalFont {
    #[default]
    JetBrainsMonoNerd,
    SystemMonospace,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalTheme {
    #[default]
    FyleMidnight,
    Nord,
    SolarizedDark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalCursor {
    #[default]
    Block,
    Bar,
    Underline,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Keep the contextual file/status summary visible at the bottom of the window.
    pub show_status_bar: bool,
    pub terminal_font: TerminalFont,
    pub terminal_font_size: f32,
    pub terminal_line_spacing: f32,
    pub terminal_theme: TerminalTheme,
    pub terminal_cursor: TerminalCursor,
    pub allow_external_terminals: bool,
    pub effects: bool,
    pub animate: bool,
    pub intensity: f32,
    pub scale: f32,
    pub highlight: [u8; 3],
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            show_status_bar: true,
            terminal_font: TerminalFont::JetBrainsMonoNerd,
            terminal_font_size: 14.0,
            terminal_line_spacing: 1.2,
            terminal_theme: TerminalTheme::FyleMidnight,
            terminal_cursor: TerminalCursor::Block,
            allow_external_terminals: false,
            effects: true,
            animate: true,
            intensity: 0.45,
            scale: 1.0,
            highlight: [0, 212, 255],
        }
    }
}
impl Appearance {
    pub fn normalized(mut self) -> Self {
        self.intensity = if self.intensity.is_finite() {
            self.intensity.clamp(0.0, 1.0)
        } else {
            0.45
        };
        self.scale = if self.scale.is_finite() {
            self.scale.clamp(0.85, 1.5)
        } else {
            1.0
        };
        self.terminal_font_size = if self.terminal_font_size.is_finite() {
            self.terminal_font_size.clamp(10.0, 28.0)
        } else {
            14.0
        };
        self.terminal_line_spacing = if self.terminal_line_spacing.is_finite() {
            self.terminal_line_spacing.clamp(0.9, 1.8)
        } else {
            1.2
        };
        self
    }
}
pub struct SettingsStore {
    pub path: PathBuf,
}
impl Default for SettingsStore {
    fn default() -> Self {
        let root = std::env::var_os("FYLE_SETTINGS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("fyle")
            });
        Self::new(root.join("appearance.json"))
    }
}
impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn load(&self) -> Result<Appearance> {
        if !self.path.exists() {
            return Ok(Appearance::default());
        }
        ensure!(
            fs::metadata(&self.path)?.len() <= 65536,
            "Appearance settings exceed 64 KiB"
        );
        Ok(serde_json::from_slice::<Appearance>(&fs::read(&self.path)?)
            .context("Invalid appearance settings")?
            .normalized())
    }
    pub fn save(&self, settings: &Appearance) -> Result<()> {
        let parent = self.path.parent().context("Settings path has no parent")?;
        fs::create_dir_all(parent)?;
        let temp = parent.join(format!(".appearance-{}.tmp", crate::harness::random_id()?));
        let result = (|| -> Result<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&serde_json::to_vec_pretty(&settings.clone().normalized())?)?;
            file.sync_all()?;
            fs::rename(&temp, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_settings_default_to_showing_the_status_bar() {
        let settings: Appearance = serde_json::from_str(
            r#"{"effects":false,"animate":false,"intensity":0.2,"scale":1.0,"highlight":[0,212,255]}"#,
        )
        .unwrap();
        assert!(settings.show_status_bar);
    }

    #[test]
    fn status_bar_preference_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("appearance.json"));
        let settings = Appearance {
            show_status_bar: false,
            terminal_font: TerminalFont::SystemMonospace,
            terminal_font_size: 18.0,
            terminal_line_spacing: 1.4,
            terminal_theme: TerminalTheme::Nord,
            terminal_cursor: TerminalCursor::Bar,
            allow_external_terminals: true,
            ..Appearance::default()
        };
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert!(!loaded.show_status_bar);
        assert_eq!(loaded.terminal_font, TerminalFont::SystemMonospace);
        assert_eq!(loaded.terminal_font_size, 18.0);
        assert_eq!(loaded.terminal_line_spacing, 1.4);
        assert_eq!(loaded.terminal_theme, TerminalTheme::Nord);
        assert_eq!(loaded.terminal_cursor, TerminalCursor::Bar);
        assert!(loaded.allow_external_terminals);
    }

    #[test]
    fn terminal_settings_are_clamped_to_readable_values() {
        let settings = Appearance {
            terminal_font_size: f32::INFINITY,
            terminal_line_spacing: -100.0,
            ..Appearance::default()
        }
        .normalized();
        assert_eq!(settings.terminal_font_size, 14.0);
        assert_eq!(settings.terminal_line_spacing, 0.9);
    }
}
