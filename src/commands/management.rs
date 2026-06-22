use std::{path::PathBuf, process::exit, sync::Arc};

use anyhow::Result;
use console::style;
use dialoguer::{FuzzySelect, Input, theme::ColorfulTheme};
use gogdl_lib::GogDl;
use walkdir::WalkDir;

use crate::auth::manage_auth;
use crate::commands::common;
use crate::hint;
use crate::settings::{AppSettings, DownloadedGame, DownloadedProtonVersion};

pub async fn handle_manage(settings: &mut AppSettings, gogdl: Arc<GogDl>) {
    if settings.downloaded_games.is_empty() {
        println!(
            "{}",
            style("No installed games found. Install a game first!").yellow()
        );
        return;
    }

    loop {
        let game_names: Vec<String> = settings
            .downloaded_games
            .iter()
            .map(|g| {
                let status = get_game_status(g);
                format!(
                    "{} {}",
                    g.path.split('/').last().unwrap_or("Unknown"),
                    status
                )
            })
            .collect();

        let mut options = game_names.clone();
        options.push("← Back / Exit".to_string());

        println!();
        println!("{}", style("⚙️  Game Management").bold().cyan());
        println!("{}", style("Select a game to configure").dim());
        println!();

        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a game to manage")
            .items(&options)
            .default(0)
            .interact_opt();

        match selection {
            Ok(Some(idx)) if idx < settings.downloaded_games.len() => {
                let game_id = settings.downloaded_games[idx].game_id;

                // Show CLI hint
                hint::print_command_hint(&format!("gogdl manage -g {}", game_id));

                manage_game(settings, game_id, gogdl.clone()).await;
            }
            Ok(Some(_)) | Ok(None) | Err(_) => {
                println!("{}", style("Goodbye!").green());
                break;
            }
        }
    }
}

fn get_game_status(game: &DownloadedGame) -> String {
    let mut status_parts = Vec::new();

    if game.proton_version.is_some() {
        status_parts.push("proton".to_string());
    } else {
        status_parts.push("no proton".to_string());
    }

    if game.executable.is_some() {
        status_parts.push("exe".to_string());
    } else {
        status_parts.push("no exe".to_string());
    }

    format!("[{}]", status_parts.join(", "))
}

async fn manage_game(settings: &mut AppSettings, game_id: i32, gogdl: Arc<GogDl>) {
    loop {
        let game = match settings
            .downloaded_games
            .iter()
            .find(|g| g.game_id == game_id)
        {
            Some(g) => g,
            None => {
                println!("{}", style("Game not found").red());
                return;
            }
        };

        println!();
        println!(
            "{}",
            style(format!(
                "📦 {}",
                game.path.split('/').last().unwrap_or("Unknown")
            ))
            .bold()
        );
        println!("{}", style(format!("Path: {}", game.path)).dim());
        println!(
            "{}",
            style(format!(
                "Proton: {}",
                game.proton_version
                    .as_ref()
                    .map(|p| p.version.as_str())
                    .unwrap_or("Not set")
            ))
            .dim()
        );
        println!(
            "{}",
            style(format!(
                "Executable: {}",
                game.executable.as_deref().unwrap_or("Not set")
            ))
            .dim()
        );
        println!("{}", style(format!("Args: {:?}", game.args)).dim());
        println!(
            "{}",
            style(format!("Env vars: {:?}", game.environment_variables)).dim()
        );
        println!();

        let options = vec![
            "Set Proton version",
            "Set executable",
            "Add launch argument",
            "Clear launch arguments",
            "Add environment variable",
            "Clear environment variables",
            "Download cloud saves",
            "Upload cloud saves",
            "← Back",
        ];

        let action = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select action")
            .items(&options)
            .default(0)
            .interact_opt();

        match action {
            Ok(Some(0)) => set_proton_interactive(settings, game_id).await,
            Ok(Some(1)) => set_executable_interactive(settings, game_id).await,
            Ok(Some(2)) => add_arg_interactive(settings, game_id).await,
            Ok(Some(3)) => clear_args(settings, game_id).await,
            Ok(Some(4)) => add_env_interactive(settings, game_id).await,
            Ok(Some(5)) => clear_env_vars(settings, game_id).await,
            Ok(Some(6)) => {
                manage_auth(gogdl.clone()).await;
                if let Err(e) = download_save_files_for_game(settings, game_id, gogdl.clone()).await
                {
                    println!("{}", style(format!("Error: {}", e)).red());
                }
            }
            Ok(Some(7)) => {
                manage_auth(gogdl.clone()).await;
                if let Err(e) = upload_save_files_for_game(settings, game_id, gogdl.clone()).await {
                    println!("{}", style(format!("Error: {}", e)).red());
                }
            }
            _ => break,
        }
    }
}

