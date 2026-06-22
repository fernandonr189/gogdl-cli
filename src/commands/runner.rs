use std::{path::Path, process::exit};

use console::style;
use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use tokio::process::Command;

use crate::commands::common;
use crate::hint;
use crate::settings::{AppSettings, DownloadedProtonVersion};

pub async fn handle_run(settings: &mut AppSettings) {
    if settings.downloaded_games.is_empty() {
        println!(
            "{}",
            style("No installed games found. Install a game first with 'gogdl games'").yellow()
        );
        return;
    }

    let game_names: Vec<String> = settings
        .downloaded_games
        .iter()
        .map(|g| {
            let name = g.path.split('/').last().unwrap_or("Unknown");
            let status = get_run_status_plain(g);
            format!("{} {}", name, status)
        })
        .collect();

    let mut options = game_names.clone();
    options.push("← Back / Exit".to_string());

    println!();
    println!("{}", style("🎮 Run a Game").bold().cyan());
    println!("{}", style("Select a game to launch").dim());
    println!();

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a game to run")
        .items(&options)
        .default(0)
        .interact_opt();

    match selection {
        Ok(Some(idx)) if idx < settings.downloaded_games.len() => {
            let game_id = settings.downloaded_games[idx].game_id;

            // Print CLI command hint
            hint::print_command_hint(&hint::run_command(game_id));

            run_game(settings, game_id).await;
        }
        Ok(Some(_)) | Ok(None) | Err(_) => {
            println!("{}", style("Goodbye!").green());
        }
    }
}

/// Plain text status for use in dialoguer menus (no ANSI codes)
fn get_run_status_plain(game: &crate::settings::DownloadedGame) -> String {
    let proton_ok = game.proton_version.is_some();
    let exe_ok = game.executable.is_some();

    if proton_ok && exe_ok {
        "[ready]".to_string()
    } else {
        let mut missing = Vec::new();
        if !proton_ok {
            missing.push("proton");
        }
        if !exe_ok {
            missing.push("exe");
        }
        format!("[needs: {}]", missing.join(", "))
    }
}

