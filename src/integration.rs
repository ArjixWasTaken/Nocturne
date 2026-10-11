#![allow(non_snake_case)]
//! Main-thread façade over a [`Backend`]: owns the model cache (one GObject per
//! id, shared by every widget), the database and the global [`CurrentState`].
//! Exposed to templates as `$NocturneIntegrationBase`.
use crate::{
    backend::{self, AlbumData, AlbumListKind, ArtistData, Backend, LyricsKind, PlaylistData, SearchCounts, SongData},
    db::{self, Db},
    models::*,
    runtime::run,
};
use gtk::{gdk, glib, prelude::*, subclass::prelude::*};
use std::{cell::RefCell, collections::HashMap, future::Future, sync::Arc};

pub const COVER_SMALL: u32 = 240;
pub const COVER_BIG: u32 = 720;

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::Integration)]
    pub struct Integration {
        #[property(get, name = "current-state")]
        pub current_state: CurrentState,
        #[property(get, set)]
        pub url: RefCell<String>,
        #[property(get, set)]
        pub user: RefCell<String>,
        #[property(get, set, name = "trustServer")]
        pub trust_server: std::cell::Cell<bool>,
        #[property(get, set, name = "libraryDir")]
        pub library_dir: RefCell<String>,
        /// Non-empty while a long operation runs (shown as a spinner in the sidebar).
        #[property(get, set, name = "loadingMessage")]
        pub loading_message: RefCell<String>,
        pub backend: RefCell<Option<Arc<dyn Backend>>>,
        pub db: RefCell<Option<Db>>,
        pub models: RefCell<HashMap<String, glib::Object>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Integration {
        const NAME: &'static str = "NocturneIntegrationBase";
        type Type = super::Integration;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Integration {}
}

glib::wrapper! {
    pub struct Integration(ObjectSubclass<imp::Integration>);
}

thread_local! {
    static CURRENT: RefCell<Option<Integration>> = const { RefCell::new(None) };
}

/// The integration the user is logged into, if any.
pub fn current() -> Option<Integration> {
    CURRENT.with(|c| c.borrow().clone())
}

/// Shorthand for widgets that only exist while logged in.
pub fn get() -> Integration {
    current().expect("no active integration")
}

pub fn set_current(i: Option<Integration>) {
    CURRENT.with(|c| *c.borrow_mut() = i);
}

fn log_err<T>(what: &str, r: anyhow::Result<T>) -> Option<T> {
    r.map_err(|e| tracing::warn!("{what}: {e:#}")).ok()
}

impl Integration {
    /// Build and connect (opening the database). Does not ping.
    pub async fn connect(kind: &str, conn: backend::Connection) -> anyhow::Result<Self> {
        let backend = backend::create(kind, conn.clone())?;
        let kind = kind.to_string();
        let db = run(async move { db::open_for(&kind).await }).await?;
        let obj: Self = glib::Object::builder()
            .property("url", &conn.url)
            .property("user", &conn.user)
            .property("trustServer", conn.trust_server)
            .property("libraryDir", &conn.library_dir)
            .build();
        obj.imp().backend.replace(Some(backend));
        obj.imp().db.replace(Some(db));
        Ok(obj)
    }

    pub fn backend(&self) -> Arc<dyn Backend> {
        self.imp().backend.borrow().clone().expect("integration without backend")
    }

    pub fn kind(&self) -> &'static str {
        self.backend().kind()
    }

    pub fn db(&self) -> Db {
        self.imp().db.borrow().clone().expect("integration without db")
    }

    /// Run a backend call on tokio.
    pub async fn call<T, F, Fut>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(Arc<dyn Backend>) -> Fut + Send + 'static,
        Fut: Future<Output = anyhow::Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        let b = self.backend();
        run(async move { f(b).await }).await
    }

    // ---------- model cache ----------

    fn model<T: IsA<glib::Object>>(&self, id: &str, make: impl FnOnce() -> T) -> T {
        let mut models = self.imp().models.borrow_mut();
        if let Some(m) = models.get(id).and_then(|o| o.clone().downcast::<T>().ok()) {
            return m;
        }
        let m = make();
        models.insert(id.to_string(), m.clone().upcast());
        m
    }

    /// Any loaded model by id.
    pub fn lookup(&self, id: &str) -> Option<glib::Object> {
        self.imp().models.borrow().get(id).cloned()
    }

    pub fn song(&self, id: &str) -> Song {
        self.model(id, || Song::new(id))
    }
    pub fn album(&self, id: &str) -> Album {
        self.model(id, || Album::new(id))
    }
    pub fn artist(&self, id: &str) -> Artist {
        self.model(id, || Artist::new(id))
    }
    pub fn playlist(&self, id: &str) -> Playlist {
        self.model(id, || Playlist::new(id))
    }

    /// Register an externally opened file (e.g. from the command line) as a song.
    pub fn insert_song(&self, song: &Song) {
        self.imp().models.borrow_mut().insert(song.id(), song.clone().upcast());
    }

    pub fn ingest_song(&self, d: &SongData) -> Song {
        let m = self.song(&d.id);
        let _guard = m.freeze_notify();
        set_if(&m, "title", &d.title);
        set_if(&m, "album", &d.album);
        set_if(&m, "albumId", &d.album_id);
        set_if(&m, "artist", &d.artist);
        set_if(&m, "artistId", &d.artist_id);
        set_if(&m, "artists", &RefList(d.artists.clone()));
        set_if(&m, "duration", &d.duration);
        set_if(&m, "track", &d.track);
        set_if(&m, "discNumber", &d.disc_number);
        set_if(&m, "starred", &d.starred);
        set_if(&m, "userRating", &d.user_rating);
        set_if(&m, "coverArt", &d.cover_art);
        set_if(&m, "albumGain", &d.album_gain);
        set_if(&m, "trackGain", &d.track_gain);
        set_if(&m, "radioStreamUrl", &d.radio_stream_url);
        set_if(&m, "container", &d.container);
        set_if(&m, "codec", &d.codec);
        set_if(&m, "path", &d.path);
        m
    }

    pub fn ingest_album(&self, d: &AlbumData) -> Album {
        let m = self.album(&d.id);
        let _guard = m.freeze_notify();
        set_if(&m, "name", &d.name);
        set_if(&m, "artist", &d.artist);
        set_if(&m, "artistId", &d.artist_id);
        set_if(&m, "artists", &RefList(d.artists.clone()));
        set_if(&m, "songCount", &d.song_count);
        set_if(&m, "duration", &d.duration);
        set_if(&m, "year", &d.year);
        set_if(&m, "starred", &d.starred);
        set_if(&m, "userRating", &d.user_rating);
        set_if(&m, "coverArt", &d.cover_art);
        if let Some(songs) = &d.songs {
            let ids = songs.iter().map(|s| self.ingest_song(s).id()).collect();
            set_if(&m, "song", &IdList(ids));
            m.set_verified(true);
        }
        m
    }

    pub fn ingest_artist(&self, d: &ArtistData) -> Artist {
        let m = self.artist(&d.id);
        let _guard = m.freeze_notify();
        set_if(&m, "name", &d.name);
        set_if(&m, "albumCount", &d.album_count);
        set_if(&m, "starred", &d.starred);
        set_if(&m, "userRating", &d.user_rating);
        set_if(&m, "coverArt", &d.cover_art);
        if !d.biography.is_empty() {
            set_if(&m, "biography", &d.biography);
        }
        if !d.similar.is_empty() {
            set_if(&m, "similarArtist", &RefList(d.similar.clone()));
        }
        if let Some(albums) = &d.albums {
            let ids = albums.iter().map(|a| self.ingest_album(a).id()).collect();
            set_if(&m, "album", &IdList(ids));
            m.set_verified(true);
        }
        m
    }

    pub fn ingest_playlist(&self, d: &PlaylistData) -> Playlist {
        let m = self.playlist(&d.id);
        let _guard = m.freeze_notify();
        set_if(&m, "name", &d.name);
        set_if(&m, "songCount", &d.song_count);
        set_if(&m, "duration", &d.duration);
        set_if(&m, "readonly", &d.readonly);
        set_if(&m, "coverArt", &d.cover_art);
        if let Some(entries) = &d.entries {
            let ids = entries.iter().map(|s| self.ingest_song(s).id()).collect();
            set_if(&m, "entry", &IdList(ids));
            m.set_verified(true);
        }
        m
    }

    // ---------- verify (fetch full metadata once) ----------

    pub async fn verify_song(&self, id: &str, force: bool) -> Song {
        let m = self.song(id);
        if (force || !m.verified()) && !m.isExternalFile() && m.radioStreamUrl().is_empty() {
            let id = id.to_string();
            if let Some(d) = log_err("song", self.call(move |b| async move { b.song(&id).await }).await) {
                self.ingest_song(&d);
                m.set_verified(true);
            }
        }
        m
    }

    pub async fn verify_album(&self, id: &str, force: bool) -> Album {
        let m = self.album(id);
        if force || !m.verified() {
            let id = id.to_string();
            if let Some(d) = log_err("album", self.call(move |b| async move { b.album(&id).await }).await) {
                self.ingest_album(&d);
            }
        }
        m
    }

    pub async fn verify_artist(&self, id: &str, force: bool) -> Artist {
        let m = self.artist(id);
        if force || !m.verified() {
            let id = id.to_string();
            if let Some(d) = log_err("artist", self.call(move |b| async move { b.artist(&id).await }).await) {
                self.ingest_artist(&d);
            }
        }
        m
    }

    pub async fn verify_playlist(&self, id: &str, force: bool) -> Playlist {
        let m = self.playlist(id);
        if force || !m.verified() {
            let id = id.to_string();
            if let Some(d) = log_err("playlist", self.call(move |b| async move { b.playlist(&id).await }).await) {
                self.ingest_playlist(&d);
            }
        }
        m
    }

    // ---------- cover art ----------

    /// Raw cover bytes (cached in the db).
    pub async fn cover_bytes(&self, id: &str, size: u32) -> Option<Vec<u8>> {
        let cover_id = self.lookup(id).map(|m| m.property::<String>("coverArt")).filter(|c| !c.is_empty()).unwrap_or_else(|| id.to_string());
        let db = self.db();
        let b = self.backend();
        let key = id.to_string();
        run(async move {
            if let Some(bytes) = db::cached_image(&db, &key, size).await {
                return Some(bytes);
            }
            let bytes = b.cover_art(&cover_id, size).await.ok().flatten()?;
            let _ = db::save_image(&db, &key, size, &bytes).await;
            Some(bytes)
        })
        .await
    }

    /// Fill `gdkPaintable` / `gdkPaintableBig` of a loaded model if missing.
    pub async fn load_cover(&self, id: &str) {
        let Some(m) = self.lookup(id) else { return };
        for (prop, size) in [("gdkPaintable", COVER_SMALL), ("gdkPaintableBig", COVER_BIG)] {
            if m.property::<Option<gdk::Paintable>>(prop).is_some() {
                continue;
            }
            if let Some(tex) = self.cover_bytes(id, size).await.and_then(|b| texture(&b)) {
                m.set_property(prop, Some(tex.upcast::<gdk::Paintable>()));
            }
        }
    }

    pub fn cover_art_url(&self, id: &str) -> String {
        self.backend().cover_art_url(id)
    }

    // ---------- lists (ingest + return ids) ----------

    pub async fn album_list(&self, kind: AlbumListKind, size: u32, offset: u32) -> Vec<String> {
        let r = self.call(move |b| async move { b.album_list(kind, size, offset).await }).await;
        log_err("album list", r).unwrap_or_default().iter().map(|a| self.ingest_album(a).id()).collect()
    }

    pub async fn artists(&self, size: u32) -> Vec<String> {
        let r = self.call(move |b| async move { b.artists(size).await }).await;
        log_err("artists", r).unwrap_or_default().iter().map(|a| self.ingest_artist(a).id()).collect()
    }

    pub async fn playlists(&self) -> Vec<String> {
        let r = self.call(|b| async move { b.playlists().await }).await;
        log_err("playlists", r).unwrap_or_default().iter().map(|p| self.ingest_playlist(p).id()).collect()
    }

    /// `(artists, albums, songs)` ids.
    pub async fn starred(&self) -> (Vec<String>, Vec<String>, Vec<String>) {
        let s = log_err("starred", self.call(|b| async move { b.starred().await }).await).unwrap_or_default();
        (
            s.artists.iter().map(|a| self.ingest_artist(a).id()).collect(),
            s.albums.iter().map(|a| self.ingest_album(a).id()).collect(),
            s.songs.iter().map(|a| self.ingest_song(a).id()).collect(),
        )
    }

    /// `(artists, albums, songs, playlists)` ids.
    pub async fn search(&self, query: &str, counts: SearchCounts) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
        let q = query.to_string();
        let s = log_err("search", self.call(move |b| async move { b.search(&q, counts).await }).await).unwrap_or_default();
        (
            s.artists.iter().map(|a| self.ingest_artist(a).id()).collect(),
            s.albums.iter().map(|a| self.ingest_album(a).id()).collect(),
            s.songs.iter().map(|a| self.ingest_song(a).id()).collect(),
            s.playlists.iter().map(|a| self.ingest_playlist(a).id()).collect(),
        )
    }

    fn ingest_songs(&self, what: &str, r: anyhow::Result<Vec<SongData>>) -> Vec<String> {
        log_err(what, r).unwrap_or_default().iter().map(|s| self.ingest_song(s).id()).collect()
    }

    pub async fn random_songs(&self, size: u32) -> Vec<String> {
        let r = self.call(move |b| async move { b.random_songs(size).await }).await;
        self.ingest_songs("random songs", r)
    }

    pub async fn similar_songs(&self, id: &str, count: u32) -> Vec<String> {
        let id = id.to_string();
        let r = self.call(move |b| async move { b.similar_songs(&id, count).await }).await;
        self.ingest_songs("similar songs", r)
    }

    pub async fn top_songs(&self, artist: &str, count: u32) -> Vec<String> {
        let id = artist.to_string();
        let r = self.call(move |b| async move { b.top_songs(&id, count).await }).await;
        self.ingest_songs("top songs", r)
    }

    pub async fn radios(&self) -> Vec<String> {
        let r = self.call(|b| async move { b.radios().await }).await;
        self.ingest_songs("radios", r)
    }

    // ---------- mutations ----------

    pub async fn set_starred(&self, id: &str, starred: bool) -> bool {
        let i = id.to_string();
        let ok = log_err("star", self.call(move |b| async move { b.set_starred(&i, starred).await }).await).is_some();
        if ok && let Some(m) = self.lookup(id) {
            m.set_property("starred", starred);
        }
        ok
    }

    pub async fn set_rating(&self, id: &str, rating: i32) -> bool {
        let i = id.to_string();
        let ok = log_err("rating", self.call(move |b| async move { b.set_rating(&i, rating).await }).await).is_some();
        if ok && let Some(m) = self.lookup(id) {
            m.set_property("userRating", rating);
        }
        ok
    }

    /// Server scrobble + monthly stats (on submission).
    pub async fn scrobble(&self, id: &str, submission: bool) {
        let song = self.song(id);
        if song.isExternalFile() || !song.radioStreamUrl().is_empty() {
            return;
        }
        let i = id.to_string();
        log_err("scrobble", self.call(move |b| async move { b.scrobble(&i, submission).await }).await);
        if submission {
            let db = self.db();
            let i = id.to_string();
            let month = chrono::Local::now().format("%m-%Y").to_string();
            log_err("stats", run(async move { db::add_monthly_play(&db, &month, &i).await }).await);
        }
    }

    // ---------- playback helpers ----------

    /// GStreamer URI for a song (resolves radio playlists, local files).
    pub async fn stream_url(&self, id: &str) -> String {
        let song = self.song(id);
        if song.isExternalFile() {
            return glib::filename_to_uri(song.path(), None).map(Into::into).unwrap_or_default();
        }
        let radio = song.radioStreamUrl();
        if !radio.is_empty() {
            return run(resolve_radio(radio, self.trustServer())).await;
        }
        let bitrate = crate::config::settings().int("max-bitrate");
        self.backend().stream_url(id, bitrate)
    }

    /// Lyrics: database first, then the server, then LRCLIB (when `online`).
    pub async fn lyrics(&self, id: &str, online: bool) -> Option<backend::Lyrics> {
        let db = self.db();
        let key = id.to_string();
        if let Some((kind, content)) = run(async move { db::lyrics(&db, &key).await }).await {
            let kind = if kind == "lrc" || kind == "synced" { LyricsKind::Synced } else { LyricsKind::Plain };
            return Some(backend::Lyrics { kind, content });
        }
        let i = id.to_string();
        if let Ok(Some(l)) = self.call(move |b| async move { b.lyrics(&i).await }).await {
            return Some(l);
        }
        if !online {
            return None;
        }
        let song = self.song(id);
        let (title, artist, album, duration) = (song.title(), song.artist(), song.album(), song.duration());
        let found = run(async move { crate::lyrics::lrclib(&title, &artist, &album, duration).await }).await;
        if let Some(l) = &found {
            self.save_lyrics(id, l).await;
        }
        found
    }

    pub async fn save_lyrics(&self, id: &str, l: &backend::Lyrics) {
        let db = self.db();
        let (i, c) = (id.to_string(), l.content.clone());
        let kind = if l.kind == LyricsKind::Synced { "lrc" } else { "plain" };
        log_err("save lyrics", run(async move { db::save_lyrics(&db, &i, kind, &c).await }).await);
    }

    pub async fn delete_lyrics(&self, id: &str) {
        let db = self.db();
        let i = id.to_string();
        log_err("delete lyrics", run(async move { db::delete_lyrics(&db, &i).await }).await);
    }

    pub async fn playlist_resume(&self, origin: &str) -> Option<(String, f64)> {
        let db = self.db();
        let o = origin.to_string();
        run(async move { db::playlist_resume(&db, &o).await }).await
    }

    pub async fn save_playlist_resume(&self, origin: &str, song: &str, ts: f64) {
        let db = self.db();
        let (o, s) = (origin.to_string(), song.to_string());
        log_err("resume", run(async move { db::save_playlist_resume(&db, &o, &s, ts).await }).await);
    }
}

