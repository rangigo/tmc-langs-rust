//! Handles the CLI's configuration file.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use toml::{value::Table, Value};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TmcConfig {
    pub projects_dir: PathBuf,
    #[serde(flatten)]
    pub table: Table,
}

impl TmcConfig {
    pub fn get(&self, key: &str) -> ConfigValue {
        match key {
            "projects-dir" => ConfigValue::Path(&self.projects_dir),
            _ => ConfigValue::Value(self.table.get(key)),
        }
    }

    pub fn insert(&mut self, key: String, value: Value) -> Result<()> {
        match key.as_str() {
            "projects-dir" => {
                if let Value::String(value) = value {
                    let path = PathBuf::from(value);
                    self.set_projects_dir(path)?;
                } else {
                    anyhow::bail!("The value for projects-dir must be a string.")
                }
            }
            _ => {
                self.table.insert(key, value);
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Result<Option<Value>> {
        match key {
            "projects-dir" => anyhow::bail!("projects-dir must always be defined"),
            _ => Ok(self.table.remove(key)),
        }
    }

    pub fn set_projects_dir(&mut self, mut target: PathBuf) -> Result<PathBuf> {
        // check if the directory is empty or not
        if fs::read_dir(&target)
            .with_context(|| format!("Failed to read directory at {}", target.display()))?
            .next()
            .is_some()
        {
            anyhow::bail!("Cannot set projects-dir to a non-empty directory.");
        }
        std::mem::swap(&mut self.projects_dir, &mut target);
        Ok(target)
    }

    pub fn save(self, client_name: &str) -> Result<()> {
        let path = Self::get_location(client_name)?;
        let toml = toml::to_string_pretty(&self).context("Failed to serialize HashMap")?;
        fs::write(&path, toml.as_bytes())
            .with_context(|| format!("Failed to write TOML to {}", path.display()))?;
        Ok(())
    }

    pub fn reset(client_name: &str) -> Result<()> {
        let path = Self::get_location(client_name)?;
        Self::init_at(client_name, &path)?;
        Ok(())
    }

    pub fn load(client_name: &str) -> Result<TmcConfig> {
        let path = Self::get_location(client_name)?;
        let config = match fs::read(&path) {
            Ok(bytes) => match toml::from_slice(&bytes) {
                Ok(config) => Ok(config),
                Err(_) => {
                    log::error!(
                        "Failed to deserialize config at {}, resetting",
                        path.display()
                    );
                    Self::init_at(client_name, &path)
                }
            },
            Err(_) => Self::init_at(client_name, &path),
        }?;
        if !config.projects_dir.exists() {
            fs::create_dir_all(&config.projects_dir).with_context(|| {
                format!(
                    "Failed to create projects-dir at {}",
                    config.projects_dir.display()
                )
            })?;
        }
        Ok(config)
    }

    // initializes the default configuration file at the given path
    fn init_at(client_name: &str, path: &Path) -> Result<TmcConfig> {
        let mut file = File::create(&path)
            .with_context(|| format!("Failed to create new config file at {}", path.display()))?;

        let default_project_dir = dirs::data_local_dir()
            .context("Failed to find local data directory")?
            .join("tmc")
            .join(Self::get_client_stub(client_name));
        fs::create_dir_all(&default_project_dir).with_context(|| {
            format!(
                "Failed to create the TMC default project directory in {}",
                default_project_dir.display()
            )
        })?;

        let config = TmcConfig {
            projects_dir: default_project_dir,
            table: Table::new(),
        };

        let toml = toml::to_string_pretty(&config).context("Failed to serialize config")?;
        file.write_all(toml.as_bytes())
            .with_context(|| format!("Failed to write default config to {}", path.display()))?;
        Ok(config)
    }

    // path to the configuration file
    fn get_location(client_name: &str) -> Result<PathBuf> {
        super::get_tmc_dir(client_name).map(|dir| dir.join("config.toml"))
    }

    // some client use a different name for the directory
    fn get_client_stub(client: &str) -> &str {
        match client {
            "vscode_plugin" => "vscode",
            s => s,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ConfigValue<'a> {
    Value(Option<&'a Value>),
    Path(&'a Path),
}
