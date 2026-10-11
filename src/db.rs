//! Per-integration SQLite database (`$XDG_DATA_HOME/integrations/<kind>/database.db`).
use anyhow::Result;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::path::Path;

pub type Db = SqlitePool;

pub async fn open(path: &Path) -> Result<Db> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let opts = SqliteConnectOptions::new().filename(path).create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(4).connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn open_for(kind: &str) -> Result<Db> {
    open(&crate::config::integrations_dir().join(kind).join("database.db")).await
}

/// In-memory database (tests / fallback).
pub async fn memory() -> Result<Db> {
    let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn cached_image(db: &Db, model: &str, size: u32) -> Option<Vec<u8>> {
    sqlx::query_scalar("SELECT image FROM images WHERE model = ? AND size = ?")
        .bind(model)
        .bind(size)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
}

pub async fn save_image(db: &Db, model: &str, size: u32, image: &[u8]) -> Result<()> {
    sqlx::query("INSERT OR REPLACE INTO images (model, size, image) VALUES (?, ?, ?)")
        .bind(model)
        .bind(size)
        .bind(image)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn lyrics(db: &Db, id: &str) -> Option<(String, String)> {
    sqlx::query_as("SELECT type, content FROM lyrics WHERE id = ?").bind(id).fetch_optional(db).await.ok().flatten()
}

pub async fn save_lyrics(db: &Db, id: &str, kind: &str, content: &str) -> Result<()> {
    sqlx::query("INSERT OR REPLACE INTO lyrics (id, type, content) VALUES (?, ?, ?)")
        .bind(id)
        .bind(kind)
        .bind(content)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete_lyrics(db: &Db, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM lyrics WHERE id = ?").bind(id).execute(db).await?;
    Ok(())
}

pub async fn add_monthly_play(db: &Db, month: &str, song: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO playback_scrobble (month, song_id, amount) VALUES (?, ?, 1)
         ON CONFLICT(month, song_id) DO UPDATE SET amount = amount + 1",
    )
    .bind(month)
    .bind(song)
    .execute(db)
    .await?;
    Ok(())
}

/// Top songs of a month (`%m-%Y`) as `(song_id, plays)`.
pub async fn monthly_plays(db: &Db, month: &str, top: u32) -> Vec<(String, i64)> {
    sqlx::query_as("SELECT song_id, amount FROM playback_scrobble WHERE month = ? ORDER BY amount DESC LIMIT ?")
        .bind(month)
        .bind(top)
        .fetch_all(db)
        .await
        .unwrap_or_default()
}

pub async fn playlist_resume(db: &Db, origin: &str) -> Option<(String, f64)> {
    sqlx::query_as("SELECT song_id, timestamp FROM playlist_resume WHERE id = ?")
        .bind(origin)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
}

pub async fn save_playlist_resume(db: &Db, origin: &str, song: &str, timestamp: f64) -> Result<()> {
    sqlx::query("INSERT OR REPLACE INTO playlist_resume (id, song_id, timestamp) VALUES (?, ?, ?)")
        .bind(origin)
        .bind(song)
        .bind(timestamp)
        .execute(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn migrations_and_queries() {
        let db = super::memory().await.unwrap();
        super::add_monthly_play(&db, "01-2026", "a").await.unwrap();
        super::add_monthly_play(&db, "01-2026", "a").await.unwrap();
        assert_eq!(super::monthly_plays(&db, "01-2026", 5).await, vec![("a".to_string(), 2)]);
        super::save_lyrics(&db, "s", "plain", "la").await.unwrap();
        assert_eq!(super::lyrics(&db, "s").await.unwrap().1, "la");
    }
}
