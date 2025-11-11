use directories::ProjectDirs;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct FileConfig {
    pub detailed: bool,
    pub fullscreen: bool,
    pub device: Option<String>,
    pub png_path: Option<String>,
    pub csv_path: Option<String>,
}

pub fn config_dir() -> Option<PathBuf> {
    ProjectDirs::from("io", "sgram", "sgram-tui").map(|p| p.config_dir().to_path_buf())
}

pub fn load_config() -> Option<FileConfig> {
    let dir = config_dir()?;
    let path = dir.join("config.toml");
    let data = fs::read_to_string(path).ok()?;
    toml::from_str::<FileConfig>(&data).ok()
}
