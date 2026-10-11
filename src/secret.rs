//! Passwords/tokens in the Secret Service keyring. Uses the same schema as the
//! Python version (`com.jeffser.Nocturne.Password` + `type`), so existing logins survive.
use anyhow::Result;
use std::collections::HashMap;

const SCHEMA: &str = "com.jeffser.Nocturne.Password";

fn attrs(kind: &str) -> HashMap<&str, &str> {
    HashMap::from([("xdg:schema", SCHEMA), ("type", kind)])
}

/// `kind` is `"password"` for the server login, or e.g. `"listenbrainz"`.
pub async fn store(kind: &str, secret: &str) -> Result<()> {
    oo7::Keyring::new().await?.create_item("Nocturne Login", &attrs(kind), secret, true).await?;
    Ok(())
}

pub async fn lookup(kind: &str) -> Result<Option<String>> {
    let keyring = oo7::Keyring::new().await?;
    let items = keyring.search_items(&attrs(kind)).await?;
    match items.first() {
        Some(item) => Ok(Some(String::from_utf8_lossy(&item.secret().await?).into_owned())),
        None => Ok(None),
    }
}

pub async fn delete(kind: &str) -> Result<()> {
    oo7::Keyring::new().await?.delete(&attrs(kind)).await?;
    Ok(())
}
