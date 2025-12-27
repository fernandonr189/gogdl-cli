use std::{process::exit, sync::Arc};

use prefix_manager::{PrefixManager, api::client::ClientError};

use crate::settings::AppSettings;

pub async fn handle_proton(
    list: bool,
    version: Option<String>,
    page: i32,
    settings: &mut AppSettings,
) -> Result<(), ClientError> {
    let prefix_manager = PrefixManager::new_with_default_client();
    let prefix_manager_arc = Arc::new(prefix_manager);
    let releases = prefix_manager_arc.get_releases(page).await?;
    if list {
        for release in &releases {
            println!("Version: {}", release.tag_name);
        }
    }

    if let Some(version) = version {
        let target_release = releases.iter().find(|rel| rel.tag_name == version);

        if let Some(target_release) = target_release {
            let checksum = target_release.get_checksum();
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<i64>();

            let release_clone = target_release.clone();

            let proton_path = format!("{}/proton", settings.data_path);

            tokio::spawn(async move {
                let _ = prefix_manager_arc
                    .download_release(&release_clone, &proton_path, checksum, tx)
                    .await;
            });

            let total_size = target_release.get_download_size().unwrap_or(0);
            let mut downloaded_size = 0;
            while let Some(size) = rx.recv().await {
                downloaded_size += size;
                print!(
                    "\rDownloaded: {} MB/{} MB -- {:.2}%",
                    downloaded_size / 1024 / 1024,
                    total_size / 1024 / 1024,
                    downloaded_size as f64 / total_size as f64 * 100.0
                );
            }
        } else {
            println!("Version not found");
            exit(1)
        }
    }

    Ok(())
}