pub async fn run_game(settings: &mut AppSettings, game_id: i32) {
    // First, check if we need to configure proton
    let needs_proton = settings
        .downloaded_games
        .iter()
        .find(|g| g.game_id == game_id)
        .map(|g| g.proton_version.is_none())
        .unwrap_or(true);

    if needs_proton {
        println!();
        println!(
            "{}",
            style("No Proton version configured for this game.").yellow()
        );

        if settings.downloaded_proton_versions.is_empty() {
            println!();
            println!(
                "{}",
                style("No Proton versions installed. Use 'gogdl proton -d <version>' to download one first.").red()
            );
            return;
        }

        let proton_version = select_proton_version(settings).await;
        if let Some(version) = proton_version {
            // Set the proton version on the game
            if let Some(game) = settings
                .downloaded_games
                .iter_mut()
                .find(|g| g.game_id == game_id)
            {
                game.proton_version = Some(version.clone());
                let _ = settings.save().await;
                println!(
                    "{}",
                    style(format!("✅ Proton version set to {}", version.version)).green()
                );
            }
        } else {
            println!("{}", style("Cancelled").dim());
            return;
        }
    }

    // Check if we need to configure executable
    let (needs_executable, game_path_for_scan) = {
        let game = settings
            .downloaded_games
            .iter()
            .find(|g| g.game_id == game_id);
        match game {
            Some(g) => (g.executable.is_none(), g.path.clone()),
            None => {
                eprintln!("{}", style("Game not found").red());
                exit(1);
            }
        }
    };

    if needs_executable {
        println!();
        println!(
            "{}",
            style("No executable configured for this game.").yellow()
        );

        let executable = select_executable(&game_path_for_scan).await;

        if let Some(exe) = executable {
            if let Some(game) = settings
                .downloaded_games
                .iter_mut()
                .find(|g| g.game_id == game_id)
            {
                game.executable = Some(exe.clone());
                let _ = settings.save().await;
                println!("{}", style(format!("✅ Executable set to {}", exe)).green());
            }
        } else {
            println!("{}", style("Cancelled").dim());
            return;
        }
    }

    // Extract all needed data before mutable operations
    let (proton_path, prefix_path_opt, game_path, executable, env_vars, args, data_path) = {
        let game = match settings
            .downloaded_games
            .iter()
            .find(|g| g.game_id == game_id)
        {
            Some(g) => g,
            None => {
                eprintln!("{}", style("Game not found").red());
                exit(1);
            }
        };

        let proton_version = match &game.proton_version {
            Some(pv) => pv,
            None => {
                eprintln!("{}", style("Proton version not set").red());
                exit(1);
            }
        };

        let executable = match &game.executable {
            Some(ex) => ex.clone(),
            None => {
                eprintln!("{}", style("Executable not found").red());
                exit(1);
            }
        };

        (
            proton_version.path.clone(),
            game.prefix_path.clone(),
            game.path.clone(),
            executable,
            game.environment_variables.clone(),
            game.args.clone(),
            settings.data_path.clone(),
        )
    };

    // Shared, persistent Steam-compat client install path. Proton does not
    // need a real Steam install here -- it only needs a writable directory
    // to exist, and this is shared across all games (matching how Lutris
    // and Heroic configure non-Steam Proton launches), unlike the per-game
    // STEAM_COMPAT_DATA_PATH (prefix_path) below.
    let steam_compat_client_path = format!("{}/steam", data_path);
    if let Err(err) = tokio::fs::create_dir_all(&steam_compat_client_path).await {
        eprintln!(
            "{}",
            style(format!(
                "Failed to create Steam compat client directory: {}",
                err
            ))
            .red()
        );
        exit(1);
    }

    // Handle prefix creation if needed
    let prefix_path = if prefix_path_opt.is_none() {
        let new_prefix_path = format!("{}/prefixes/{}", data_path, game_id);

        let path = Path::new(&new_prefix_path);

        match tokio::fs::create_dir_all(path).await {
            Ok(_) => {}
            Err(err) => {
                eprintln!(
                    "{}",
                    style(format!("Failed to create prefix directory: {}", err)).red()
                );
                exit(1);
            }
        }

        println!();
        println!("{}", style("Setting up Wine prefix (first run)...").cyan());

        let result = Command::new(format!("{}/proton", proton_path))
            .arg("run")
            .arg("wineboot")
            .env(
                "STEAM_COMPAT_CLIENT_INSTALL_PATH",
                &steam_compat_client_path,
            )
            .env("STEAM_COMPAT_DATA_PATH", &new_prefix_path)
            .status()
            .await;

        match result {
            Ok(status) => {
                if !status.success() {
                    eprintln!("{}", style("Failed to run wineboot").red());
                    exit(1);
                } else {
                    // Update the game with the new prefix path
                    if let Some(game) = settings
                        .downloaded_games
                        .iter_mut()
                        .find(|g| g.game_id == game_id)
                    {
                        game.prefix_path = Some(new_prefix_path.clone());
                        let _ = settings.save().await;
                    }
                    println!("{}", style("✅ Wine prefix created").green());
                    new_prefix_path
                }
            }
            Err(err) => {
                eprintln!(
                    "{}",
                    style(format!("Failed to run wineboot: {}", err)).red()
                );
                exit(1);
            }
        }
    } else {
        prefix_path_opt.unwrap()
    };

    // Now run the game
    let full_game_path = format!("{}/{}", game_path, &executable);
    let parent_path = Path::new(&full_game_path).parent();

    println!();
    println!("{}", style(format!("🚀 Launching {}", executable)).green());
    println!("{}", style(format!("Path: {}", full_game_path)).dim());
    println!();

    let mut command = Command::new(format!("{}/proton", proton_path));

    for (key, value) in &env_vars {
        command.env(key, value);
    }

    command
        .arg("run")
        .arg(&full_game_path)
        .env(
            "STEAM_COMPAT_CLIENT_INSTALL_PATH",
            &steam_compat_client_path,
        )
        .env("STEAM_COMPAT_DATA_PATH", &prefix_path)
        .current_dir(parent_path.unwrap());

    for arg in &args {
        command.arg(arg);
    }

    let result = command.status().await;

    match result {
        Ok(status) => {
            if !status.success() {
                eprintln!("{}", style("Game exited with error").yellow());
            } else {
                println!("{}", style("Game exited successfully").green());
            }
        }
        Err(err) => {
            eprintln!("{}", style(format!("Failed to run game: {}", err)).red());
            exit(1);
        }
    }
}

async fn select_proton_version(settings: &AppSettings) -> Option<DownloadedProtonVersion> {
    let version_names: Vec<&str> = settings
        .downloaded_proton_versions
        .iter()
        .map(|v| v.version.as_str())
        .collect();

    let mut options: Vec<&str> = version_names.clone();
    options.push("← Cancel");

    println!();
    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a Proton version")
        .items(&options)
        .default(0)
        .interact_opt();

    match selection {
        Ok(Some(idx)) if idx < settings.downloaded_proton_versions.len() => {
            Some(settings.downloaded_proton_versions[idx].clone())
        }
        _ => None,
    }
}

async fn select_executable(game_path: &str) -> Option<String> {
    println!("{}", style("Scanning for executables...").dim());

    let executables = common::find_executables(game_path).await;

    if executables.is_empty() {
        println!(
            "{}",
            style("No executable files found automatically.").yellow()
        );

        let manual: String = dialoguer::Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter executable path manually (relative to game folder)")
            .allow_empty(true)
            .interact_text()
            .unwrap_or_default();

        if manual.is_empty() {
            return None;
        }
        return Some(manual);
    }

    println!(
        "{}",
        style(format!("Found {} executable(s)", executables.len())).dim()
    );

    let mut options: Vec<String> = executables.clone();
    options.push("Enter path manually".to_string());
    options.push("← Cancel".to_string());

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select the game executable")
        .items(&options)
        .default(0)
        .interact_opt();

    match selection {
        Ok(Some(idx)) if idx < executables.len() => Some(executables[idx].clone()),
        Ok(Some(idx)) if idx == executables.len() => {
            let manual: String = dialoguer::Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter executable path manually (relative to game folder)")
                .allow_empty(true)
                .interact_text()
                .unwrap_or_default();

            if manual.is_empty() {
                None
            } else {
                Some(manual)
            }
        }
        _ => None,
    }
}