async fn set_proton_interactive(settings: &mut AppSettings, game_id: i32) {
    if settings.downloaded_proton_versions.is_empty() {
        println!();
        println!(
            "{}",
            style(
                "No Proton versions installed. Use 'gogdl proton -l' to list and download versions."
            )
            .yellow()
        );
        return;
    }

    let version_names: Vec<&str> = settings
        .downloaded_proton_versions
        .iter()
        .map(|v| v.version.as_str())
        .collect();

    let mut options: Vec<&str> = version_names.clone();
    options.push("<- Cancel");

    println!();
    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select Proton version")
        .items(&options)
        .default(0)
        .interact_opt();

    match selection {
        Ok(Some(idx)) if idx < settings.downloaded_proton_versions.len() => {
            let version_name = settings.downloaded_proton_versions[idx].version.clone();

            // Print CLI command hint
            hint::print_command_hint(&hint::manage_set_proton_command(game_id, &version_name));

            set_proton_version(settings, game_id, &version_name).await;
            println!(
                "{}",
                style(format!("✅ Proton version set to {}", version_name)).green()
            );
        }
        _ => {
            println!("{}", style("Cancelled").dim());
        }
    }
}

async fn set_executable_interactive(settings: &mut AppSettings, game_id: i32) {
    let game = match settings
        .downloaded_games
        .iter()
        .find(|g| g.game_id == game_id)
    {
        Some(g) => g,
        None => {
            println!("{}", style("Game not found").red());
            return;
        }
    };

    let game_path = game.path.clone();

    // Find executable files in the game directory
    let executables = common::find_executables(&game_path).await;

    if executables.is_empty() {
        println!();
        println!(
            "{}",
            style("No executable files found. Enter path manually:").yellow()
        );

        let manual_path: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Executable path (relative to game folder)")
            .interact_text()
            .unwrap_or_default();

        if !manual_path.is_empty() {
            set_executable(settings, game_id, &manual_path).await;
        }
        return;
    }

    let mut options: Vec<String> = executables.clone();
    options.push("Enter path manually".to_string());
    options.push("<- Cancel".to_string());

    println!();
    println!(
        "{}",
        style(format!("Found {} executable(s)", executables.len())).dim()
    );

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select executable")
        .items(&options)
        .default(0)
        .interact_opt();

    match selection {
        Ok(Some(idx)) if idx < executables.len() => {
            let selected_exe = &executables[idx];

            // Print CLI command hint
            hint::print_command_hint(&hint::manage_set_executable_command(game_id, selected_exe));

            set_executable(settings, game_id, selected_exe).await;
            println!(
                "{}",
                style(format!("✅ Executable set to {}", selected_exe)).green()
            );
        }
        Ok(Some(idx)) if idx == executables.len() => {
            let manual_path: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Executable path (relative to game folder)")
                .interact_text()
                .unwrap_or_default();

            if !manual_path.is_empty() {
                // Print CLI command hint
                hint::print_command_hint(&hint::manage_set_executable_command(
                    game_id,
                    &manual_path,
                ));

                set_executable(settings, game_id, &manual_path).await;
                println!(
                    "{}",
                    style(format!("✅ Executable set to {}", manual_path)).green()
                );
            }
        }
        _ => {
            println!("{}", style("Cancelled").dim());
        }
    }
}

async fn add_arg_interactive(settings: &mut AppSettings, game_id: i32) {
    println!();
    let arg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter launch argument (without leading dash)")
        .interact_text()
        .unwrap_or_default();

    if !arg.is_empty() {
        // Print CLI command hint
        hint::print_command_hint(&hint::manage_add_arg_command(game_id, &arg));

        set_arg(settings, game_id, &arg).await;
        println!("{}", style(format!("✅ Added argument: -{}", arg)).green());
    } else {
        println!("{}", style("Cancelled").dim());
    }
}

