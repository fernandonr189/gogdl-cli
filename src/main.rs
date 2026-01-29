use std::{process::exit, sync::Arc};

use clap::Parser;
use gogdl_lib::GogDl;

use crate::{
    cli::ManageAction,
    commands::{
        games::handle_games,
        management::{download_save_files_cli, handle_manage, upload_save_files_cli},
        proton::handle_proton,
        runner::handle_run,
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

    let gogdl = Arc::new(GogDl::new(None));

    match args.command {
        cli::Commands::Login { code, login_code } => {
            commands::login::handle_login(code, login_code, gogdl).await;
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

            gogdl.set_auth(Some(auth)).await;

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
            gogdl.set_auth(Some(auth)).await;

            if list {
                // Direct CLI mode: just list games
                commands::games::list_games_cli(gogdl).await;
            } else {
                // Interactive mode
                handle_games(gogdl, &mut settings).await;
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
                    ManageAction::UploadSaveFiles => {
                        let auth = match secret::recover_token().await {
                            Ok(auth) => auth,
                            Err(err) => {
                                eprintln!("Failed to recover token: {}, please login again", err);
                                exit(1);
                            }
                        };
                        gogdl.set_auth(Some(auth)).await;
                        if let Err(e) = upload_save_files_cli(&mut settings, gid, gogdl).await {
                            println!("Error uploading save files: {}", e);
                            exit(1);
                        }
                    }
                    ManageAction::DownloadSaveFiles => {
                        let auth = match secret::recover_token().await {
                            Ok(auth) => auth,
                            Err(err) => {
                                eprintln!("Failed to recover token: {}, please login again", err);
                                exit(1);
                            }
                        };
                        gogdl.set_auth(Some(auth)).await;
                        if let Err(e) = download_save_files_cli(&mut settings, gid, gogdl).await {
                            eprintln!("Error downloading save files: {}", e);
                            exit(1);
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
                handle_manage(&mut settings, gogdl).await;
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
