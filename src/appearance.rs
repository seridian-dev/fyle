//! Persistent appearance policy; no GUI or graphics dependencies.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub effects: bool,
    pub animate: bool,
    pub intensity: f32,
    pub scale: f32,
    pub highlight: [u8; 3],
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
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