async fn clear_args(settings: &mut AppSettings, game_id: i32) {
    // Print CLI command hint
    hint::print_command_hint(&hint::manage_clear_args_command(game_id));

    let game = match settings
        .downloaded_games
        .iter_mut()
        .find(|g| g.game_id == game_id)
    {
        Some(g) => g,
        None => {
            println!("{}", style("Game not found").red());
            return;
        }
    };

    game.args.clear();
    let _ = settings.save().await;
    println!("{}", style("✅ Launch arguments cleared").green());
}

/// CLI mode: clear all launch arguments
pub async fn clear_args_cli(settings: &mut AppSettings, game_id: i32) {
    let game = match settings
        .downloaded_games
        .iter_mut()
        .find(|g| g.game_id == game_id)
    {
        Some(g) => g,
        None => {
            println!("{}", style("Game not found").red());
            return;
        }
    };

    game.args.clear();
    let _ = settings.save().await;
    println!("{}", style("✅ Launch arguments cleared").green());
}

async fn add_env_interactive(settings: &mut AppSettings, game_id: i32) {
    println!();
    let key: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Environment variable name")
        .interact_text()
        .unwrap_or_default();

    if key.is_empty() {
        println!("{}", style("Cancelled").dim());
        return;
    }

    let value: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Environment variable value")
        .interact_text()
        .unwrap_or_default();

    // Print CLI command hint
    hint::print_command_hint(&hint::manage_add_env_command(game_id, &key, &value));

    set_env(settings, game_id, &key, &value).await;
    println!(
        "{}",
        style(format!("✅ Added environment variable: {}={}", key, value)).green()
    );
}

async fn clear_env_vars(settings: &mut AppSettings, game_id: i32) {
    // Print CLI command hint
    hint::print_command_hint(&hint::manage_clear_env_command(game_id));

    let game = match settings
        .downloaded_games
        .iter_mut()
        .find(|g| g.game_id == game_id)
    {
        Some(g) => g,
        None => {
            println!("{}", style("Game not found").red());
            return;
        }
    };

    game.environment_variables.clear();
    let _ = settings.save().await;
    println!("{}", style("✅ Environment variables cleared").green());
}

/// CLI mode: clear all environment variables
pub async fn clear_env_cli(settings: &mut AppSettings, game_id: i32) {
    let game = match settings
        .downloaded_games
        .iter_mut()
        .find(|g| g.game_id == game_id)
    {
        Some(g) => g,
        None => {
            println!("{}", style("Game not found").red());
            return;
        }
    };

    game.environment_variables.clear();
    let _ = settings.save().await;
    println!("{}", style("✅ Environment variables cleared").green());
}

// Original functions kept for compatibility
pub async fn set_proton_version(settings: &mut AppSettings, game_id: i32, proton_version: &str) {
    let proton_path = match settings
        .downloaded_proton_versions
        .iter()
        .find(|&version| version.version == proton_version)
    {
        Some(version) => version.path.clone(),
        None => {
            println!("{}", style("Proton version not found").red());
            exit(1);
        }
    };

    let game = match settings
        .downloaded_games
        .iter_mut()
        .find(|game| game.game_id == game_id)
    {
        Some(game) => game,
        None => {
            println!("{}", style("Game not found").red());
            exit(1);
        }
    };

    game.proton_version = Some(DownloadedProtonVersion {
        version: proton_version.to_string(),
        path: proton_path,
    });
    let _ = settings.save().await;
}

pub async fn set_executable(settings: &mut AppSettings, game_id: i32, executable_path: &str) {
    let game = match settings
        .downloaded_games
        .iter_mut()
        .find(|game| game.game_id == game_id)
    {
        Some(game) => game,
        None => {
            println!("{}", style("Game not found").red());
            exit(1);
        }
    };

    let full_path = format!("{}/{}", game.path, executable_path);

    let _file = match tokio::fs::File::open(&full_path).await {
        Ok(file) => file,
        Err(_) => {
            println!(
                "{}",
                style(format!("File does not exist: {}", full_path)).red()
            );
            exit(1);
        }
    };

    game.executable = Some(executable_path.to_owned());
    let _ = settings.save().await;
}

pub async fn set_arg(settings: &mut AppSettings, game_id: i32, arg: &str) {
    let game = match settings
        .downloaded_games
        .iter_mut()
        .find(|game| game.game_id == game_id)
    {
        Some(game) => game,
        None => {
            println!("{}", style("Game not found").red());
            exit(1);
        }
    };

    let new_arg = format!("-{}", arg);

    game.args.push(new_arg);
    let _ = settings.save().await;
}

