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
    pub custom_environments:
        std::collections::HashMap<String, std::collections::HashMap<String, toml::Value>>,
}

impl ProjectConfig {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut config_file_path = PathBuf::from(path);
        config_file_path.push("jmeter/config.toml");

        let config_str = std::fs::read_to_string(&config_file_path).map_err(|err| {
            format!(
                "Could not read config file at {}: {}",
                config_file_path.display(),
                err
            )
        })?;

        let project_config = toml::from_str(&config_str).map_err(|err| {
            format!(
                "Could not parse config file at {}: {}",
                config_file_path.display(),
                err
            )
        })?;

        Ok(project_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_project_config_open_valid() {
        let tmp = tempdir().unwrap();
        let jmeter_dir = tmp.path().join("jmeter");
        fs::create_dir(&jmeter_dir).unwrap();

        let config_content = r#"
[jmeter]
inactivity_kill_timeout = 60
default_test_plan = "test.jmx"

[plugins]
jpgc-dummy = "0.4"

[env.dev]
URL = "http://localhost"
PORT = "8080"
"#;
        fs::write(jmeter_dir.join("config.toml"), config_content).unwrap();

        let config = ProjectConfig::open(tmp.path()).unwrap();
        assert_eq!(config.jmeter.inactivity_kill_timeout, Some(60));
        assert_eq!(
            config.jmeter.default_test_plan,
            Some("test.jmx".to_string())
        );
        assert_eq!(
            config.plugins.plugin_map.get("jpgc-dummy"),
            Some(&"0.4".to_string())
        );
        assert!(config.env.custom_environments.contains_key("dev"));
    }

    #[test]
    fn test_project_config_open_missing_file() {
        let tmp = tempdir().unwrap();
        let result = ProjectConfig::open(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Could not read config file"));
    }

    #[test]
    fn test_project_config_open_invalid_toml() {
        let tmp = tempdir().unwrap();
        let jmeter_dir = tmp.path().join("jmeter");
        fs::create_dir(&jmeter_dir).unwrap();

        let invalid_content = "this is not valid toml [[[";
        fs::write(jmeter_dir.join("config.toml"), invalid_content).unwrap();

        let result = ProjectConfig::open(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Could not parse config file"));
    }

    #[test]
    fn test_project_config_optional_fields() {
        let tmp = tempdir().unwrap();
        let jmeter_dir = tmp.path().join("jmeter");
        fs::create_dir(&jmeter_dir).unwrap();

        let minimal_config = r#"
[jmeter]

[plugins]

[env.prod]
URL = "https://example.com"
"#;
        fs::write(jmeter_dir.join("config.toml"), minimal_config).unwrap();

        let config = ProjectConfig::open(tmp.path()).unwrap();
        assert_eq!(config.jmeter.inactivity_kill_timeout, None);
        assert_eq!(config.jmeter.default_test_plan, None);
        assert!(config.plugins.plugin_map.is_empty());
        assert!(config.env.custom_environments.contains_key("prod"));
    }
}
