use std::process::exit;

use console::style;
use dialoguer::{FuzzySelect, Input, theme::ColorfulTheme};
use gogdl_lib::{GogDl, GogdlError, client::ClientError, games::GameDetails};

use crate::{secret, settings::AppSettings};

pub async fn handle_games(gogdl: &GogDl, settings: &mut AppSettings) {
    let games = match fetch_games(gogdl).await {
        Ok(games) => games,
        Err(err) => {
            handle_error(err, gogdl).await;
            return;
        }
    };

    if games.is_empty() {
        println!("{}", style("No games found in your library.").yellow());
        return;
    }

    loop {
        let game_names: Vec<String> = games
            .iter()
            .map(|g| {
                let installed = settings
                    .downloaded_games
                    .iter()
                    .any(|dg| dg.game_id == g.id);
                if installed {
                    format!("{} {}", g.title, style("[installed]").green())
                } else {
                    g.title.clone()
                }
            })
            .collect();

        let mut options = game_names.clone();
        options.push(style("← Back / Exit").dim().to_string());

        println!();
        println!("{}", style("🎮 Your GOG Library").bold().cyan());
        println!(
            "{}",
            style("Use arrow keys to navigate, type to search").dim()
        );
        println!();

        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a game")
            .items(&options)
            .default(0)
            .interact_opt();

        match selection {
            Ok(Some(idx)) if idx < games.len() => {
                let selected_game = &games[idx];
                handle_game_selection(selected_game, settings).await;
            }
            Ok(Some(_)) | Ok(None) | Err(_) => {
                println!("{}", style("Goodbye!").green());
                break;
            }
        }
    }
}

async fn handle_game_selection(game: &GameDetails, settings: &mut AppSettings) {
    let is_installed = settings
        .downloaded_games
        .iter()
        .any(|dg| dg.game_id == game.id);

    println!();
    println!("{}", style(format!("📦 {}", game.title)).bold());
    println!("{}", style(format!("ID: {}", game.id)).dim());
    println!();

    let options = if is_installed {
        vec!["Reinstall / Update", "Back"]
    } else {
        vec!["Install", "Back"]
    };

    let action = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("What would you like to do?")
        .items(&options)
        .default(0)
        .interact_opt();

    match action {
        Ok(Some(0)) => {
            install_game(game, settings).await;
        }
        _ => {}
    }
}

async fn install_game(game: &GameDetails, settings: &mut AppSettings) {
    println!();
    println!("{}", style(format!("Installing: {}", game.title)).cyan());

    let default_path = format!("{}/games", settings.data_path);

    let custom_path: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Install path (leave empty for default)")
        .default(default_path.clone())
        .allow_empty(true)
        .interact_text()
        .unwrap_or(default_path.clone());

    let download_path = if custom_path.is_empty() {
        default_path
    } else if custom_path.starts_with('/') || custom_path.starts_with('~') {
        custom_path
    } else {
        let pwd = std::env::current_dir().unwrap_or_default();
        format!("{}/{}", pwd.display(), custom_path)
    };

    println!();
    println!(
        "{}",
        style(format!("📁 Downloading to: {}", download_path)).dim()
    );
    println!();

    // Store game info before calling download
    let game_id = game.id;
    let game_title = game.title.clone();

    match crate::commands::download::handle_download_for_game(
        game_id,
        None,
        &download_path,
        settings,
        false,
    )
    .await
    {
        Ok(_) => {
            println!();
            println!(
                "{}",
                style(format!("✅ {} installed successfully!", game_title)).green()
            );
        }
        Err(err) => {
            println!();
            println!("{}", style(format!("❌ Failed to install: {}", err)).red());
        }
    }
}

async fn fetch_games(gogdl: &GogDl) -> Result<Vec<GameDetails>, GogdlError> {
    gogdl.get_owned_games().await
}

async fn handle_error(err: GogdlError, gogdl: &GogDl) {
    match err {
        GogdlError::ClientError(ClientError::Http { status, body }) => {
            if status.as_u16() == 401 {
                let auth = match gogdl.refresh_token().await {
                    Ok(auth) => auth,
                    Err(_) => {
                        println!(
                            "{}",
                            style("Could not refresh auth, please login again").red()
                        );
                        exit(1);
                    }
                };
                match secret::store_token(&auth).await {
                    Ok(_) => println!("{}", style("Token refreshed successfully").green()),
                    Err(err) => {
                        eprintln!("{}", style(format!("Error storing token: {}", err)).red())
                    }
                }
                println!("{}", style("Please try again").yellow());
            } else {
                println!(
                    "{}",
                    style(format!(
                        "HTTP Error: Status {}, Body: {}",
                        status.as_u16(),
                        body
                    ))
                    .red()
                );
            }
        }
        _ => println!("{}", style(format!("Error: {}", err)).red()),
    }
}
