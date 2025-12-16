use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
pub struct ProjectConfig {
    pub jmeter: JMeterConfig,
    pub plugins: PluginsConfig,
    pub env: EnvConfig,
}

#[derive(Deserialize, Debug)]
pub struct JMeterConfig {
    pub inactivity_kill_timeout: Option<usize>,
    pub default_test_plan: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PluginsConfig {
    #[serde(flatten)]
    pub plugin_map: std::collections::HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
pub struct EnvConfig {
    #[serde(flatten)]
    pub custom_environments: std::collections::HashMap<String, std::collections::HashMap<String, toml::Value>>,
}

impl ProjectConfig {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut config_file_path = PathBuf::from(path);
        config_file_path.push("jmeter/config.toml");

        let config_str = std::fs::read_to_string(&config_file_path)
            .map_err(|err| format!("Could not read config file at {}: {}", config_file_path.display(), err))?;

        let project_config = toml::from_str(&config_str)
            .map_err(|err| format!("Could not parse config file at {}: {}", config_file_path.display(), err))?;

        Ok(project_config)
    }
}