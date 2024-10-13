use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::{fs, io::AsyncWriteExt};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("Expected file, found directory: {0}")]
    FoundDirectory(PathBuf),
    #[error("JSON decode error: {0}")]
    JsonDecode(serde_jsonc::Error),
    #[error("JSON encode error: {0}")]
    JsonEncode(serde_jsonc::Error),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub address: String,
    pub port: u16,
    pub storage: StorageConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 8080,
            storage: StorageConfig::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub path: Option<PathBuf>,
}

pub struct ConfigFile {
    pub config: Config,
    path: PathBuf,
}

impl ConfigFile {
    pub async fn new(path: PathBuf) -> Result<Self, Error> {
        if path.is_file() {
            let config_raw = fs::read_to_string(&path).await.map_err(Error::Io)?;

            let config = serde_jsonc::from_str(&config_raw).map_err(Error::JsonDecode)?;

            Ok(ConfigFile { path, config })
        } else if path.is_dir() {
            Err(Error::FoundDirectory(path))
        } else {
            let config_file = Self {
                path,
                config: Config::default(),
            };

            config_file.update().await?;

            Ok(config_file)
        }
    }
    pub async fn update(&self) -> Result<(), Error> {
        let mut f = fs::File::create(&self.path).await.map_err(Error::Io)?;

        let mut config_raw = serde_jsonc::to_string_pretty(&self.config).map_err(Error::JsonEncode)?;
        config_raw.push('\n');

        f.write_all(config_raw.as_bytes()).await.map_err(Error::Io)?;

        Ok(())
    }
}
