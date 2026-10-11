//! Jellyfin backend.
use super::*;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;

pub const KIND: &str = "NocturneIntegrationJellyfin";
const TICKS: f64 = 10_000_000.0;

type Ratings = HashMap<String, i32>;
type Params = Vec<(&'static str, String)>;

#[derive(Default)]
struct Auth {
    token: String,
    user_id: String,
    user_name: String,
    library: String,
}

#[derive(Default)]
struct PlaySession {
    item: String,
    session: String,
    last_ping: f64,
}

pub struct Jellyfin {
    conn: Connection,
    http: reqwest::Client,
    auth: Mutex<Auth>,
    /// song id -> (container, codec), filled while parsing, used by `stream_url`.
    formats: Mutex<HashMap<String, (String, String)>>,
    deleted_radios: Mutex<Vec<String>>,
    play: tokio::sync::Mutex<PlaySession>,
    db: tokio::sync::OnceCell<crate::db::Db>,
}

// ---- wire types ----

#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "PascalCase")]
struct WRef {
    id: String,
    name: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "PascalCase")]
struct WStream {
    #[serde(rename = "Type")]
    kind: String,
    codec: String,
    bit_depth: i32,
    sample_rate: i32,
    channels: i32,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "PascalCase")]
struct WSource {
    media_streams: Vec<WStream>,
    size: i64,
    bitrate: i64,
    container: String,
    path: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "PascalCase")]
struct WUserData {
    is_favorite: bool,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "PascalCase")]
struct WItem {
    id: String,
    name: String,
    album: String,
    album_id: String,
    album_artist: String,
    album_primary_image_tag: Option<String>,
    artist_items: Vec<WRef>,
    artists: Vec<String>,
    run_time_ticks: i64,
    index_number: Option<i32>,
    parent_index_number: Option<i32>,
    production_year: i32,
    user_data: WUserData,
    image_tags: HashMap<String, String>,
    normalization_gain: Option<f64>,
    album_normalization_gain: Option<f64>,
    container: String,
    media_sources: Vec<WSource>,
    overview: String,
    child_count: i32,
    collection_type: String,
    path: String,
    genres: Vec<String>,
    provider_ids: HashMap<String, String>,
    playlist_item_id: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "PascalCase")]
struct WItems {
    items: Vec<WItem>,
    total_record_count: i32,
}

fn items(v: Value) -> WItems {
    serde_json::from_value(v).unwrap_or_default()
}

fn refs(v: &[WRef]) -> Vec<Ref> {
    v.iter().map(|a| Ref { id: a.id.clone(), name: a.name.clone() }).collect()
}

impl WItem {
    fn cover(&self) -> String {
        match self.image_tags.get("Primary") {
            Some(t) => format!("Items/{}/Images/Primary?={t}", self.id),
            None => "None".into(),
        }
    }
    fn secs(&self) -> i32 {
        (self.run_time_ticks as f64 / TICKS) as i32
    }
    fn codec(&self) -> String {
        self.media_sources
            .first()
            .and_then(|s| s.media_streams.iter().rfind(|s| s.kind == "Audio"))
            .map(|s| s.codec.to_lowercase())
            .unwrap_or_default()
    }
}

fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname").map(|s| s.trim().to_string()).unwrap_or_else(|_| "Linux".into())
}

fn device_id() -> String {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "nocturne".into())
}

fn auth_header(token: &str) -> String {
    let mut h = format!(
        r#"MediaBrowser Client="Nocturne", Device="{}", DeviceId="{}", Version="{}""#,
        hostname(),
        device_id(),
        env!("CARGO_PKG_VERSION")
    );
    if !token.is_empty() {
        h.push_str(&format!(r#", Token="{token}""#));
    }
    h
}

async fn send(http: &reqwest::Client, method: Method, url: String, token: &str, query: &[(&str, String)], body: Option<Value>) -> Result<reqwest::Response> {
    let mut rb = http.request(method, url).header("Authorization", auth_header(token)).header("Accept", "application/json").query(query);
    if let Some(b) = body {
        rb = rb.json(&b);
    }
    Ok(rb.send().await?.error_for_status()?)
}

async fn to_json(r: reqwest::Response) -> Result<Value> {
    let bytes = r.bytes().await?;
    Ok(if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes)? })
}

/// QuickConnect step 1: returns `(secret, code)`. Show `code` to the user, who
/// approves it in another Jellyfin client.
pub async fn quick_connect_initiate(url: &str, trust_server: bool) -> Result<(String, String)> {
    let http = http_client(trust_server)?;
    let v = to_json(send(&http, Method::POST, format!("{}/QuickConnect/Initiate", url.trim_end_matches('/')), "", &[], None).await?).await?;
    Ok((v["Secret"].as_str().unwrap_or_default().into(), v["Code"].as_str().unwrap_or_default().into()))
}

