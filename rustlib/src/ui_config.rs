use std::fs;
use std::io::{ErrorKind, Write};
use std::sync::{Arc, Mutex};

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

const UI_CONFIG_FILE: &str = "ui.config.json";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ColorScheme {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct UiConfig {
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub color_scheme: ColorScheme,
}

impl UiConfig {
    fn load(dir: &Utf8Path) -> UiConfig {
        let path = dir.join(UI_CONFIG_FILE);
        let json = match fs::read(&path) {
            Ok(json) => json,
            Err(error) if error.kind() == ErrorKind::NotFound => return UiConfig::default(),
            Err(error) => {
                tracing::error!(message_id = "cJ4wRt8B", %path, %error, "failed to read ui config, using defaults");
                return UiConfig::default();
            }
        };
        match serde_json::from_slice(&json) {
            Ok(ui_config) => ui_config,
            Err(error) => {
                tracing::error!(message_id = "sQ7kNv2H", %path, %error, "failed to parse ui config, using defaults");
                UiConfig::default()
            }
        }
    }

    fn save(&self, dir: &Utf8Path) -> Result<(), ()> {
        let json = serde_json::to_vec_pretty(self).map_err(|error| tracing::error!(message_id = "mV3xDb6T", %error, "failed to encode ui config"))?;
        fs::create_dir_all(dir).map_err(|error| tracing::error!(message_id = "hR8pLc4W", %dir, %error, "failed to create ui config directory"))?;
        let mut file = NamedTempFile::new_in(dir)
            .map_err(|error| tracing::error!(message_id = "yF2nGw7K", %dir, %error, "failed to create temporary ui config file"))?;
        file.write_all(&json)
            .and_then(|()| file.as_file_mut().sync_data())
            .map_err(|error| tracing::error!(message_id = "bX6tJm3P", %dir, %error, "failed to write temporary ui config file"))?;
        let path = dir.join(UI_CONFIG_FILE);
        file.persist(&path)
            .map_err(|error| tracing::error!(message_id = "wZ5qSh9D", %path, %error, "failed to persist ui config file"))?;
        Ok(())
    }
}

pub struct UiConfigHandle {
    dir: Option<Utf8PathBuf>,
    ui_config: Mutex<UiConfig>,
}

impl UiConfigHandle {
    pub fn load(dir: Option<Utf8PathBuf>) -> Self {
        let ui_config = match &dir {
            Some(dir) => UiConfig::load(dir),
            None => {
                tracing::error!(message_id = "gN7cVs3Q", "no ui config directory, using defaults without persistence");
                UiConfig::default()
            }
        };
        Self { dir, ui_config: Mutex::new(ui_config) }
    }

    pub fn get(&self) -> UiConfig {
        self.ui_config.lock().unwrap().clone()
    }

    pub async fn update(self: Arc<Self>, update: impl FnOnce(&mut UiConfig) + Send + 'static) {
        let task = tokio::task::spawn_blocking(move || {
            let mut ui_config = self.ui_config.lock().unwrap();
            update(&mut ui_config);
            if let Some(dir) = &self.dir {
                let _ = ui_config.save(dir);
            }
        });
        if let Err(error) = task.await {
            tracing::error!(message_id = "dQ2mXf7R", %error, "ui config update task failed");
        }
    }
}
