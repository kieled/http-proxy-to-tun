use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
    FullDark,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: Theme,
    pub killswitch: bool,
    pub last_proxy_id: Option<String>,
    pub minimize_to_tray: bool,
    pub auto_connect: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            killswitch: true,
            last_proxy_id: None,
            minimize_to_tray: true,
            auto_connect: false,
        }
    }
}

pub struct SettingsStore {
    config_path: PathBuf,
    settings: AppSettings,
}

impl SettingsStore {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::internal("Could not find config directory"))?
            .join("http-tun");

        fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("settings.json");
        let settings = if config_path.exists() {
            serde_json::from_str(&fs::read_to_string(&config_path)?)?
        } else {
            AppSettings::default()
        };

        Ok(Self {
            config_path,
            settings,
        })
    }

    fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.settings)?;
        fs::write(&self.config_path, json)?;
        Ok(())
    }

    pub fn get(&self) -> AppSettings {
        self.settings.clone()
    }

    pub fn update(&mut self, settings: AppSettings) -> Result<AppSettings> {
        self.settings = settings;
        self.save()?;
        Ok(self.settings.clone())
    }

    pub fn set_theme(&mut self, theme: Theme) -> Result<()> {
        self.settings.theme = theme;
        self.save()
    }

    pub fn set_killswitch(&mut self, enabled: bool) -> Result<()> {
        self.settings.killswitch = enabled;
        self.save()
    }

    pub fn set_last_proxy(&mut self, proxy_id: Option<String>) -> Result<()> {
        self.settings.last_proxy_id = proxy_id;
        self.save()
    }

    pub fn set_minimize_to_tray(&mut self, enabled: bool) -> Result<()> {
        self.settings.minimize_to_tray = enabled;
        self.save()
    }

    pub fn set_auto_connect(&mut self, enabled: bool) -> Result<()> {
        self.settings.auto_connect = enabled;
        self.save()
    }
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            config_path: PathBuf::new(),
            settings: AppSettings::default(),
        })
    }
}
