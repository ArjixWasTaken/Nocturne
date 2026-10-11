//! Server/backends. Everything here is `Send` plain data that runs on tokio;
//! [`crate::integration`] turns it into GObject models on the main thread.
use crate::models::Ref;
use anyhow::{Result, bail};
use async_trait::async_trait;

pub mod jellyfin;
pub mod local;
pub mod subsonic;

#[derive(Clone, Debug, Default)]
pub struct SongData {
    pub id: String,
    pub title: String,
    pub album: String,
    pub album_id: String,
    pub artist: String,
    pub artist_id: String,
    pub artists: Vec<Ref>,
    pub duration: i32,
    pub track: i32,
    pub disc_number: i32,
    pub year: i32,
    pub starred: bool,
    pub user_rating: i32,
    pub cover_art: String,
    pub album_gain: f64,
    pub track_gain: f64,
    pub radio_stream_url: String,
    pub container: String,
    pub codec: String,
    pub path: String,
}

#[derive(Clone, Debug, Default)]
pub struct AlbumData {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub artist_id: String,
    pub artists: Vec<Ref>,
    pub song_count: i32,
    pub duration: i32,
    pub year: i32,
    pub starred: bool,
    pub user_rating: i32,
    pub cover_art: String,
    /// `Some` when the full track list was fetched.
    pub songs: Option<Vec<SongData>>,
}

#[derive(Clone, Debug, Default)]
pub struct ArtistData {
    pub id: String,
    pub name: String,
    pub album_count: i32,
    pub starred: bool,
    pub user_rating: i32,
    pub cover_art: String,
    pub biography: String,
    pub similar: Vec<Ref>,
    /// `Some` when the album list was fetched.
    pub albums: Option<Vec<AlbumData>>,
}

