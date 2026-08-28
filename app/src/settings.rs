//! VS Code & Zed-style persistent settings system.
//!
//! Stores user configuration in a platform-standard `settings.json`:
//! - Windows: `%APPDATA%/html2gpui/settings.json`
//! - macOS:   `~/Library/Application Support/html2gpui/settings.json`
//! - Linux:   `~/.config/html2gpui/settings.json`

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoSaveMode {
    #[serde(alias = "off", alias = "OFF", alias = "false", alias = "none", alias = "disabled")]
    Off,
    #[serde(alias = "afterDelay", alias = "after_delay", alias = "on", alias = "ON", alias = "true", alias = "delay", alias = "auto", alias = "enabled")]
    AfterDelay,
    #[serde(alias = "onFocusChange", alias = "on_focus_change", alias = "focusChange", alias = "focus")]
    OnFocusChange,
}

impl Default for AutoSaveMode {
    fn default() -> Self {
        AutoSaveMode::Off
    }
}

impl AutoSaveMode {
    pub fn description(&self) -> &'static str {
        match self {
            AutoSaveMode::Off => "A dirty file is never automatically saved (Ctrl+S to save).",
            AutoSaveMode::AfterDelay => "A dirty file is automatically saved after the configured delay.",
            AutoSaveMode::OnFocusChange => "A dirty file is automatically saved when switching tabs or editor focus.",
        }
    }
}

fn default_font_size() -> f32 {
    14.5
}

fn default_theme() -> String {
    "One Dark Pro".to_string()
}

fn default_auto_save() -> AutoSaveMode {
    AutoSaveMode::Off
}

fn default_auto_save_delay() -> u64 {
    1000
}

fn default_tab_size() -> usize {
    4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(rename = "editor.fontSize", default = "default_font_size")]
    pub editor_font_size: f32,

    #[serde(rename = "workbench.colorTheme", default = "default_theme")]
    pub workbench_color_theme: String,

    #[serde(rename = "editor.autoSave", default = "default_auto_save")]
    pub editor_auto_save: AutoSaveMode,

    #[serde(rename = "editor.autoSaveDelay", default = "default_auto_save_delay")]
    pub editor_auto_save_delay: u64,

    #[serde(rename = "editor.tabSize", default = "default_tab_size")]
    pub editor_tab_size: usize,

    #[serde(rename = "terminal.integrated.shell", default, skip_serializing_if = "Option::is_none")]
    pub terminal_integrated_shell: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            editor_font_size: default_font_size(),
            workbench_color_theme: default_theme(),
            editor_auto_save: default_auto_save(),
            editor_auto_save_delay: default_auto_save_delay(),
            editor_tab_size: default_tab_size(),
            terminal_integrated_shell: None,
        }
    }
}

/// Returns the configuration directory for the current operating system.
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("html2gpui");
        }
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            return PathBuf::from(userprofile).join("AppData").join("Roaming").join("html2gpui");
        }
        PathBuf::from(".").join("config")
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("html2gpui");
        }
        PathBuf::from(".").join("config")
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("html2gpui");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join("html2gpui");
        }
        PathBuf::from(".").join("config")
    }
}

/// Returns the full path to `settings.json`.
pub fn settings_file_path() -> PathBuf {
    config_dir().join("settings.json")
}

impl Settings {
    /// Loads settings from `settings.json` on disk, creating a default file if none exists.
    pub fn load() -> Self {
        let path = settings_file_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<Settings>(&content) {
                return settings;
            }
        }

        let defaults = Settings::default();
        let _ = defaults.save();
        defaults
    }

    /// Saves the current settings to `settings.json` with pretty JSON formatting.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)?;
        let path = settings_file_path();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json.as_bytes())?;
        Ok(())
    }
}