/// QuickConnect step 2 (poll): true once approved. On success, log in with
/// the secret as `Connection::password` (the user can be empty).
pub async fn quick_connect_check(url: &str, trust_server: bool, secret: &str) -> Result<bool> {
    let http = http_client(trust_server)?;
    let r = send(&http, Method::GET, format!("{}/QuickConnect/Connect", url.trim_end_matches('/')), "", &[("secret", secret.into())], None).await?;
    Ok(to_json(r).await?["Authenticated"].as_bool().unwrap_or(false))
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

impl Jellyfin {
    pub fn new(conn: Connection) -> Result<Self> {
        let library = conn.library_ids.first().filter(|l| *l != "All").cloned().unwrap_or_default();
        Ok(Self {
            http: http_client(conn.trust_server)?,
            conn,
            auth: Mutex::new(Auth { library, ..Default::default() }),
            formats: Default::default(),
            deleted_radios: Default::default(),
            play: Default::default(),
            db: Default::default(),
        })
    }

    fn base(&self) -> String {
        self.conn.url.trim_end_matches('/').to_string()
    }
    fn token(&self) -> String {
        self.auth.lock().unwrap().token.clone()
    }
    fn uid(&self) -> String {
        self.auth.lock().unwrap().user_id.clone()
    }
    fn lib(&self) -> String {
        self.auth.lock().unwrap().library.clone()
    }

    async fn raw(&self, method: Method, path: &str, query: &[(&str, String)], body: Option<Value>) -> Result<reqwest::Response> {
        send(&self.http, method, format!("{}/{}", self.base(), path.trim_start_matches('/')), &self.token(), query, body).await
    }

    async fn req(&self, method: Method, path: &str, query: &[(&str, String)], body: Option<Value>) -> Result<Value> {
        to_json(self.raw(method, path, query, body).await?).await
    }

    async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value> {
        self.req(Method::GET, path, query, None).await
    }

    async fn user_items(&self, query: Params) -> Result<WItems> {
        Ok(items(self.get(&format!("Users/{}/Items", self.uid()), &query).await?))
    }

    async fn item(&self, id: &str, fields: &str) -> Result<WItem> {
        let v = self.get(&format!("Users/{}/Items/{id}", self.uid()), &[("Fields", fields.into())]).await?;
        let it: WItem = serde_json::from_value(v)?;
        if it.id.is_empty() {
            bail!("item {id} not found");
        }
        Ok(it)
    }

    async fn db(&self) -> Result<&crate::db::Db> {
        self.db.get_or_try_init(|| crate::db::open_for(KIND)).await
    }

    async fn ratings(&self) -> Ratings {
        let Ok(db) = self.db().await else { return Ratings::new() };
        sqlx::query_as::<_, (String, i32)>("SELECT id, rating FROM ratings").fetch_all(db).await.unwrap_or_default().into_iter().collect()
    }

    fn song(&self, it: WItem, r: &Ratings) -> SongData {
        let codec = it.codec();
        self.formats.lock().unwrap().insert(it.id.clone(), (it.container.clone(), codec.clone()));
        let cover = match (it.image_tags.get("Primary"), &it.album_primary_image_tag) {
            (Some(_), _) => it.cover(),
            (None, Some(t)) if !it.album_id.is_empty() => format!("Items/{}/Images/Primary?={t}", it.album_id),
            _ if !it.album_id.is_empty() => format!("Items/{}/Images/Primary", it.album_id),
            _ => "None".into(),
        };
        SongData {
            title: it.name.clone(),
            album: it.album.clone(),
            album_id: it.album_id.clone(),
            artist: it.album_artist.clone(),
            artist_id: it.artist_items.first().map(|a| a.id.clone()).unwrap_or_default(),
            artists: refs(&it.artist_items),
            duration: it.secs(),
            track: it.index_number.unwrap_or(0),
            disc_number: it.parent_index_number.unwrap_or(0),
            year: it.production_year,
            starred: it.user_data.is_favorite,
            user_rating: r.get(&it.id).copied().unwrap_or(0),
            cover_art: cover,
            album_gain: it.album_normalization_gain.or(it.normalization_gain).unwrap_or(0.0),
            track_gain: it.normalization_gain.unwrap_or(0.0),
            container: it.container.clone(),
            codec,
            path: it.path.clone(),
            id: it.id,
            ..Default::default()
        }
    }

    fn songs(&self, v: Vec<WItem>, r: &Ratings) -> Vec<SongData> {
        v.into_iter().map(|s| self.song(s, r)).collect()
    }

    fn album_data(it: &WItem, r: &Ratings) -> AlbumData {
        AlbumData {
            id: it.id.clone(),
            name: it.name.clone(),
            artist: it.album_artist.clone(),
            artist_id: it.artist_items.first().map(|a| a.id.clone()).unwrap_or_default(),
            artists: refs(&it.artist_items),
            song_count: it.child_count,
            duration: it.secs(),
            year: it.production_year,
            starred: it.user_data.is_favorite,
            user_rating: r.get(&it.id).copied().unwrap_or(0),
            cover_art: it.cover(),
            songs: None,
        }
    }

    fn artist_data(it: &WItem, r: &Ratings) -> ArtistData {
        ArtistData {
            id: it.id.clone(),
            name: it.name.clone(),
            starred: it.user_data.is_favorite,
            user_rating: r.get(&it.id).copied().unwrap_or(0),
            cover_art: it.cover(),
            biography: it.overview.clone(),
            ..Default::default()
        }
    }

    fn playlist_data(it: &WItem) -> PlaylistData {
        PlaylistData {
            id: it.id.clone(),
            name: it.name.clone(),
            song_count: it.child_count,
            duration: it.secs(),
            cover_art: it.cover(),
            ..Default::default()
        }
    }

    async fn albums_of(&self, v: Vec<WItem>) -> Vec<AlbumData> {
        let r = self.ratings().await;
        v.iter().map(|a| Self::album_data(a, &r)).collect()
    }

    async fn artists_of(&self, v: Vec<WItem>) -> Vec<ArtistData> {
        let r = self.ratings().await;
        v.iter().map(|a| Self::artist_data(a, &r)).collect()
    }

    async fn album_artists(&self, mut q: Params) -> Result<Vec<WItem>> {
        q.extend([("userId", self.uid()), ("parentId", self.lib()), ("Recursive", "true".into())]);
        Ok(items(self.get("Artists/AlbumArtists", &q).await?).items)
    }

    /// Search helper (`__fetch_type`).
    async fn fetch_type(&self, kind: &str, query: &str, (limit, offset): (u32, u32)) -> Result<Vec<WItem>> {
        if limit == 0 {
            return Ok(vec![]);
        }
        let mut q: Params = vec![("SearchTerm", query.into()), ("Limit", limit.to_string()), ("StartIndex", offset.to_string())];
        if kind == "MusicArtist" {
            q.push(("Fields", "Overview".into()));
            return self.album_artists(q).await;
        }
        q.extend([("IncludeItemTypes", kind.into()), ("Recursive", "true".into())]);
        q.push(("Fields", if kind == "Audio" { "ArtistItems,RunTimeTicks,MediaSources".into() } else { "ChildCount".into() }));
        if kind != "Playlist" {
            q.push(("ParentId", self.lib()));
        }
        Ok(self.user_items(q).await?.items)
    }

    async fn playlist_items(&self, id: &str) -> Result<Vec<WItem>> {
        let q = [("UserId", self.uid()), ("Fields", "RunTimeTicks,MediaSources,ArtistItems".into())];
        Ok(items(self.get(&format!("Playlists/{id}/Items"), &q).await?).items)
    }

    fn queue_path() -> std::path::PathBuf {
        crate::config::integrations_dir().join(KIND).join("queue.json")
    }

    async fn session_post(&self, path: &str, body: Value) -> bool {
        match self.raw(Method::POST, path, &[], Some(body)).await {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!("jellyfin {path}: {e}");
                false
            }
        }
    }
}

