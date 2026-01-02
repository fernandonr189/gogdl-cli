use std::process::exit;

use crate::settings::AppSettings;

pub async fn set_proton_version(settings: &mut AppSettings, game_id: i32, proton_version: &str) {
    match settings
        .downloaded_proton_versions
        .iter()
        .find(|&version| version.version == proton_version)
    {
        Some(_version) => {}
        None => {
            println!("Proton version not found");
            exit(1);
        }
    }

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

    game.proton_version = Some(proton_version.to_string());
    let _ = settings.save().await;
}
