use std::process::exit;

use gogdl_lib::GogDl;

use crate::secret::store_token;

pub async fn handle_login(login: bool, code: Option<String>) {
    if login {
        if let Some(code) = code {
            let gogdl = GogDl::new(None);
            let token = match gogdl.get_login_tokens(&code).await {
                Ok(token) => token,
                Err(err) => {
                    eprintln!("Error: {}", err);
                    exit(1)
                }
            };

            match store_token(&token).await {
                Ok(_) => println!("Token stored successfully"),
                Err(err) => eprintln!("Error storing token: {}", err),
            }
        } else {
            eprintln!("Please enter a valid login code");
            exit(1);
        }
    } else {
        open::that(gogdl_lib::GogDl::get_login_url()).expect("Could not open browser")
    }
}
