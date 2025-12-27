use std::process::exit;

use clap::Parser;
use gogdl_lib::GogDl;

use crate::settings::AppSettings;

mod cli;
mod commands;
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
                    Some(path) => {
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
            .await
        }
        cli::Commands::Games => {
            let auth = match secret::recover_token().await {
                Ok(auth) => auth,
                Err(err) => {
                    eprintln!("Failed to recover token: {}, please login again", err);
                    exit(1);
                }
            };
            let gogdl = GogDl::new(Some(auth));
            commands::games::handle_games(&gogdl).await;
        }
        cli::Commands::Proton {
            list,
            download,
            page,
        } => commands::proton::handle_proton(list, download, page, &mut settings).await?,
    }
    Ok(())
}
