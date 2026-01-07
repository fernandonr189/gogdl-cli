use std::{path::Path, process::exit};

use tokio::process::Command;

use crate::settings::AppSettings;

pub async fn run_game(settings: &mut AppSettings, game_id: i32) {
    let game = match settings
        .downloaded_games
        .iter_mut()
        .find(|game| game.game_id == game_id)
    {
        Some(game) => game,
        None => {
            eprintln!("Game not found");
            exit(1);
        }
    };

    let proton_version = match &game.proton_version {
        Some(proton_version) => proton_version,
        None => {
            eprintln!("Proton version not set");
            exit(1);
        }
    };

    if let None = game.prefix_path {
        let prefix_path = format!("{}/prefixes/{}", settings.data_path, game.game_id);

        let path = Path::new(&prefix_path);

        match tokio::fs::create_dir_all(path).await {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Failed to create prefix directory: {}", err);
                exit(1);
            }
        }

        let result = Command::new(format!("{}/proton", proton_version.path))
            .arg("run")
            .arg("wineboot")
            .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", "/tmp/steam")
            .env("STEAM_COMPAT_DATA_PATH", prefix_path.clone())
            .status()
            .await;

        match result {
            Ok(status) => {
                if !status.success() {
                    eprintln!("Failed to run wineboot");
                    exit(1);
                } else {
                    game.prefix_path = Some(prefix_path.clone());
                    let _ = settings.save().await;
                }
            }
            Err(err) => {
                eprintln!("Failed to run wineboot: {}", err);
                exit(1);
            }
        }
    } else {
        let executable = match game.executable.clone() {
            Some(ex) => ex,
            None => {
                println!("Executable not found");
                exit(1);
            }
        };

        let game_path = format!("{}/{}", game.path, &executable);
        let parent_path = Path::new(&game_path).parent();
        println!("Running {}", game_path);

        let mut command = Command::new(format!("{}/proton", proton_version.path));

        for (key, value) in &game.environment_variables {
            command.env(key, value);
        }

        command
            .arg("run")
            .arg(game_path.clone())
            .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", "/tmp/steam")
            .env("STEAM_COMPAT_DATA_PATH", game.prefix_path.clone().unwrap())
            .current_dir(parent_path.unwrap());

        for arg in &game.args {
            command.arg(arg);
        }

        println!("{:?}", command);
        let result = command.status().await;

        match result {
            Ok(status) => {
                if !status.success() {
                    eprintln!("Failed to run game");
                    exit(1);
                }
            }
            Err(err) => {
                eprintln!("Failed to run game: {}", err);
                exit(1);
            }
        }
    }
}