#[derive(Clone, Debug, Default)]
pub struct PlaylistData {
    pub id: String,
    pub name: String,
    pub song_count: i32,
    pub duration: i32,
    pub readonly: bool,
    pub cover_art: String,
    /// `Some` when the entries were fetched.
    pub entries: Option<Vec<SongData>>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchResults {
    pub artists: Vec<ArtistData>,
    pub albums: Vec<AlbumData>,
    pub songs: Vec<SongData>,
    pub playlists: Vec<PlaylistData>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SearchCounts {
    pub artist: (u32, u32),
    pub album: (u32, u32),
    pub song: (u32, u32),
    pub playlist: (u32, u32),
}

#[derive(Clone, Debug, Default)]
pub struct Starred {
    pub artists: Vec<ArtistData>,
    pub albums: Vec<AlbumData>,
    pub songs: Vec<SongData>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlbumListKind {
    Random,
    Newest,
    Frequent,
    Recent,
    Starred,
    AlphabeticalByName,
}

impl AlbumListKind {
    pub fn from_name(s: &str) -> Self {
        match s {
            "random" => Self::Random,
            "newest" => Self::Newest,
            "frequent" => Self::Frequent,
            "starred" => Self::Starred,
            "alphabeticalByName" | "all" => Self::AlphabeticalByName,
            _ => Self::Recent,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricsKind {
    Plain,
    Synced,
}

#[derive(Clone, Debug)]
pub struct Lyrics {
    pub kind: LyricsKind,
    /// Plain text, or LRC for synced lyrics.
    pub content: String,
}

#[derive(Clone, Debug, Default)]
pub struct ServerInfo {
    pub username: String,
    pub title: String,
    pub link: String,
    pub avatar: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Default)]
pub struct PlayQueue {
    pub current: Option<String>,
    pub ids: Vec<String>,
    pub songs: Vec<SongData>,
    pub position_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackReport {
    Play,
    Pause,
    Progress,
    Stop,
}

/// Credentials/config used to construct a backend.
#[derive(Clone, Debug, Default)]
pub struct Connection {
    pub url: String,
    pub user: String,
    pub password: String,
    pub trust_server: bool,
    pub library_dir: String,
    pub library_ids: Vec<String>,
}

fn unsupported<T>() -> Result<T> {
    bail!("not supported by this backend")
}

/// A music source. Only the core methods are mandatory; the rest default to
/// "unsupported" / empty so backends stay small.
#[async_trait]
pub trait Backend: Send + Sync {
    /// GType-style identifier stored in `selected-instance-type`.
    fn kind(&self) -> &'static str;

    /// Check connectivity / credentials.
    async fn ping(&self) -> Result<()>;

    async fn album_list(&self, kind: AlbumListKind, size: u32, offset: u32) -> Result<Vec<AlbumData>>;
    async fn artists(&self, size: u32) -> Result<Vec<ArtistData>>;
    async fn playlists(&self) -> Result<Vec<PlaylistData>>;
    async fn starred(&self) -> Result<Starred>;

    /// Full metadata (including albums).
    async fn artist(&self, id: &str) -> Result<ArtistData>;
    /// Full metadata (including songs).
    async fn album(&self, id: &str) -> Result<AlbumData>;
    /// Full metadata (including entries).
    async fn playlist(&self, id: &str) -> Result<PlaylistData>;
    async fn song(&self, id: &str) -> Result<SongData>;

    async fn search(&self, query: &str, counts: SearchCounts) -> Result<SearchResults>;

    /// URI playable by GStreamer.
    fn stream_url(&self, song: &str, max_bitrate: i32) -> String;
    async fn cover_art(&self, id: &str, size: u32) -> Result<Option<Vec<u8>>>;
    /// Public URL to the cover, for MPRIS. Empty if unavailable.
    fn cover_art_url(&self, _id: &str) -> String {
        String::new()
    }

    async fn set_starred(&self, _id: &str, _starred: bool) -> Result<()> {
        unsupported()
    }
    async fn set_rating(&self, _id: &str, _rating: i32) -> Result<()> {
        unsupported()
    }
    async fn scrobble(&self, _id: &str, _submission: bool) -> Result<()> {
        Ok(())
    }
    async fn playback_report(&self, _song: &str, _report: PlaybackReport, _position_s: f64) -> Result<()> {
        Ok(())
    }

    async fn random_songs(&self, _size: u32) -> Result<Vec<SongData>> {
        Ok(vec![])
    }
    async fn similar_songs(&self, _id: &str, count: u32) -> Result<Vec<SongData>> {
        self.random_songs(count).await
    }
    async fn top_songs(&self, _artist_id: &str, _count: u32) -> Result<Vec<SongData>> {
        Ok(vec![])
    }

    async fn play_queue(&self) -> Result<PlayQueue> {
        Ok(PlayQueue::default())
    }
    async fn save_play_queue(&self, _ids: &[String], _current: &str, _position_ms: i64) -> Result<()> {
        Ok(())
    }

    /// Server-provided lyrics, if any.
    async fn lyrics(&self, _id: &str) -> Result<Option<Lyrics>> {
        Ok(None)
    }

    async fn radios(&self) -> Result<Vec<SongData>> {
        Ok(vec![])
    }
    async fn create_radio(&self, _name: &str, _url: &str) -> Result<()> {
        unsupported()
    }
    async fn update_radio(&self, _id: &str, _name: &str, _url: &str) -> Result<()> {
        unsupported()
    }
    async fn delete_radio(&self, _id: &str) -> Result<()> {
        unsupported()
    }

    /// Returns the new playlist id.
    async fn create_playlist(&self, _name: &str, _songs: &[String]) -> Result<String> {
        unsupported()
    }
    async fn update_playlist(&self, _id: &str, _name: Option<&str>, _add: &[String], _remove_indexes: &[u32]) -> Result<()> {
        unsupported()
    }
    async fn delete_playlist(&self, _id: &str) -> Result<()> {
        unsupported()
    }

    /// `(multiple_selection_allowed, libraries)`
    async fn libraries(&self) -> Result<(bool, Vec<Ref>)> {
        Ok((false, vec![]))
    }

    /// Label/value pairs for the song details dialog.
    async fn song_details(&self, _id: &str) -> Result<Vec<(String, String)>> {
        Ok(vec![])
    }

    async fn server_info(&self) -> Result<ServerInfo> {
        Ok(ServerInfo::default())
    }

    /// Download the original file to `dest`.
    async fn download(&self, _id: &str, _dest: &std::path::Path) -> Result<()> {
        unsupported()
    }
}

/// Construct a backend by its kind name.
pub fn create(kind: &str, conn: Connection) -> Result<std::sync::Arc<dyn Backend>> {
    Ok(match kind {
        subsonic::KIND => std::sync::Arc::new(subsonic::Subsonic::new(conn)?),
        jellyfin::KIND => std::sync::Arc::new(jellyfin::Jellyfin::new(conn)?),
        local::KIND => std::sync::Arc::new(local::Local::new(conn)?),
        other => bail!("unknown backend {other}"),
    })
}

/// Shared HTTP client builder honoring "trust server".
pub fn http_client(trust_server: bool) -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("Nocturne/", env!("CARGO_PKG_VERSION")))
        .danger_accept_invalid_certs(trust_server)
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}
