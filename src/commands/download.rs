use std::{fs::File, io::BufWriter, process::exit, sync::Arc};

use chrono::{DateTime, Utc};
use gogdl_lib::{GogDl, GogdlError, client::ClientError, games::GameBuild};

use crate::{secret, settings::AppSettings};

/// Handle download from interactive games menu (creates new GogDl wrapped in Arc)
pub async fn handle_download_for_game(
    game_id: i32,
    version_id: Option<String>,
    path: &str,
    settings: &mut AppSettings,
    fix: bool,
    gogdl: Arc<GogDl>,
) -> Result<(), anyhow::Error> {
    handle_download_internal(gogdl, game_id, version_id, path, settings, fix).await
}

/// Handle download from CLI command (takes owned GogDl)
pub async fn handle_download(
    gogdl: Arc<GogDl>,
    game_id: i32,
    version_id: Option<String>,
    path: &str,
    settings: &mut AppSettings,
    fix: bool,
) -> Result<(), anyhow::Error> {
    handle_download_internal(gogdl, game_id, version_id, path, settings, fix).await
}

/// Internal download handler that uses Arc<GogDl> for proper progress reporting
async fn handle_download_internal(
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
            let game_builds = match get_builds(&gogdl, game_id).await {
                Ok(builds) => builds,
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

    if let Err(err) = result {
        match err {
            GogdlError::ClientError(ClientError::Http { status, body }) => {
                if status.as_u16() == 401 {
                    let auth = match gogdl.refresh_token().await {
                        Ok(auth) => auth,
                        Err(_) => {
                            println!("Could not refresh auth, please login again");
                            exit(1)
                        }
                    };
                    match secret::store_token(&auth).await {
                        Ok(_) => println!("Token stored successfully"),
                        Err(err) => eprintln!("Error storing token: {}", err),
                    }

                    println!("Access token refreshed, please try again");
                    Ok(())
                } else {
                    println!("HttpError: Status: {}, Body: {}", status.as_u16(), body);
                    Ok(())
                }
            }
            _ => {
                println!("{}", err);
                Ok(())
            }
        }
    } else {
        let complete = result.unwrap_or(false);
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
}

pub async fn get_builds(gogdl: &GogDl, game_id: i32) -> Result<Vec<GameBuild>, GogdlError> {
    match gogdl.get_game_builds(game_id).await {
        Ok(game_builds) => Ok(game_builds.items),
        Err(err) => Err(err),
    }
}

pub async fn download_game(
    gogdl: Arc<GogDl>,
    game_id: i32,
    build_name: &str,
    path: &str,
) -> Result<bool, GogdlError> {
    let total_size = get_build_size(&gogdl, game_id, build_name).await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<i64>();
    let build_name_clone = build_name.to_string();
    let path_clone = path.to_string();

    // Spawn the download task
    tokio::spawn(async move {
        match gogdl
            .download_build(game_id, &build_name_clone, tx, &path_clone)
            .await
        {
            Ok(_) => {}
            Err(err) => {
                println!("\nError downloading build: {}", err);
            }
        }
    });

    // Progress reporting loop
    let mut downloaded_size: i64 = 0;
    let mut last_percent = 0;
    use std::io::Write;

    while let Some(size) = rx.recv().await {
        downloaded_size += size;
        let percent = ((downloaded_size as f64 / total_size as f64) * 100.0) as i32;

        // Only update display when percentage changes
        if percent != last_percent {
            let downloaded_mb = downloaded_size / 1024 / 1024;
            let total_mb = total_size / 1024 / 1024;

            print!(
                "\r{} [{:50}] {:.1}%  ({} MB / {} MB)",
                console::style("Progress:").green(),
                "=".repeat((percent as usize) / 2),
                percent as f64,
                downloaded_mb,
                total_mb
            );

            // Flush stdout to ensure progress is displayed immediately
            let _ = std::io::stdout().flush();
            last_percent = percent;
        }
    }

    println!(); // New line after progress

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

pub async fn get_build_size(gogdl: &GogDl, game_id: i32, build_name: &str) -> u64 {
    match gogdl.get_build_chunks(game_id, build_name).await {
        Ok(build_chunks) => {
            let total_size: u64 = build_chunks.iter().map(|chunk| chunk.compressed_size).sum();
            println!("Total size: {} MB", total_size / 1024 / 1024);
            println!("Number of chunks: {}", build_chunks.len());
            total_size
        }
        Err(err) => match err {
            GogdlError::ClientError(ClientError::Http { status, body }) => {
                let _body = body;
                if status.as_u16() == 401 {
                    let auth = match gogdl.refresh_token().await {
                        Ok(auth) => auth,
                        Err(_) => {
                            println!("Could not refresh auth, please login again");
                            exit(1)
                        }
                    };
                    let file = File::create("auth.json").unwrap();
                    let writer = BufWriter::new(file);

                    serde_json::to_writer_pretty(writer, &auth).unwrap();

                    println!("Access token refreshed, please try again")
                }
                0
            }
            _ => {
                println!("{}", err);
                0
            }
        },
    }
}