const SONG_FIELDS: &str = "ArtistItems,AlbumId,RunTimeTicks,UserData,IndexNumber,ParentIndexNumber,MediaSources,Path";

#[async_trait]
impl Backend for Jellyfin {
    fn kind(&self) -> &'static str {
        KIND
    }

    async fn ping(&self) -> Result<()> {
        *self.auth.lock().unwrap() = Auth { library: self.lib(), ..Default::default() };
        let qc = self.req(Method::POST, "Users/AuthenticateWithQuickConnect", &[], Some(json!({"Secret": self.conn.password}))).await;
        let v = match qc {
            Ok(v) if v["AccessToken"].is_string() => v,
            _ => self
                .req(Method::POST, "Users/AuthenticateByName", &[], Some(json!({"Username": self.conn.user, "Pw": self.conn.password})))
                .await
                .map_err(|e| anyhow::anyhow!("Could not log in: {e}"))?,
        };
        let (Some(token), Some(uid)) = (v["AccessToken"].as_str(), v["User"]["Id"].as_str()) else { bail!("Could not log in") };
        {
            let mut a = self.auth.lock().unwrap();
            a.token = token.into();
            a.user_id = uid.into();
            a.user_name = v["User"]["Name"].as_str().unwrap_or(&self.conn.user).into();
        }
        if self.lib().is_empty() && self.conn.library_ids.is_empty() {
            if let Ok((_, libs)) = self.libraries().await
                && let Some(l) = libs.first().filter(|l| l.id != "All")
            {
                self.auth.lock().unwrap().library = l.id.clone();
            }
        }
        Ok(())
    }

    async fn album_list(&self, kind: AlbumListKind, size: u32, offset: u32) -> Result<Vec<AlbumData>> {
        let mut q: Params = vec![
            ("IncludeItemTypes", "MusicAlbum".into()),
            ("Recursive", "true".into()),
            ("Limit", size.to_string()),
            ("StartIndex", offset.to_string()),
            ("Fields", "ArtistItems,ChildCount".into()),
            ("ParentId", self.lib()),
        ];
        let sort = |by: &str| [("SortBy", by.to_string()), ("SortOrder", "Descending".to_string())];
        match kind {
            AlbumListKind::Random => q.push(("SortBy", "Random".into())),
            AlbumListKind::Newest => q.extend(sort("DateCreated")),
            AlbumListKind::Frequent => q.extend(sort("PlayCount")),
            AlbumListKind::Recent => q.extend(sort("DatePlayed")),
            AlbumListKind::Starred => q.push(("Filters", "IsFavorite".into())),
            AlbumListKind::AlphabeticalByName => q.push(("SortBy", "SortName".into())),
        }
        let v = self.user_items(q).await?.items;
        Ok(self.albums_of(v).await)
    }

    async fn artists(&self, size: u32) -> Result<Vec<ArtistData>> {
        let v = self
            .album_artists(vec![("Limit", size.to_string()), ("Fields", "Overview".into()), ("SortBy", "Random".into())])
            .await?;
        Ok(self.artists_of(v).await)
    }

    async fn playlists(&self) -> Result<Vec<PlaylistData>> {
        let q = vec![("IncludeItemTypes", "Playlist".into()), ("Recursive", "true".into()), ("Fields", "ChildCount".into())];
        Ok(self.user_items(q).await?.items.iter().map(Self::playlist_data).collect())
    }

    async fn starred(&self) -> Result<Starred> {
        let fav = |t: &str| -> Params {
            vec![
                ("IncludeItemTypes", t.into()),
                ("Recursive", "true".into()),
                ("Filters", "IsFavorite".into()),
                ("ParentId", self.lib()),
                ("Fields", "ArtistItems,ChildCount,RunTimeTicks,MediaSources".into()),
            ]
        };
        let (ar, al, so) = tokio::join!(
            self.album_artists(vec![("Filters", "IsFavorite".into())]),
            self.user_items(fav("MusicAlbum")),
            self.user_items(fav("Audio"))
        );
        let r = self.ratings().await;
        Ok(Starred {
            artists: ar?.iter().map(|a| Self::artist_data(a, &r)).collect(),
            albums: al?.items.iter().map(|a| Self::album_data(a, &r)).collect(),
            songs: self.songs(so?.items, &r),
        })
    }

    async fn artist(&self, id: &str) -> Result<ArtistData> {
        let it = self.item(id, "Overview").await?;
        let albums_q = vec![
            ("AlbumArtistIds", id.to_string()),
            ("IncludeItemTypes", "MusicAlbum".into()),
            ("Recursive", "true".into()),
            ("SortBy", "PremiereDate".into()),
            ("Fields", "ArtistItems,ChildCount".into()),
        ];
        let (sim_path, sim_q) = (format!("Items/{id}/Similar"), [("userId", self.uid()), ("limit", "12".to_string())]);
        let (albums, similar) = tokio::join!(self.user_items(albums_q), self.get(&sim_path, &sim_q));
        let r = self.ratings().await;
        let mut a = Self::artist_data(&it, &r);
        let albums = albums?;
        a.album_count = if albums.total_record_count > 0 { albums.total_record_count } else { albums.items.len() as i32 };
        a.albums = Some(albums.items.iter().map(|x| Self::album_data(x, &r)).collect());
        a.similar = similar.map(|v| items(v).items.into_iter().map(|s| Ref { id: s.id, name: s.name }).collect()).unwrap_or_default();
        Ok(a)
    }

    async fn album(&self, id: &str) -> Result<AlbumData> {
        let it = self.item(id, "ArtistItems").await?;
        let q = vec![
            ("ParentId", id.to_string()),
            ("IncludeItemTypes", "Audio".into()),
            ("Recursive", "true".into()),
            ("Fields", SONG_FIELDS.into()),
            ("SortBy", "ParentIndexNumber,IndexNumber".into()),
            ("SortOrder", "Ascending".into()),
        ];
        let songs = self.user_items(q).await?.items;
        let r = self.ratings().await;
        let mut a = Self::album_data(&it, &r);
        let mut songs = self.songs(songs, &r);
        for (i, s) in songs.iter_mut().enumerate() {
            if s.track == 0 {
                s.track = i as i32;
            }
        }
        a.song_count = songs.len() as i32;
        a.duration = songs.iter().map(|s| s.duration).sum();
        a.songs = Some(songs);
        Ok(a)
    }

    async fn playlist(&self, id: &str) -> Result<PlaylistData> {
        let (it, entries) = tokio::join!(self.item(id, "ChildCount"), self.playlist_items(id));
        let mut p = Self::playlist_data(&it?);
        let entries = self.songs(entries?, &self.ratings().await);
        p.song_count = entries.len() as i32;
        p.duration = entries.iter().map(|s| s.duration).sum();
        p.entries = Some(entries);
        Ok(p)
    }

    async fn song(&self, id: &str) -> Result<SongData> {
        let it = self.item(id, SONG_FIELDS).await?;
        Ok(self.song(it, &self.ratings().await))
    }

    async fn search(&self, query: &str, c: SearchCounts) -> Result<SearchResults> {
        let (ar, al, so, pl) = tokio::join!(
            self.fetch_type("MusicArtist", query, c.artist),
            self.fetch_type("MusicAlbum", query, c.album),
            self.fetch_type("Audio", query, c.song),
            self.fetch_type("Playlist", query, c.playlist)
        );
        let r = self.ratings().await;
        Ok(SearchResults {
            artists: ar?.iter().map(|a| Self::artist_data(a, &r)).collect(),
            albums: al?.iter().map(|a| Self::album_data(a, &r)).collect(),
            songs: self.songs(so?, &r),
            playlists: pl?.iter().map(Self::playlist_data).collect(),
        })
    }

    fn stream_url(&self, song: &str, max_bitrate: i32) -> String {
        let (mut container, mut codec) = self.formats.lock().unwrap().get(song).cloned().unwrap_or_default();
        let mut u = match url::Url::parse(&format!("{}/Audio/{song}/universal", self.base())) {
            Ok(u) => u,
            Err(_) => return String::new(),
        };
        {
            let mut q = u.query_pairs_mut();
            q.append_pair("ApiKey", &self.token()).append_pair("userId", &self.uid()).append_pair("deviceId", &device_id());
            if max_bitrate > 0 {
                if ["flac", "wav", "alac", "ape", "pcm"].contains(&codec.as_str()) {
                    container = "opus,ogg".into();
                    codec = "opus".into();
                }
                q.append_pair("maxStreamingBitrate", &(max_bitrate * 1000).to_string()).append_pair("transcodingProtocol", "hls");
            }
            q.append_pair("container", &container).append_pair("audioCodec", &codec);
        }
        u.into()
    }

    async fn cover_art(&self, id: &str, size: u32) -> Result<Option<Vec<u8>>> {
        if id.is_empty() || id == "None" {
            return Ok(None);
        }
        let path = if id.starts_with("Items/") { id.to_string() } else { format!("Items/{id}/Images/Primary") };
        let r = self.raw(Method::GET, &path, &[("maxWidth", size.to_string()), ("quality", "90".into())], None).await;
        match r {
            Ok(r) => Ok(Some(r.bytes().await?.to_vec()).filter(|b| !b.is_empty())),
            Err(_) => Ok(None),
        }
    }

    fn cover_art_url(&self, id: &str) -> String {
        let path = if id.starts_with("Items/") { id.to_string() } else { format!("Items/{id}/Images/Primary") };
        let Ok(mut u) = url::Url::parse(&format!("{}/{path}", self.base())) else { return String::new() };
        u.query_pairs_mut().append_pair("maxWidth", "240").append_pair("quality", "90").append_pair("api_key", &self.token());
        u.into()
    }

    async fn set_starred(&self, id: &str, starred: bool) -> Result<()> {
        let m = if starred { Method::POST } else { Method::DELETE };
        self.req(m, &format!("Users/{}/FavoriteItems/{id}", self.uid()), &[], None).await.map(drop)
    }

    async fn set_rating(&self, id: &str, rating: i32) -> Result<()> {
        let db = self.db().await?;
        if rating == 0 {
            sqlx::query("DELETE FROM ratings WHERE id = ?").bind(id).execute(db).await?;
        } else {
            sqlx::query("INSERT OR REPLACE INTO ratings (id, rating) VALUES (?, ?)").bind(id).bind(rating).execute(db).await?;
        }
        Ok(())
    }

    async fn playback_report(&self, song: &str, report: PlaybackReport, position_s: f64) -> Result<()> {
        let ticks = (position_s * TICKS) as i64;
        let mut s = self.play.lock().await;
        let stop = |s: &PlaySession| json!({"ItemId": s.item, "PlaySessionId": s.session});
        if report == PlaybackReport::Stop {
            if !s.session.is_empty() && self.session_post("Sessions/Playing/Stopped", stop(&s)).await {
                *s = PlaySession::default();
            }
            return Ok(());
        }
        // New song, or a new session after 10 minutes of inactivity.
        if s.item != song || s.session.is_empty() || now() - s.last_ping > 600.0 {
            if !s.session.is_empty() && s.item != song {
                self.session_post("Sessions/Playing/Stopped", stop(&s)).await;
            }
            *s = PlaySession { item: song.into(), session: uuid::Uuid::new_v4().to_string(), last_ping: 0.0 };
            let body = json!({"ItemId": song, "PositionTicks": ticks, "PlaySessionId": s.session, "IsPaused": report == PlaybackReport::Pause});
            if !self.session_post("Sessions/Playing", body).await {
                *s = PlaySession::default();
                bail!("No play session provided by server");
            }
            s.last_ping = now();
            if report != PlaybackReport::Progress {
                return Ok(());
            }
        }
        let mut body = json!({"ItemId": s.item, "PositionTicks": ticks, "PlaySessionId": s.session});
        match report {
            PlaybackReport::Play => body["IsPaused"] = false.into(),
            PlaybackReport::Pause => body["IsPaused"] = true.into(),
            _ => {}
        }
        if self.session_post("Sessions/Playing/Progress", body).await {
            s.last_ping = now();
        }
        Ok(())
    }

    async fn random_songs(&self, size: u32) -> Result<Vec<SongData>> {
        let q = vec![
            ("IncludeItemTypes", "Audio".into()),
            ("Recursive", "true".into()),
            ("Fields", SONG_FIELDS.into()),
            ("Limit", size.to_string()),
            ("SortBy", "Random".into()),
            ("MediaTypes", "Audio".into()),
            ("ParentId", self.lib()),
        ];
        let v = self.user_items(q).await?.items;
        Ok(self.songs(v, &self.ratings().await))
    }

    async fn similar_songs(&self, id: &str, count: u32) -> Result<Vec<SongData>> {
        // `id` is an artist id (as in Python): take one of its songs as the seed.
        let seed = self
            .user_items(vec![("ArtistIds", id.into()), ("IncludeItemTypes", "Audio".into()), ("Recursive", "true".into()), ("Limit", "1".into())])
            .await?
            .items;
        let seed = seed.first().map(|s| s.id.clone()).unwrap_or_else(|| id.to_string());
        let q = [("UserId", self.uid()), ("Limit", count.to_string()), ("IncludeItemTypes", "Audio".into()), ("Fields", SONG_FIELDS.into())];
        let v = items(self.get(&format!("Items/{seed}/Similar"), &q).await?).items;
        Ok(self.songs(v, &self.ratings().await))
    }

    async fn top_songs(&self, artist_id: &str, count: u32) -> Result<Vec<SongData>> {
        let q = vec![
            ("ArtistIds", artist_id.into()),
            ("IncludeItemTypes", "Audio".into()),
            ("SortBy", "PlayCount".into()),
            ("SortOrder", "Descending".into()),
            ("Limit", count.to_string()),
            ("Recursive", "true".into()),
            ("ParentId", self.lib()),
            ("Fields", SONG_FIELDS.into()),
        ];
        let v = self.user_items(q).await?.items;
        Ok(self.songs(v, &self.ratings().await))
    }

    async fn play_queue(&self) -> Result<PlayQueue> {
        let v: Value = tokio::fs::read(Self::queue_path()).await.ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let ids: Vec<String> = serde_json::from_value(v["id"].clone()).unwrap_or_default();
        let cur = v["current"].as_str().filter(|c| ids.iter().any(|i| i == c)).map(String::from).or_else(|| ids.first().cloned());
        Ok(PlayQueue { current: cur, ids, songs: vec![], position_ms: v["position"].as_i64().unwrap_or(0) })
    }

    async fn save_play_queue(&self, ids: &[String], current: &str, position_ms: i64) -> Result<()> {
        let cur = if ids.iter().any(|i| i == current) { current.to_string() } else { ids.first().cloned().unwrap_or_default() };
        let p = Self::queue_path();
        if let Some(d) = p.parent() {
            tokio::fs::create_dir_all(d).await?;
        }
        tokio::fs::write(p, serde_json::to_vec(&json!({"id": ids, "current": cur, "position": position_ms}))?).await?;
        Ok(())
    }

    async fn lyrics(&self, id: &str) -> Result<Option<Lyrics>> {
        let v = self.get(&format!("Audio/{id}/Lyrics"), &[]).await?;
        Ok(parse_lyrics(&v))
    }

    async fn radios(&self) -> Result<Vec<SongData>> {
        let v = items(self.get("LiveTv/Channels", &[("userId", self.uid()), ("type", "Radio".into())]).await?).items;
        let deleted = self.deleted_radios.lock().unwrap().clone();
        let mut out = vec![];
        for r in v.into_iter().filter(|r| !deleted.contains(&r.id)) {
            let info = self.get(&format!("Items/{}/PlaybackInfo", r.id), &[("fields", "Path".into()), ("userId", self.uid())]).await.unwrap_or_default();
            let url = info["MediaSources"][0]["Path"].as_str().map(String::from).unwrap_or_else(|| self.stream_url(&r.id, 0));
            out.push(SongData { cover_art: r.cover(), id: r.id, title: r.name, duration: -1, radio_stream_url: url, ..Default::default() });
        }
        Ok(out)
    }

    async fn create_radio(&self, name: &str, url: &str) -> Result<()> {
        let v = self.req(Method::POST, "LiveTv/TunerHosts", &[], Some(json!({"Url": url, "Type": "M3U", "FriendlyName": name}))).await?;
        if v["Id"].is_null() {
            bail!("Could not create radio");
        }
        Ok(())
    }

    async fn delete_radio(&self, id: &str) -> Result<()> {
        self.req(Method::DELETE, "LiveTv/TunerHosts", &[("id", id.into())], None).await?;
        self.deleted_radios.lock().unwrap().push(id.into());
        Ok(())
    }

    async fn create_playlist(&self, name: &str, songs: &[String]) -> Result<String> {
        let v = self
            .req(Method::POST, "Playlists", &[("UserId", self.uid()), ("MediaType", "Audio".into())], Some(json!({"Name": name, "Ids": songs})))
            .await?;
        Ok(v["Id"].as_str().unwrap_or_default().into())
    }

    async fn update_playlist(&self, id: &str, name: Option<&str>, add: &[String], remove: &[u32]) -> Result<()> {
        if let Some(n) = name {
            self.req(Method::POST, &format!("Playlists/{id}"), &[], Some(json!({"Name": n}))).await?;
        }
        if !remove.is_empty() {
            let cur = self.playlist_items(id).await?;
            let entries: Vec<&str> = remove.iter().filter_map(|i| cur.get(*i as usize)).map(|i| i.playlist_item_id.as_str()).collect();
            if !entries.is_empty() {
                self.req(Method::DELETE, &format!("Playlists/{id}/Items"), &[("EntryIds", entries.join(","))], None).await?;
            }
        }
        if !add.is_empty() {
            self.req(Method::POST, &format!("Playlists/{id}/Items"), &[("Ids", add.join(",")), ("UserId", self.uid())], None).await?;
        }
        Ok(())
    }

    async fn delete_playlist(&self, id: &str) -> Result<()> {
        self.req(Method::DELETE, &format!("Items/{id}"), &[], None).await.map(drop)
    }

    async fn libraries(&self) -> Result<(bool, Vec<Ref>)> {
        let v = items(self.get(&format!("Users/{}/Views", self.uid()), &[]).await?).items;
        let mut libs: Vec<Ref> = v.into_iter().filter(|l| l.collection_type == "music").map(|l| Ref { id: l.id, name: l.name }).collect();
        if libs.len() > 1 {
            libs.insert(0, Ref { id: "All".into(), name: "All".into() });
        }
        Ok((false, libs))
    }

    async fn song_details(&self, id: &str) -> Result<Vec<(String, String)>> {
        let s = self.item(id, "MediaSources,Genres,ArtistItems,Path,ProductionYear,Taglines,ProviderIds").await?;
        let src = s.media_sources.first().cloned().unwrap_or_default();
        let st = src.media_streams.iter().find(|m| m.kind == "Audio").cloned().unwrap_or_default();
        let rows = [
            ("Album", s.album.clone()),
            ("Display Artist", s.artists.first().cloned().unwrap_or_default()),
            ("Artists", s.artist_items.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ")),
            ("Genres", s.genres.join(", ")),
            ("Track Number", s.index_number.unwrap_or(0).to_string()),
            ("Disc Number", s.parent_index_number.unwrap_or(0).to_string()),
            ("Year", s.production_year.to_string()),
            ("Duration", crate::config::display_time(s.secs().into())),
            ("Suffix", src.container.clone()),
            ("Size", if src.size > 0 { format!("{:.1} MB", src.size as f64 / 1_048_576.0) } else { String::new() }),
            ("Bit Rate", format!("{} kbps", src.bitrate / 1000)),
            ("Bit Depth", st.bit_depth.to_string()),
            ("Sampling Rate", format!("{} Hz", st.sample_rate)),
            ("Channel Count", st.channels.to_string()),
            ("Track Gain", s.normalization_gain.unwrap_or(0.0).to_string()),
            ("Album Gain", s.album_normalization_gain.or(s.normalization_gain).unwrap_or(0.0).to_string()),
            ("MusicBrainz ID", s.provider_ids.get("MusicBrainzTrack").cloned().unwrap_or_default()),
            ("Path", if s.path.is_empty() { src.path } else { s.path.clone() }),
        ];
        Ok(rows.into_iter().filter(|(_, v)| !v.is_empty() && v != "0" && !v.starts_with("0 ")).map(|(k, v)| (k.to_string(), v)).collect())
    }

    async fn server_info(&self) -> Result<ServerInfo> {
        let q = [("maxWidth", "240".to_string()), ("quality", "90".into())];
        let avatar = match self.raw(Method::GET, &format!("Users/{}/Images/Primary", self.uid()), &q, None).await {
            Ok(r) => r.bytes().await.ok().map(|b| b.to_vec()).filter(|b| !b.is_empty()),
            Err(_) => None,
        };
        let info = self.get("System/Info", &[]).await.unwrap_or_default();
        let user = self.auth.lock().unwrap().user_name.clone();
        Ok(ServerInfo {
            username: if user.is_empty() { self.conn.user.clone() } else { user },
            title: format!("{} {}", info["ServerName"].as_str().unwrap_or("Jellyfin"), info["Version"].as_str().unwrap_or("")),
            link: self.base(),
            avatar,
        })
    }

    async fn download(&self, id: &str, dest: &std::path::Path) -> Result<()> {
        use futures::StreamExt;
        use tokio::io::AsyncWriteExt;
        let mut stream = self.raw(Method::GET, &format!("Items/{id}/Download"), &[], None).await?.bytes_stream();
        let mut f = tokio::fs::File::create(dest).await?;
        while let Some(chunk) = stream.next().await {
            f.write_all(&chunk?).await?;
        }
        f.flush().await?;
        Ok(())
    }
}

