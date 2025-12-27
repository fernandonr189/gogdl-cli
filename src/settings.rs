use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Serialize, Deserialize)]
pub struct DownloadedGame {
    pub build_id: String,
    pub path: String,
}

#[derive(Serialize, Deserialize)]
pub struct AppSettings {
    pub data_path: String,
    pub downloaded_games: Vec<DownloadedGame>,
}

impl AppSettings {
    pub async fn initialize() -> Result<Self, anyhow::Error> {
        if let Some(project_dirs) = ProjectDirs::from("com", "fernandonr189", "gogdl") {
            let path = project_dirs.config_dir().join("settings.json");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }
            let default = Self::default();
            fs::write(&path, serde_json::to_string_pretty(&default)?).await?;

            Ok(default)
        } else {
            Err(anyhow::anyhow!("Failed to initialize settings"))
        }
    }

    pub async fn load() -> Result<Self, anyhow::Error> {
        if let Some(project_dirs) = ProjectDirs::from("com", "fernandonr189", "gogdl") {
            let path = project_dirs.config_dir().join("settings.json");
            if path.exists() {
                let contents = fs::read_to_string(path).await?;
                let settings = match serde_json::from_str(&contents) {
                    Ok(settings) => settings,
                    Err(err) => {
                        println!("Failed to parse settings file: {}", err);
                        Self::initialize().await?
                    }
                };
                Ok(settings)
            } else {
                Self::initialize().await
            }
        } else {
            Err(anyhow::anyhow!("Failed to load settings"))
        }
    }
    pub async fn _save(&self) -> Result<(), anyhow::Error> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "fernandonr189", "gogdl") {
            let dir = proj_dirs.config_dir();
            let file_path = dir.join("settings.json");
            let contents = serde_json::to_string_pretty(self)?;
            fs::write(file_path, contents).await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to save settings"))
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from("com", "fernandonr189", "gogdl") {
            let dir = proj_dirs.data_dir();

            AppSettings {
                data_path: dir.to_string_lossy().into_owned(),
                downloaded_games: Vec::new(),
            }
        } else {
            AppSettings {
                data_path: "".to_string(),
                downloaded_games: Vec::new(),
            }
        }
    }
}
