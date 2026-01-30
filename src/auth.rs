use std::{process::exit, sync::Arc};

use console::style;
use gogdl_lib::{GogDl, client::ClientError};

use crate::secret;

pub async fn manage_auth(gogdl: Arc<GogDl>) {
    let auth = match secret::recover_token().await {
        Ok(auth) => Some(auth),
        Err(err) => {
            eprintln!(
                "{}",
                style(format!("Failed to recover token: {}", err)).red()
            );
            None
        }
    };
    gogdl.set_auth(auth).await;
    match gogdl.validate_auth().await {
        Ok(_) => {}
        Err(ClientError::TokenExpired) => {
            let auth = match gogdl.refresh_token().await {
                Ok(auth) => auth,
                Err(err) => {
                    println!(
                        "{}",
                        style(format!("Failed to refresh token: {}", err)).red()
                    );
                    println!(
                        "{}",
                        style("Please log in using the `login` command.").yellow()
                    );
                    exit(1);
                }
            };
            match secret::store_token(&auth).await {
                Ok(_) => {}
                Err(err) => println!("{}", style(format!("Failed to store token: {}", err)).red()),
            }
            gogdl.set_auth(Some(auth)).await;
        }
        Err(ClientError::NotLoggedIn) => {
            println!(
                "{}",
                style("You are not logged in. Please log in using the `login` command.").yellow()
            );
            exit(1);
        }
        _ => {}
    };
}
