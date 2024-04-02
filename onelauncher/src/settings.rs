use std::fs;

use serde::{Deserialize, Serialize};

use crate::utils::dirs;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Setings {
    pub discord_rpc: bool,
}

impl Default for Setings {
    fn default() -> Self {
        Self {
            discord_rpc: true,
        }
    }
}

#[derive(Debug)]
pub struct SettingsManager {
    pub settings: Setings,
}

impl SettingsManager {
    pub fn new() -> crate::Result<Self> {
        let path = dirs::app_config_dir()?.join("settings.json");

        if path.exists() {
            let file = fs::read_to_string(path)?;
            let settings: Setings = serde_json::from_str(&file)?;

            return Ok(Self { 
                settings
            });
        }

        let this = Self { 
            settings: Setings::default()
        };

        this.save()?;
        Ok(this)
    }

    pub fn get_settings(&self) -> &Setings {
        &self.settings
    }

    pub fn get_settings_mut(&mut self) -> &mut Setings {
        &mut self.settings
    }

    pub fn save(&self) -> crate::Result<()> {
        let path = dirs::app_config_dir()?.join("settings.json");
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, serde_json::to_string(&self.settings)?)?;

        Ok(())
    }
}