/// Set a property only if it changed (avoids notify storms).
fn set_if<V: ToValue + PartialEq + for<'a> glib::value::FromValue<'a> + 'static>(obj: &impl IsA<glib::Object>, prop: &str, value: &V) {
    if obj.property::<V>(prop) != *value {
        obj.set_property(prop, value);
    }
}

pub fn texture(bytes: &[u8]) -> Option<gdk::Texture> {
    gdk::Texture::from_bytes(&glib::Bytes::from(bytes)).ok()
}

/// If a radio URL points to an m3u/pls playlist, return the first stream in it.
async fn resolve_radio(url: String, trust: bool) -> String {
    let Ok(client) = backend::http_client(trust) else { return url };
    let Ok(resp) = client.get(&url).timeout(std::time::Duration::from_secs(10)).send().await else { return url };
    let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
    if !(ct.contains("mpegurl") || ct.contains("text/plain") || ct.contains("scpls")) {
        return url;
    }
    let Ok(body) = resp.text().await else { return url };
    body.lines()
        .map(str::trim)
        .map(|l| l.split_once('=').filter(|(k, _)| k.to_lowercase().starts_with("file")).map_or(l, |(_, v)| v))
        .find(|l| !l.is_empty() && !l.starts_with('#') && l.contains("://"))
        .map(Into::into)
        .unwrap_or(url)
}
