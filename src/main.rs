use std::process::exit;

use clap::Parser;
use gogdl_lib::GogDl;

mod cli;
mod commands;
mod secret;
mod settings;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = cli::Args::parse();

    match args.command {
        cli::Commands::Login { code, login_code } => {
            commands::login::handle_login(code, login_code).await;
        }
        cli::Commands::Download {
            game_id,
            version_id,
            path,
        } => {
            let auth = match secret::recover_token().await {
                Ok(auth) => auth,
                Err(err) => {
                    eprintln!("Failed to recover token: {}, please login again", err);
                    exit(1);
                }
            };
            let gogdl = GogDl::new(Some(auth));
            commands::download::handle_download(gogdl, game_id, version_id, &path).await
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
        } => commands::proton::handle_proton(list, download, page).await?,
    }
    Ok(())
}
