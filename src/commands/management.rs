use std::process::exit;

use crate::settings::{AppSettings, DownloadedProtonVersion};

pub async fn set_proton_version(settings: &mut AppSettings, game_id: i32, proton_version: &str) {
    let proton_path = match settings
        .downloaded_proton_versions
        .iter()
        .find(|&version| version.version == proton_version)
    {
        Some(version) => version.path.clone(),
        None => {
            println!("Proton version not found");
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
            println!("Game not found");
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
            println!("Game not found");
            exit(1);
        }
    };

    let full_path = format!("{}/{}", game.path, executable_path);

    let _file = match tokio::fs::File::open(&full_path).await {
        Ok(file) => file,
        Err(_) => {
            println!("File does not exist");
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
            println!("Game not found");
            exit(1);
        }
    };

    let new_arg = format!("-{}", arg);

    game.args.push(new_arg);
    let _ = settings.save().await;
}

pub async fn set_env(settings: &mut AppSettings, game_id: i32, key: &str, value: &str) {
    let game = match settings
        .downloaded_games
        .iter_mut()
        .find(|game| game.game_id == game_id)
    {
        Some(game) => game,
        None => {
            println!("Game not found");
            exit(1);
        }
    };

    game.environment_variables
        .push((key.to_owned(), value.to_owned()));
    let _ = settings.save().await;
}
