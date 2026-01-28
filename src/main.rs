use std::{path::PathBuf, process::exit, sync::Arc};

use clap::Parser;
use gogdl_lib::GogDl;

use crate::{
    cli::ManageAction,
    commands::{
        games::handle_games, management::handle_manage, proton::handle_proton, runner::handle_run,
    },
    settings::AppSettings,
};

mod cli;
mod commands;
mod hint;
mod secret;
mod settings;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = cli::Args::parse();

    let mut settings = AppSettings::load().await?;

    match args.command {
        cli::Commands::Login { code, login_code } => {
            commands::login::handle_login(code, login_code).await;
        }
        cli::Commands::Download {
            game_id,
            version_id,
            path,
            fix,
        } => {
            let auth = match secret::recover_token().await {
                Ok(auth) => auth,
                Err(err) => {
                    eprintln!("Failed to recover token: {}, please login again", err);
                    exit(1);
                }
            };

            let download_path = format!(
                "{}/{}",
                match path {
                    Some(ref path) => {
                        let pwd = std::env::current_dir().unwrap_or_default();
                        format!("{}/{}", pwd.display(), path)
                    }
                    None => settings.data_path.clone(),
                },
                "games"
            );

            let gogdl = GogDl::new(Some(auth));

            commands::download::handle_download(
                gogdl,
                game_id,
                version_id,
                &download_path,
                &mut settings,
                fix,
            )
            .await?;
        }
        cli::Commands::Games { list } => {
            let auth = match secret::recover_token().await {
                Ok(auth) => auth,
                Err(err) => {
                    eprintln!("Failed to recover token: {}, please login again", err);
                    exit(1);
                }
            };
            let gogdl = GogDl::new(Some(auth));

            if list {
                // Direct CLI mode: just list games
                commands::games::list_games_cli(&gogdl).await;
            } else {
                // Interactive mode
                handle_games(&gogdl, &mut settings).await;
            }
        }
        cli::Commands::Proton {
            list,
            download,
            page,
            installed,
            remove,
        } => {
            // Check if any direct CLI flags are provided
            if list || download.is_some() || installed || remove.is_some() {
                // Direct CLI mode
                commands::proton::handle_proton_cli(
                    list,
                    download,
                    page,
                    installed,
                    remove,
                    &mut settings,
                )
                .await;
            } else {
                // Interactive mode
                handle_proton(&mut settings).await;
            }
        }
        cli::Commands::Manage { game_id, action } => {
            if let (Some(gid), Some(act)) = (game_id, action.clone()) {
                // Direct CLI mode
                match act {
                    ManageAction::DownloadSaveFiles => {
                        let auth = match secret::recover_token().await {
                            Ok(auth) => auth,
                            Err(err) => {
                                eprintln!("Failed to recover token: {}, please login again", err);
                                exit(1);
                            }
                        };
                        let gogdl = GogDl::new(Some(auth));
                        let gogdl_arc = Arc::new(gogdl);
                        let (client_id, client_secret) = gogdl_arc.get_auth_ids(1771973390).await?;
                        let remote_config = gogdl_arc.get_remote_config(&client_id).await?;

                        if remote_config.is_supported() {
                            let (mut mapped, rest) = remote_config.get_path()?;
                            println!("Remote config: {:?}", remote_config.get_path());

                            let game = if let Some(game) = settings
                                .downloaded_games
                                .iter()
                                .find(|game| game.game_id == 1771973390)
                            {
                                game
                            } else {
                                panic!("Game not found");
                            };

                            let mut parent_path = PathBuf::new();
                            if mapped == "INSTALLATION_PATH" {
                                mapped = game.path.clone();
                                parent_path.push(mapped);
                                parent_path.push(rest);
                            } else {
                                let prefix_path = if let Some(path) = &game.prefix_path {
                                    path.clone()
                                } else {
                                    panic!("Prefix path not found");
                                };
                                println!("path: {}", prefix_path);

                                parent_path.push(prefix_path);
                                parent_path.push("pfx");
                                parent_path.push("drive_c");
                                parent_path.push("users");
                                parent_path.push("steamuser");
                                parent_path.push(mapped);
                                parent_path.push(rest);
                            }

                            let saves = gogdl_arc
                                .get_save_file_list(&client_id, &client_secret)
                                .await?;
                            for save in saves {
                                println!("\n\nSave: {:?}", save);
                                let mut file_path = parent_path.clone();
                                file_path.push(save.get_path());

                                let client_id_clone = client_id.clone();
                                let client_secret_clone = client_secret.clone();
                                let save_clone = save.clone();
                                let gogdl_arc_clone = gogdl_arc.clone();
                                let (tx, mut rx) =
                                    tokio::sync::mpsc::unbounded_channel::<(i64, i64)>();
                                println!("File path: {:?}", file_path);
                                tokio::spawn(async move {
                                    let _ = gogdl_arc_clone
                                        .download_save_file(
                                            &save_clone,
                                            &client_id_clone,
                                            &client_secret_clone,
                                            tx,
                                            &file_path,
                                        )
                                        .await;
                                });

                                while let Some((downloaded, total)) = rx.recv().await {
                                    print!(
                                        "\rDownloaded: {} bytes / {} bytes -- {:.2}%",
                                        downloaded,
                                        total,
                                        (downloaded as f64 / total as f64) * 100.0
                                    );
                                }
                            }
                        } else {
                            println!("Cloud saves are not supported for this game!")
                        }
                    }
                    ManageAction::SetProton { version } => {
                        commands::management::set_proton_version(&mut settings, gid, &version)
                            .await;
                    }
                    ManageAction::SetExecutable { path } => {
                        commands::management::set_executable(&mut settings, gid, &path).await;
                    }
                    ManageAction::AddArg { arg } => {
                        commands::management::set_arg(&mut settings, gid, &arg).await;
                    }
                    ManageAction::ClearArgs => {
                        commands::management::clear_args_cli(&mut settings, gid).await;
                    }
                    ManageAction::AddEnv { key, value } => {
                        commands::management::set_env(&mut settings, gid, &key, &value).await;
                    }
                    ManageAction::ClearEnv => {
                        commands::management::clear_env_cli(&mut settings, gid).await;
                    }
                }
            } else if game_id.is_some() && action.is_none() {
                eprintln!("Error: game_id provided but no action specified.");
                eprintln!("Use --help to see available actions.");
                exit(1);
            } else {
                // Interactive mode
                handle_manage(&mut settings).await;
            }
        }
        cli::Commands::Run { game_id } => {
            if let Some(gid) = game_id {
                // Direct CLI mode
                commands::runner::run_game(&mut settings, gid).await;
            } else {
                // Interactive mode
                handle_run(&mut settings).await;
            }
        }
    }
    Ok(())
}