fn parse_lyrics(v: &Value) -> Option<Lyrics> {
    let lines = v["Lyrics"].as_array().filter(|l| !l.is_empty())?;
    let text = |l: &Value| l["Text"].as_str().unwrap_or("").to_string();
    if lines[0]["Start"].as_i64().is_some_and(|s| s > 0) || lines.get(1).is_some_and(|l| l["Start"].is_number()) {
        let content = lines
            .iter()
            .map(|l| {
                let ms = l["Start"].as_i64().unwrap_or(0) / 10_000;
                format!("[{:02}:{:02}.{:02}] {}", ms / 60000, (ms % 60000) / 1000, (ms % 1000) / 10, text(l).trim())
            })
            .collect::<Vec<_>>()
            .join("\n");
        Some(Lyrics { kind: LyricsKind::Synced, content })
    } else {
        let content = lines.iter().map(text).collect::<Vec<_>>().join("\n");
        (!content.trim().is_empty()).then_some(Lyrics { kind: LyricsKind::Plain, content })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_song_item() {
        let v = json!({"Items": [{"Id": "s1", "Name": "Song", "Album": "Al", "AlbumId": "a1", "AlbumArtist": "X",
            "AlbumPrimaryImageTag": "tag", "ArtistItems": [{"Id": "ar1", "Name": "X"}], "RunTimeTicks": 1_850_000_000i64,
            "IndexNumber": 3, "ParentIndexNumber": 1, "ProductionYear": 2001, "UserData": {"IsFavorite": true},
            "NormalizationGain": -6.5, "Container": "flac",
            "MediaSources": [{"MediaStreams": [{"Type": "Audio", "Codec": "FLAC"}]}]}], "TotalRecordCount": 1});
        let j = Jellyfin::new(Connection { url: "http://h:8096/".into(), ..Default::default() }).unwrap();
        let w = items(v);
        assert_eq!(w.total_record_count, 1);
        let s = j.song(w.items[0].clone(), &Ratings::from([("s1".into(), 4)]));
        assert_eq!((s.duration, s.track, s.year, s.user_rating), (185, 3, 2001, 4));
        assert!(s.starred);
        assert_eq!(s.artist_id, "ar1");
        assert_eq!(s.cover_art, "Items/a1/Images/Primary?=tag");
        assert_eq!(s.album_gain, -6.5);
        let url = j.stream_url("s1", 128);
        assert!(url.starts_with("http://h:8096/Audio/s1/universal?"));
        assert!(url.contains("audioCodec=opus") && url.contains("maxStreamingBitrate=128000"));
        assert!(j.stream_url("s1", 0).contains("audioCodec=flac"));
    }

    #[test]
    fn parses_lyrics() {
        let l = parse_lyrics(&json!({"Lyrics": [{"Start": 0, "Text": "a"}, {"Start": 12_340_000, "Text": " b "}]})).unwrap();
        assert_eq!(l.kind, LyricsKind::Synced);
        assert_eq!(l.content, "[00:00.00] a\n[00:01.23] b");
        assert_eq!(parse_lyrics(&json!({"Lyrics": [{"Text": "x"}]})).unwrap().kind, LyricsKind::Plain);
    }

    #[test]
    fn header() {
        assert!(auth_header("tok").starts_with(r#"MediaBrowser Client="Nocturne""#));
        assert!(auth_header("tok").ends_with(r#"Token="tok""#));
    }
}
