use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[allow(dead_code)]
    pub version: u32,
    pub server_url: String,
    pub public_url: String,
    pub proxy_port: u16,
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".nightshift").join("config.json")
}

fn load_from(path: &Path) -> Option<Config> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn load() -> Option<Config> {
    let cfg = load_from(&config_path());
    if cfg.is_some() {
        return cfg;
    }

    #[cfg(debug_assertions)]
    {
        tracing::info!("no config file found, using dev defaults");
        Some(Config {
            version: 1,
            server_url: "http://localhost:4001".into(),
            public_url: "http://localhost:19277".into(),
            proxy_port: 19277,
        })
    }

    #[cfg(not(debug_assertions))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn should_parse_valid_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{"version":1,"serverUrl":"https://nightshift.fly.dev","publicUrl":"https://sprite-abc.fly.dev:8080","proxyPort":8080}"#,
        )
        .unwrap();

        let cfg = load_from(&path).expect("should parse");
        assert_eq!(cfg.version, 1);
        assert_eq!(cfg.server_url, "https://nightshift.fly.dev");
        assert_eq!(cfg.public_url, "https://sprite-abc.fly.dev:8080");
        assert_eq!(cfg.proxy_port, 8080);
    }

    #[test]
    fn should_return_none_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        assert!(load_from(&path).is_none());
    }

    #[test]
    fn should_return_none_when_json_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "not json").unwrap();
        assert!(load_from(&path).is_none());
    }

    #[test]
    fn should_return_none_when_required_fields_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"serverUrl":"https://example.com"}"#).unwrap();
        assert!(load_from(&path).is_none());
    }

}
