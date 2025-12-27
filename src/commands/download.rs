use std::{fs::File, io::BufWriter, process::exit, sync::Arc};

use chrono::{DateTime, Utc};
use gogdl_lib::{GogDl, GogdlError, client::ClientError, games::GameBuild};

use crate::{secret, settings::AppSettings};

pub async fn handle_download(
    gogdl: GogDl,
    game_id: i32,
    version_id: Option<String>,
    path: &str,
    settings: &mut AppSettings,
    fix: bool,
) {
    let gogdl_clone = Arc::new(gogdl);
    let mut download_build = version_id.clone().unwrap_or_default();
    let result = {
        if let Some(version_id) = version_id {
            if settings
                .downloaded_games
                .iter()
                .find(|game| {
                    game.game_id == game_id
                        && game.build_id == version_id
                        && game.download_complete
                        && !fix
                })
                .is_some()
            {
                println!("Game already downloaded");
                return;
            }
            download_game(gogdl_clone.clone(), game_id, &version_id, path).await
        } else {
            let game_builds = match get_builds(gogdl_clone.as_ref(), game_id).await {
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
                if settings
                    .downloaded_games
                    .iter()
                    .find(|game| {
                        game.game_id == game_id
                            && game.build_id == download_build
                            && game.download_complete
                            && !fix
                    })
                    .is_some()
                {
                    println!("Game already downloaded");
                    return;
                }
                download_game(gogdl_clone.clone(), game_id, &download_build, path).await
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
                    // refresh token

                    let auth = match gogdl_clone.refresh_token().await {
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

                    println!("Access token refreshed, please try again")
                } else {
                    println!("HttpError: Status: {}, Body: {}", status.as_u16(), body)
                }
            }
            _ => println!("{}", err),
        }
    } else {
        let complete = result.unwrap_or(false);
        settings
            .add_game(&download_build, path, None, complete, game_id)
            .await;
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
    tokio::spawn(async move {
        match gogdl
            .download_build(game_id, &build_name_clone, tx, &path_clone)
            .await
        {
            Ok(_) => {}
            Err(err) => {
                println!("Error downloading build: {}", err);
                exit(1);
            }
        }
    });

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

    if downloaded_size == total_size as i64 {
        println!("\nDownload complete!");
        Ok(true)
    } else {
        println!("\nDownload incomplete!");
        Ok(false)
    }
}

pub async fn get_build_size(gogdl: &GogDl, game_id: i32, build_name: &str) -> u64 {
    match gogdl.get_build_chunks(game_id, build_name).await {
        Ok(build_chunks) => {
            let total_size: u64 = build_chunks.iter().map(|chunk| chunk.compressed_size).sum();
            println!("Total size: {} MB", total_size / 1024 / 1024);
            println!("Number of chunks: {}", build_chunks.len());
            return total_size;
        }
        Err(err) => match err {
            GogdlError::ClientError(ClientError::Http { status, body }) => {
                let _body = body;
                if status.as_u16() == 401 {
                    // refresh token

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
            }
            _ => println!("{}", err),
        },
    }
    return 0;
}
