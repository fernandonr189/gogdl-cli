use std::collections::HashMap;

use anyhow::anyhow;
use gogdl_lib::auth::Auth;
use secret_service::{EncryptionType, SecretService};

pub async fn store_token(auth: &Auth) -> Result<(), anyhow::Error> {
    let ss = SecretService::connect(EncryptionType::Dh).await?;

    let collection = ss.get_default_collection().await?;
    let string = serde_json::to_string(auth)?;

    collection
        .create_item(
            "gogdl-cli auth token",
            HashMap::from([("app", "gogdl-cli")]),
            string.as_bytes(),
            true, // replace if exists
            "text/plain",
        )
        .await?;

    Ok(())
}

pub async fn recover_token() -> Result<Auth, anyhow::Error> {
    let ss = SecretService::connect(EncryptionType::Dh).await?;

    let search_items = ss
        .search_items(HashMap::from([("app", "gogdl-cli")]))
        .await?;

    let item = match search_items.unlocked.first() {
        Some(item) => item,
        None => return Err(anyhow!("Not found!")),
    };

    let secret = item.get_secret().await?;

    let auth_json = String::from_utf8(secret)?;

    let auth: Auth = serde_json::from_str(&auth_json)?;

    Ok(auth)
}