pub async fn upload_save_files_for_game(
    settings: &mut AppSettings,
    game_id: i32,
    gogdl: Arc<GogDl>,
) -> Result<()> {
    let game = match settings
        .downloaded_games
        .iter()
        .find(|g| g.game_id == game_id)
    {
        Some(game) => game,
        None => {
            return Err(anyhow::anyhow!("Game not found in downloaded games"));
        }
    };

    println!("{}", style("Getting game authentication details...").blue());
    let (client_id, client_secret) = gogdl.get_auth_ids(game_id).await?;

    println!("{}", style("Checking cloud save support...").blue());
    let remote_config = gogdl.get_remote_config(&client_id).await?;

    if !remote_config.is_supported() {
        println!(
            "{}",
            style("Cloud saves are not supported for this game!").yellow()
        );
        return Ok(());
    }

    println!("{}", style("Determining save file location...").blue());

    let (mut mapped, rest) = remote_config.get_path()?;

    let mut parent_path = PathBuf::new();
    if mapped == "INSTALLATION_PATH" {
        mapped = game.path.clone();
        parent_path.push(mapped);
        parent_path.push(rest);
    } else {
        let prefix_path = if let Some(path) = &game.prefix_path {
            path.clone()
        } else {
            return Err(anyhow::anyhow!("Game prefix path not found"));
        };

        parent_path.push(prefix_path);
        parent_path.push("pfx");
        parent_path.push("drive_c");
        parent_path.push("users");
        parent_path.push("steamuser");
        parent_path.push(mapped);
        parent_path.push(rest);
    }

    println!(
        "{}",
        style(format!("Saves path: {}", parent_path.display())).blue()
    );

    let files_to_upload: Vec<_> = WalkDir::new(parent_path.clone())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    if files_to_upload.is_empty() {
        println!(
            "{}",
            style("No save files found in the directory!").yellow()
        );
        return Ok(());
    }

    println!(
        "{}",
        style(format!("Found {} files to upload", files_to_upload.len())).blue()
    );

    for (i, file_entry) in files_to_upload.iter().enumerate() {
        let full_path = file_entry.path();

        let relative_path = match full_path.strip_prefix(&parent_path) {
            Ok(path) => path,
            Err(_) => {
                println!(
                    "{}",
                    style(format!("Failed to get relative path for {:?}", full_path)).red()
                );
                continue;
            }
        };

        let url_path = relative_path
            .components()
            .map(|c: std::path::Component| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        println!(
            "{}",
            style(format!(
                "Uploading file {}/{}: {}",
                i + 1,
                files_to_upload.len(),
                url_path
            ))
            .blue()
        );

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(i64, i64)>();

        let client_id_clone = client_id.clone();
        let client_secret_clone = client_secret.clone();
        let full_path_clone = full_path.to_path_buf();
        let gogdl_clone = gogdl.clone();

        let upload_task = tokio::spawn(async move {
            gogdl_clone
                .upload_save_file(
                    &client_id_clone,
                    &client_secret_clone,
                    tx,
                    &full_path_clone,
                    &url_path,
                )
                .await
        });

        while let Some((sent, total)) = rx.recv().await {
            let percent = if total > 0 {
                ((sent as f64 / total as f64) * 100.0) as f32
            } else {
                0.0
            };

            let sent_str = format_bytes(sent);
            let total_str = format_bytes(total);

            print!(
                "\r{} [{:50}] {:.1}%  ({} / {})",
                style("Progress:").green(),
                "=".repeat((percent as usize) / 2),
                percent as f64,
                sent_str,
                total_str
            );
        }
        println!(); // New line after progress bar

        match upload_task.await {
            Ok(result) => match result {
                Ok(_) => println!("{}", style("✓ Save file uploaded successfully").green()),
                Err(e) => println!(
                    "{}",
                    style(format!("× Failed to upload save file: {}", e)).red()
                ),
            },
            Err(e) => println!("{}", style(format!("× Upload task failed: {}", e)).red()),
        }
    }

    println!("{}", style("All save files uploaded successfully!").green());
    Ok(())
}

pub async fn download_save_files_for_game(
    settings: &mut AppSettings,
    game_id: i32,
    gogdl: Arc<GogDl>,
) -> Result<()> {
    let game = match settings
        .downloaded_games
        .iter()
        .find(|g| g.game_id == game_id)
    {
        Some(game) => game,
        None => {
            return Err(anyhow::anyhow!("Game not found in downloaded games"));
        }
    };

    println!("{}", style("Connecting to GOG...").blue());

    println!("{}", style("Getting game authentication details...").blue());
    let (client_id, client_secret) = gogdl.get_auth_ids(game_id).await?;

    println!("{}", style("Checking cloud save support...").blue());
    let remote_config = gogdl.get_remote_config(&client_id).await?;

    if !remote_config.is_supported() {
        println!(
            "{}",
            style("Cloud saves are not supported for this game!").yellow()
        );
        return Ok(());
    }

    println!("{}", style("Determining save file location...").blue());
    let (mut mapped, rest) = remote_config.get_path()?;

    let mut parent_path = PathBuf::new();
    if mapped == "INSTALLATION_PATH" {
        mapped = game.path.clone();
        parent_path.push(mapped);
        parent_path.push(rest);
    } else {
        let prefix_path = if let Some(path) = &game.prefix_path {
            path.clone()
        } else {
            return Err(anyhow::anyhow!("Game prefix path not found"));
        };

        parent_path.push(prefix_path);
        parent_path.push("pfx");
        parent_path.push("drive_c");
        parent_path.push("users");
        parent_path.push("steamuser");
        parent_path.push(mapped);
        parent_path.push(rest);
    }

    tokio::fs::create_dir_all(&parent_path).await?;

    println!(
        "{}",
        style(format!(
            "Save files will be stored in: {}",
            parent_path.display()
        ))
        .dim()
    );

    println!("{}", style("Fetching available save files...").blue());
    let saves = gogdl.get_save_file_list(&client_id, &client_secret).await?;

    if saves.is_empty() {
        println!(
            "{}",
            style("No save files found in the cloud for this game.").yellow()
        );
        return Ok(());
    }

    println!(
        "{}",
        style(format!("Found {} save file(s)", saves.len())).green()
    );

    for (i, save) in saves.iter().enumerate() {
        println!(
            "{}",
            style(format!(
                "Downloading save file {}/{}: {}",
                i + 1,
                saves.len(),
                save.get_path()
            ))
            .blue()
        );

        let mut file_path = parent_path.clone();
        file_path.push(save.get_path());

        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let client_id_clone = client_id.clone();
        let client_secret_clone = client_secret.clone();
        let save_clone = save.clone();
        let gogdl_clone = gogdl.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(i64, i64)>();

        let download_task = tokio::spawn(async move {
            gogdl_clone
                .download_save_file(
                    &save_clone,
                    &client_id_clone,
                    &client_secret_clone,
                    tx,
                    &file_path,
                )
                .await
        });

        let mut last_percent = 0;
        while let Some((downloaded, total)) = rx.recv().await {
            let percent = ((downloaded as f64 / total as f64) * 100.0) as i32;

            if percent != last_percent {
                let downloaded_str = format_bytes(downloaded);
                let total_str = format_bytes(total);

                print!(
                    "\r{} [{:50}] {:.1}%  ({} / {})",
                    style("Progress:").green(),
                    "=".repeat((percent as usize) / 2),
                    percent as f64,
                    downloaded_str,
                    total_str
                );
                last_percent = percent;
            }
        }
        println!(); // New line after progress bar

        match download_task.await {
            Ok(result) => match result {
                Ok(_) => println!("{}", style("✓ Save file downloaded successfully").green()),
                Err(e) => println!(
                    "{}",
                    style(format!("× Failed to download save file: {}", e)).red()
                ),
            },
            Err(e) => println!("{}", style(format!("× Download task failed: {}", e)).red()),
        }
    }

    println!(
        "{}",
        style("All save files downloaded successfully!").green()
    );
    Ok(())
}

fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub async fn set_env(settings: &mut AppSettings, game_id: i32, key: &str, value: &str) {
    let game = match settings
        .downloaded_games
        .iter_mut()
        .find(|game| game.game_id == game_id)
    {
        Some(game) => game,
        None => {
            println!("{}", style("Game not found").red());
            exit(1);
        }
    };

    game.environment_variables
        .push((key.to_owned(), value.to_owned()));
    let _ = settings.save().await;
}
