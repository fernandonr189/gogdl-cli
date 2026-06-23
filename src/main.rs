use std::{process::exit, sync::Arc};

use clap::Parser;
use gogdl_lib::GogDl;

use crate::{
    auth::manage_auth,
    cli::ManageAction,
    commands::{
        games::handle_games,
        management::{
            download_save_files_for_game, handle_manage, update_cli, upload_save_files_for_game,
            verify_download_cli,
        },
        proton::handle_proton,
        runner::handle_run,
    },
    settings::AppSettings,
};

mod auth;
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
            list_builds,
        } => {
            manage_auth(gogdl.clone()).await;

            if list_builds {
                commands::download::list_builds_cli(gogdl, game_id).await?;
                return Ok(());
            }

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
            manage_auth(gogdl.clone()).await;
            if list {
                commands::games::list_games_cli(gogdl).await;
            } else {
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
            if list || download.is_some() || installed || remove.is_some() {
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
                handle_proton(&mut settings).await;
            }
        }
        cli::Commands::Manage { game_id, action } => {
            if let (Some(gid), Some(act)) = (game_id, action.clone()) {
                match act {
                    ManageAction::UploadSaveFiles => {
                        manage_auth(gogdl.clone()).await;
                        if let Err(e) = upload_save_files_for_game(&mut settings, gid, gogdl).await
                        {
                            println!("Error uploading save files: {}", e);
                            exit(1);
                        }
                    }
                    ManageAction::DownloadSaveFiles => {
                        manage_auth(gogdl.clone()).await;
                        if let Err(e) =
                            download_save_files_for_game(&mut settings, gid, gogdl).await
                        {
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
                    ManageAction::VerifyDownload => {
                        manage_auth(gogdl.clone()).await;
                        if let Err(e) = verify_download_cli(&mut settings, gid, gogdl).await {
                            eprintln!("Error verifying download: {}", e);
                            exit(1);
                        }
                    }
                    ManageAction::Update { version } => {
                        manage_auth(gogdl.clone()).await;
                        if let Err(e) = update_cli(&mut settings, gid, gogdl, version).await {
                            eprintln!("Error updating game: {}", e);
                            exit(1);
                        }
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
                commands::runner::run_game(&mut settings, gid).await;
            } else {
                // Interactive mode
                handle_run(&mut settings).await;
            }
        }
    }
    Ok(())
}
