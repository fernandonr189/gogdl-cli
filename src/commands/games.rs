use std::process::exit;

use gogdl_lib::{GogDl, GogdlError, client::ClientError};

use crate::secret;

pub async fn handle_games(gogdl: &GogDl) {
    let result = { list_games(gogdl).await };
    if let Err(err) = result {
        match err {
            GogdlError::ClientError(ClientError::Http { status, body }) => {
                if status.as_u16() == 401 {
                    // refresh token

                    let auth = match gogdl.refresh_token().await {
                        Ok(auth) => auth,
                        Err(_) => {
                            println!("Could not refresh auth, please login again");
                            exit(1)
                        }
                    };
                    match secret::store_token(&auth).await {
                        Ok(_) => println!("Token stored successfully"),
                        Err(err) => eprintln!("Error storing token: {}", err),
                    }

                    println!("Access token refreshed, please try again")
                } else {
                    println!("HttpError: Status: {}, Body: {}", status.as_u16(), body)
                }
            }
            _ => println!("{}", err),
        }
    }
}

pub async fn list_games(gogdl: &GogDl) -> Result<(), GogdlError> {
    match gogdl.get_owned_games().await {
        Ok(games) => {
            for game in games {
                println!("{} - {}", game.id, game.title);
            }
            Ok(())
        }
        Err(err) => return Err(err),
    }
}
