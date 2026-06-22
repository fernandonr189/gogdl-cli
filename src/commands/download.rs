use std::{io::Write, process::exit, sync::Arc};

use chrono::{DateTime, Utc};
use gogdl_lib::{
    GogDl, GogdlError,
    client::ClientError,
    games::{DownloadOptions, OperatingSystem},
};
use tokio_util::sync::CancellationToken;

use crate::{auth::manage_auth, settings::AppSettings};

pub async fn handle_download(
    gogdl: Arc<GogDl>,
    game_id: i32,
    version_id: Option<String>,
    path: &str,
    settings: &mut AppSettings,
    fix: bool,
) -> Result<(), anyhow::Error> {
    let mut download_build = version_id.clone().unwrap_or_default();
    let game_details = gogdl.get_game_details(game_id).await?;

    let result = {
        if let Some(version_id) = version_id {
            if settings.downloaded_games.iter().any(|game| {
                game.game_id == game_id
                    && game.build_id == version_id
                    && game.download_complete
                    && !fix
            }) {
                println!("Game already downloaded");
                return Ok(());
            }
            download_game(gogdl.clone(), game_id, &version_id, path).await
        } else {
            let game_builds = match gogdl
                .get_game_builds(game_id, OperatingSystem::Windows)
                .await
            {
                Ok(builds) => builds.items,
                Err(err) => {
                    println!("Error fetching game builds: {}", err);
                    exit(1)
                }
            };

            let latest_build = game_builds
                .iter()
                .filter_map(|b| {
                    DateTime::parse_from_str(&b.date_published, "%Y-%m-%dT%H:%M:%S%z")
                        .ok()
                        .map(|dt| (dt.with_timezone(&Utc), b))
                })
                .max_by_key(|(dt, _)| *dt)
                .map(|(_, b)| b);

            if let Some(latest) = latest_build {
                download_build = latest.version_name.clone();
                if settings.downloaded_games.iter().any(|game| {
                    game.game_id == game_id
                        && game.build_id == download_build
                        && game.download_complete
                        && !fix
                }) {
                    println!("Game already downloaded");
                    return Ok(());
                }
                download_game(gogdl.clone(), game_id, &download_build, path).await
            } else {
                println!("Could not fetch latest build");
                exit(1)
            }
        }
    };

    let complete = match result {
        Ok(complete) => complete,
        Err(err) => {
            if let GogdlError::ClientError(ClientError::Http { status, .. }) = &err {
                if status.as_u16() == 401 {
                    manage_auth(gogdl.clone()).await;
                }
            }
            return Err(err.into());
        }
    };

    settings
        .add_game(
            &download_build,
            &format!("{}/{}", path, game_details.title),
            None,
            complete,
            game_id,
        )
        .await;
    Ok(())
}

pub async fn download_game(
    gogdl: Arc<GogDl>,
    game_id: i32,
    build_name: &str,
    path: &str,
) -> Result<bool, GogdlError> {
    let total_size = {
        let chunks = gogdl
            .get_build_chunks(game_id, OperatingSystem::Windows, build_name)
            .await?;
        let total_size: u64 = chunks.iter().map(|chunk| chunk.compressed_size).sum();
        println!("Total size: {} MB", total_size / 1024 / 1024);
        println!("Number of chunks: {}", chunks.len());
        total_size
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(i64, i64)>();
    let build_name_clone = build_name.to_string();
    let path_clone = path.to_string();

    // Spawn the download task
    let download_task = tokio::spawn(async move {
        gogdl
            .download_build(
                game_id,
                &build_name_clone,
                tx,
                &path_clone,
                OperatingSystem::Windows,
                CancellationToken::new(),
                DownloadOptions::default(),
            )
            .await
    });

    // Progress reporting loop
    let mut downloaded_size: i64 = 0;

    while let Some((downloaded, _total)) = rx.recv().await {
        downloaded_size = downloaded;
        let percent = ((downloaded_size as f64 / total_size as f64) * 100.0) as f64;

        // Only update display when percentage changes
        let downloaded_mb = downloaded_size / 1024 / 1024;
        let total_mb = total_size / 1024 / 1024;

        print!(
            "\r{} [{:50}] {:.2}%  ({:.2} MB / {:.2} MB)",
            console::style("Progress:").green(),
            "=".repeat((percent as usize) / 2),
            percent,
            downloaded_mb,
            total_mb
        );

        // Flush stdout to ensure progress is displayed immediately
        let _ = std::io::stdout().flush();
    }

    println!(); // New line after progress

    match download_task.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            println!("Error downloading build: {}", err);
            return Err(err);
        }
        Err(join_err) => {
            println!("Download task failed: {}", join_err);
            return Ok(false);
        }
    }

    if downloaded_size >= total_size as i64 {
        println!("Download complete!");
        Ok(true)
    } else {
        println!(
            "Download incomplete! ({}/{} bytes)",
            downloaded_size, total_size
        );
        Ok(false)
    }
}
