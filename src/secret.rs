//! Passwords/tokens in the Secret Service keyring, with a private-file fallback
//! (`$XDG_CONFIG_HOME/<kind>.secret`, mode 0600) when no keyring is available.
use anyhow::Result;
use std::{collections::HashMap, path::PathBuf};

const SCHEMA: &str = "dev.arjix.Nocturne.Password";

fn attrs(kind: &str) -> HashMap<&str, &str> {
    HashMap::from([("xdg:schema", SCHEMA), ("type", kind)])
}

fn fallback_path(kind: &str) -> PathBuf {
    gtk::glib::user_config_dir().join(format!("nocturne-{kind}.secret"))
}

async fn keyring_store(kind: &str, secret: &str) -> Result<()> {
    oo7::Keyring::new().await?.create_item("Nocturne Login", &attrs(kind), secret, true).await?;
    Ok(())
}

async fn keyring_lookup(kind: &str) -> Result<Option<String>> {
    let keyring = oo7::Keyring::new().await?;
    let items = keyring.search_items(&attrs(kind)).await?;
    match items.first() {
        Some(item) => Ok(Some(String::from_utf8_lossy(&item.secret().await?).into_owned())),
        None => Ok(None),
    }
}

/// `kind` is `"password"` for the server login, or e.g. `"listenbrainz"`.
pub async fn store(kind: &str, secret: &str) -> Result<()> {
    let (kind, secret) = (kind.to_string(), secret.to_string());
    crate::runtime::run(async move { store_inner(&kind, &secret).await }).await
}

async fn store_inner(kind: &str, secret: &str) -> Result<()> {
    if let Err(e) = keyring_store(kind, secret).await {
        tracing::warn!("keyring unavailable ({e:#}), storing {kind} in a private file");
        let path = fallback_path(kind);
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        tokio::fs::write(&path, secret).await?;
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?;
    }
    Ok(())
}

pub async fn lookup(kind: &str) -> Result<Option<String>> {
    let kind = kind.to_string();
    crate::runtime::run(async move { lookup_inner(&kind).await }).await
}

async fn lookup_inner(kind: &str) -> Result<Option<String>> {
    match keyring_lookup(kind).await {
        Ok(Some(s)) => Ok(Some(s)),
        _ => Ok(tokio::fs::read_to_string(fallback_path(kind)).await.ok()),
    }
}

pub async fn delete(kind: &str) -> Result<()> {
    let kind = kind.to_string();
    crate::runtime::run(async move { delete_inner(&kind).await }).await
}

async fn delete_inner(kind: &str) -> Result<()> {
    let _ = tokio::fs::remove_file(fallback_path(kind)).await;
    if let Ok(k) = oo7::Keyring::new().await {
        k.delete(&attrs(kind)).await?;
    }
    Ok(())
}
