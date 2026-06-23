use std::sync::Arc;

use console::style;
use dialoguer::{FuzzySelect, Input, theme::ColorfulTheme};
use gogdl_lib::{GogDl, GogdlError, client::ClientError, games::GameDetails};

use crate::{auth::manage_auth, commands::common, hint, settings::AppSettings};

/// Interactive games browser
pub async fn handle_games(gogdl: Arc<GogDl>, settings: &mut AppSettings) {
    let games = match gogdl.get_owned_games().await {
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
                    format!("{} [installed]", g.title)
                } else {
                    g.title.clone()
                }
            })
            .collect();

        let mut options = game_names.clone();
        options.push("← Back / Exit".to_string());

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
                handle_game_selection(selected_game, settings, gogdl.clone()).await;
            }
            Ok(Some(_)) | Ok(None) | Err(_) => {
                println!("{}", style("Goodbye!").green());
                break;
            }
        }
    }
}

pub async fn list_games_cli(gogdl: Arc<GogDl>) {
    let games = match gogdl.get_owned_games().await {
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

    println!();
    println!("{}", style("Your GOG Library:").bold());
    println!();

    for game in &games {
        println!("{} - {}", style(game.id).cyan(), game.title);
    }

    println!();
    println!("{}", style(format!("Total: {} game(s)", games.len())).dim());
}

async fn handle_game_selection(game: &GameDetails, settings: &mut AppSettings, gogdl: Arc<GogDl>) {
    let is_installed = settings
        .downloaded_games
        .iter()
        .any(|dg| dg.game_id == game.id);

    if is_installed {
        crate::commands::management::manage_game(settings, game.id, gogdl).await;
        return;
    }

    println!();
    println!("{}", style(format!("📦 {}", game.title)).bold());
    println!("{}", style(format!("ID: {}", game.id)).dim());
    println!();

    let options = vec!["Install", "Back"];

    let action = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("What would you like to do?")
        .items(&options)
        .default(0)
        .interact_opt();

    match action {
        Ok(Some(0)) => {
            install_game(game, settings, gogdl).await;
        }
        _ => {}
    }
}

async fn install_game(game: &GameDetails, settings: &mut AppSettings, gogdl: Arc<GogDl>) {
    println!();
    println!("{}", style(format!("Installing: {}", game.title)).cyan());

    let chosen_build = match common::select_build_interactive(gogdl.clone(), game.id, None).await
    {
        Some(build) => build,
        None => {
            println!("{}", style("Cancelled").dim());
            return;
        }
    };

    let default_path = format!("{}/games", settings.data_path);

    let custom_path: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Install path (leave empty for default)")
        .default(default_path.clone())
        .allow_empty(true)
        .interact_text()
        .unwrap_or(default_path.clone());

    let download_path = if custom_path.is_empty() {
        default_path.clone()
    } else if custom_path.starts_with('/') || custom_path.starts_with('~') {
        custom_path.clone()
    } else {
        let pwd = std::env::current_dir().unwrap_or_default();
        format!("{}/{}", pwd.display(), custom_path)
    };

    // Print CLI command hint
    let path_arg = if download_path == format!("{}/games", settings.data_path) {
        None
    } else {
        Some(download_path.as_str())
    };
    hint::print_command_hint(&hint::download_command(
        game.id,
        path_arg,
        Some(&chosen_build.version_name),
    ));

    println!(
        "{}",
        style(format!("📁 Downloading to: {}", download_path)).dim()
    );
    println!();

    // Store game info before calling download
    let game_id = game.id;
    let game_title = game.title.clone();

    match crate::commands::download::handle_download(
        gogdl,
        game_id,
        Some(chosen_build.version_name.clone()),
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

async fn handle_error(err: GogdlError, gogdl: Arc<GogDl>) {
    match err {
        GogdlError::ClientError(ClientError::Http { status, body }) => {
            if status.as_u16() == 401 {
                manage_auth(gogdl.clone()).await;
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
