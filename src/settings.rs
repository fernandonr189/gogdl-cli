use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Serialize, Deserialize)]
pub struct DownloadedGame {
    pub build_id: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub custom_prefix_path: Option<String>,
    pub downloaded_games: Vec<DownloadedGame>,
}

impl AppSettings {
    pub async fn load() -> Result<Self, anyhow::Error> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "fernandonr189", "gogdl") {
            let dir = proj_dirs.config_dir();
            let contents = fs::read_to_string(dir).await?;
            let settings: AppSettings = serde_json::from_str(&contents)?;
            Ok(settings)
        } else {
            Ok(AppSettings::default())
        }
    }
}
