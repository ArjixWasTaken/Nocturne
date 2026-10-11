//! OpenSubsonic / Navidrome backend.
use super::*;
use md5::{Digest, Md5};
use serde::Deserialize;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};

pub const KIND: &str = "NocturneIntegrationNavidrome";

pub struct Subsonic {
    conn: Connection,
    http: reqwest::Client,
    use_api_key: AtomicBool,
}

/// Salted MD5 token auth: `t = md5(password + salt)`.
pub fn token(password: &str, salt: &str) -> String {
    format!("{:x}", Md5::digest(format!("{password}{salt}")))
}

// ---- wire types (only the fields we use; everything optional) ----

fn de_id<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => s,
        Value::Null => String::new(),
        v => v.to_string(),
    })
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct WRef {
    #[serde(deserialize_with = "de_id")]
    id: String,
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct WGain {
    track_gain: f64,
    album_gain: f64,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct WSong {
    #[serde(deserialize_with = "de_id")]
    id: String,
    title: String,
    album: String,
    #[serde(deserialize_with = "de_id")]
    album_id: String,
    artist: String,
    #[serde(deserialize_with = "de_id")]
    artist_id: String,
    artists: Vec<WRef>,
    duration: i32,
    track: i32,
    disc_number: i32,
    year: i32,
    starred: Option<String>,
    user_rating: i32,
    cover_art: String,
    replay_gain: Option<WGain>,
    content_type: String,
    suffix: String,
    path: String,
    size: i64,
    bit_rate: i32,
    bit_depth: i32,
    sampling_rate: i32,
    channel_count: i32,
    bpm: i32,
    music_brainz_id: String,
    genres: Vec<WRef>,
}

impl From<WSong> for SongData {
    fn from(s: WSong) -> Self {
        let gain = s.replay_gain.unwrap_or_default();
        let artists = artists_or(s.artists, &s.artist_id, &s.artist);
        SongData {
            id: s.id,
            title: s.title,
            album: s.album,
            album_id: s.album_id,
            artist: s.artist,
            artist_id: s.artist_id,
            artists,
            duration: s.duration,
            track: s.track,
            disc_number: s.disc_number,
            year: s.year,
            starred: s.starred.is_some(),
            user_rating: s.user_rating,
            cover_art: s.cover_art,
            album_gain: gain.album_gain,
            track_gain: gain.track_gain,
            container: s.suffix,
            codec: s.content_type,
            ..Default::default()
        }
    }
}

fn artists_or(list: Vec<WRef>, id: &str, name: &str) -> Vec<Ref> {
    if list.is_empty() && !id.is_empty() {
        return vec![Ref { id: id.into(), name: name.into() }];
    }
    list.into_iter().map(|a| Ref { id: a.id, name: a.name }).collect()
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct WAlbum {
    #[serde(deserialize_with = "de_id")]
    id: String,
    name: String,
    artist: String,
    #[serde(deserialize_with = "de_id")]
    artist_id: String,
    artists: Vec<WRef>,
    song_count: i32,
    duration: i32,
    year: i32,
    starred: Option<String>,
    user_rating: i32,
    cover_art: String,
    song: Option<Vec<WSong>>,
}

impl From<WAlbum> for AlbumData {
    fn from(a: WAlbum) -> Self {
        AlbumData {
            artists: artists_or(a.artists, &a.artist_id, &a.artist),
            id: a.id,
            name: a.name,
            artist: a.artist,
            artist_id: a.artist_id,
            song_count: a.song_count,
            duration: a.duration,
            year: a.year,
            starred: a.starred.is_some(),
            user_rating: a.user_rating,
            cover_art: a.cover_art,
            songs: a.song.map(|s| s.into_iter().map(Into::into).collect()),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct WArtist {
    #[serde(deserialize_with = "de_id")]
    id: String,
    name: String,
    album_count: i32,
    starred: Option<String>,
    user_rating: i32,
    cover_art: String,
    album: Option<Vec<WAlbum>>,
}

impl From<WArtist> for ArtistData {
    fn from(a: WArtist) -> Self {
        ArtistData {
            id: a.id,
            name: a.name,
            album_count: a.album_count,
            starred: a.starred.is_some(),
            user_rating: a.user_rating,
            cover_art: a.cover_art,
            albums: a.album.map(|v| v.into_iter().map(Into::into).collect()),
            ..Default::default()
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct WPlaylist {
    #[serde(deserialize_with = "de_id")]
    id: String,
    name: String,
    song_count: i32,
    duration: i32,
    readonly: bool,
    cover_art: String,
    entry: Option<Vec<WSong>>,
}

impl From<WPlaylist> for PlaylistData {
    fn from(p: WPlaylist) -> Self {
        PlaylistData {
            id: p.id,
            name: p.name,
            song_count: p.song_count,
            duration: p.duration,
            readonly: p.readonly,
            cover_art: p.cover_art,
            entries: p.entry.map(|v| v.into_iter().map(Into::into).collect()),
        }
    }
}

fn get<T: serde::de::DeserializeOwned + Default>(v: &Value, path: &[&str]) -> T {
    let mut cur = v;
    for p in path {
        cur = &cur[*p];
    }
    serde_json::from_value(cur.clone()).unwrap_or_default()
}

fn songs(v: &Value, path: &[&str]) -> Vec<SongData> {
    get::<Vec<WSong>>(v, path).into_iter().map(Into::into).collect()
}

type Params<'a> = Vec<(&'a str, String)>;

impl Subsonic {
    pub fn new(conn: Connection) -> Result<Self> {
        Ok(Self { http: http_client(conn.trust_server)?, conn, use_api_key: AtomicBool::new(false) })
    }

    fn base(&self) -> String {
        self.conn.url.trim_end_matches('/').to_string()
    }

    fn auth_params(&self) -> Params<'static> {
        let mut p: Params = vec![("v", "1.16.1".into()), ("c", "Nocturne".into()), ("f", "json".into())];
        if self.use_api_key.load(Ordering::Relaxed) {
            p.push(("apiKey", self.conn.password.clone()));
        } else {
            let salt: String = {
                use rand::distr::{Alphanumeric, SampleString};
                Alphanumeric.sample_string(&mut rand::rng(), 12)
            };
            p.push(("u", self.conn.user.clone()));
            p.push(("t", token(&self.conn.password, &salt)));
            p.push(("s", salt));
        }
        p
    }

    fn url(&self, action: &str, params: &[(&str, String)]) -> String {
        let mut u = url::Url::parse(&format!("{}/rest/{action}", self.base())).expect("invalid server url");
        {
            let mut q = u.query_pairs_mut();
            for (k, v) in self.auth_params().iter().chain(params) {
                q.append_pair(k, v);
            }
        }
        u.into()
    }

    /// Returns the inner `subsonic-response` object; errors on `status: failed`.
    async fn call(&self, action: &str, params: &[(&str, String)]) -> Result<Value> {
        for _ in 0..2 {
            let mut v: Value = self.http.get(self.url(action, params)).send().await?.error_for_status()?.json().await?;
            let resp = v["subsonic-response"].take();
            if resp["status"] == "ok" {
                return Ok(resp);
            }
            let code = resp["error"]["code"].as_i64().unwrap_or(0);
            if matches!(code, 41 | 42) && !self.use_api_key.swap(true, Ordering::Relaxed) {
                continue; // retry with apiKey auth
            }
            bail!("{}", resp["error"]["message"].as_str().unwrap_or("Unknown Error"));
        }
        unreachable!()
    }

    fn folder_params(&self) -> Params<'static> {
        self.conn.library_ids.iter().map(|id| ("musicFolderId", id.clone())).collect()
    }

    async fn star_params(&self, id: &str) -> &'static str {
        // Subsonic needs to know the type; probe album/artist first.
        if self.call("getAlbum", &[("id", id.into())]).await.is_ok() {
            "albumId"
        } else if self.call("getArtist", &[("id", id.into())]).await.is_ok() {
            "artistId"
        } else {
            "id"
        }
    }
}

#[async_trait]
impl Backend for Subsonic {
    fn kind(&self) -> &'static str {
        KIND
    }

    async fn ping(&self) -> Result<()> {
        self.call("ping", &[]).await.map(drop)
    }

    async fn album_list(&self, kind: AlbumListKind, size: u32, offset: u32) -> Result<Vec<AlbumData>> {
        let t = match kind {
            AlbumListKind::Random => "random",
            AlbumListKind::Newest => "newest",
            AlbumListKind::Frequent => "frequent",
            AlbumListKind::Recent => "recent",
            AlbumListKind::Starred => "starred",
            AlbumListKind::AlphabeticalByName => "alphabeticalByName",
        };
        let mut p = vec![("type", t.to_string()), ("size", size.to_string()), ("offset", offset.to_string())];
        p.extend(self.folder_params());
        let r = self.call("getAlbumList2", &p).await?;
        Ok(get::<Vec<WAlbum>>(&r, &["albumList2", "album"]).into_iter().map(Into::into).collect())
    }

    async fn artists(&self, size: u32) -> Result<Vec<ArtistData>> {
        let r = self.call("getArtists", &self.folder_params()).await?;
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Index {
            artist: Vec<WArtist>,
        }
        let mut out: Vec<ArtistData> =
            get::<Vec<Index>>(&r, &["artists", "index"]).into_iter().flat_map(|i| i.artist).map(Into::into).collect();
        if size > 0 {
            out.truncate(size as usize);
        }
        Ok(out)
    }

    async fn playlists(&self) -> Result<Vec<PlaylistData>> {
        let r = self.call("getPlaylists", &[]).await?;
        Ok(get::<Vec<WPlaylist>>(&r, &["playlists", "playlist"]).into_iter().map(Into::into).collect())
    }

    async fn starred(&self) -> Result<Starred> {
        let r = self.call("getStarred2", &self.folder_params()).await?;
        Ok(Starred {
            artists: get::<Vec<WArtist>>(&r, &["starred2", "artist"]).into_iter().map(Into::into).collect(),
            albums: get::<Vec<WAlbum>>(&r, &["starred2", "album"]).into_iter().map(Into::into).collect(),
            songs: songs(&r, &["starred2", "song"]),
        })
    }

    async fn artist(&self, id: &str) -> Result<ArtistData> {
        let p = [("id", id.to_string())];
        let (base, info) = tokio::join!(self.call("getArtist", &p), self.call("getArtistInfo2", &p));
        let mut a: ArtistData = get::<WArtist>(&base?, &["artist"]).into();
        if let Ok(info) = info {
            a.biography = info["artistInfo2"]["biography"].as_str().unwrap_or_default().to_string();
            a.similar = get::<Vec<WRef>>(&info, &["artistInfo2", "similarArtist"])
                .into_iter()
                .map(|r| Ref { id: r.id, name: r.name })
                .collect();
        }
        Ok(a)
    }

    async fn album(&self, id: &str) -> Result<AlbumData> {
        let r = self.call("getAlbum", &[("id", id.into())]).await?;
        Ok(get::<WAlbum>(&r, &["album"]).into())
    }

    async fn playlist(&self, id: &str) -> Result<PlaylistData> {
        let r = self.call("getPlaylist", &[("id", id.into())]).await?;
        Ok(get::<WPlaylist>(&r, &["playlist"]).into())
    }

    async fn song(&self, id: &str) -> Result<SongData> {
        let r = self.call("getSong", &[("id", id.into())]).await?;
        Ok(get::<WSong>(&r, &["song"]).into())
    }

    async fn search(&self, query: &str, c: SearchCounts) -> Result<SearchResults> {
        let mut p = vec![
            ("query", query.to_string()),
            ("artistCount", c.artist.0.to_string()),
            ("artistOffset", c.artist.1.to_string()),
            ("albumCount", c.album.0.to_string()),
            ("albumOffset", c.album.1.to_string()),
            ("songCount", c.song.0.to_string()),
            ("songOffset", c.song.1.to_string()),
        ];
        p.extend(self.folder_params());
        let r = self.call("search3", &p).await?;
        let mut res = SearchResults {
            artists: get::<Vec<WArtist>>(&r, &["searchResult3", "artist"]).into_iter().map(Into::into).collect(),
            albums: get::<Vec<WAlbum>>(&r, &["searchResult3", "album"]).into_iter().map(Into::into).collect(),
            songs: songs(&r, &["searchResult3", "song"]),
            playlists: vec![],
        };
        if c.playlist.0 > 0 {
            let q = query.to_lowercase();
            res.playlists = self
                .playlists()
                .await?
                .into_iter()
                .filter(|p| p.name.to_lowercase().contains(&q))
                .skip(c.playlist.1 as usize)
                .take(c.playlist.0 as usize)
                .collect();
        }
        Ok(res)
    }

    fn stream_url(&self, song: &str, max_bitrate: i32) -> String {
        let mut p = vec![("id", song.to_string())];
        if max_bitrate > 0 {
            p.push(("maxBitRate", max_bitrate.to_string()));
        }
        self.url("stream", &p)
    }

    async fn cover_art(&self, id: &str, size: u32) -> Result<Option<Vec<u8>>> {
        let resp = self.http.get(self.url("getCoverArt", &[("id", id.into()), ("size", size.to_string())])).send().await?;
        let is_image = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|t| t.starts_with("image/"));
        if !resp.status().is_success() || !is_image {
            return Ok(None);
        }
        Ok(Some(resp.bytes().await?.to_vec()))
    }

    fn cover_art_url(&self, id: &str) -> String {
        self.url("getCoverArt", &[("id", id.into()), ("size", "600".into())])
    }

    async fn set_starred(&self, id: &str, starred: bool) -> Result<()> {
        let key = self.star_params(id).await;
        self.call(if starred { "star" } else { "unstar" }, &[(key, id.into())]).await.map(drop)
    }

    async fn set_rating(&self, id: &str, rating: i32) -> Result<()> {
        self.call("setRating", &[("id", id.into()), ("rating", rating.to_string())]).await.map(drop)
    }

    async fn scrobble(&self, id: &str, submission: bool) -> Result<()> {
        self.call("scrobble", &[("id", id.into()), ("submission", submission.to_string())]).await.map(drop)
    }

    async fn random_songs(&self, size: u32) -> Result<Vec<SongData>> {
        let mut p = vec![("size", size.to_string())];
        p.extend(self.folder_params());
        Ok(songs(&self.call("getRandomSongs", &p).await?, &["randomSongs", "song"]))
    }

    async fn similar_songs(&self, id: &str, count: u32) -> Result<Vec<SongData>> {
        let r = self.call("getSimilarSongs2", &[("id", id.into()), ("count", count.to_string())]).await;
        match r {
            Ok(r) if !songs(&r, &["similarSongs2", "song"]).is_empty() => Ok(songs(&r, &["similarSongs2", "song"])),
            _ => self.random_songs(count).await,
        }
    }

    async fn top_songs(&self, artist_id: &str, count: u32) -> Result<Vec<SongData>> {
        let name = self.artist(artist_id).await?.name;
        let r = self.call("getTopSongs", &[("artist", name), ("count", count.to_string())]).await?;
        Ok(songs(&r, &["topSongs", "song"]))
    }

    async fn play_queue(&self) -> Result<PlayQueue> {
        let r = self.call("getPlayQueue", &[]).await?;
        let songs = songs(&r, &["playQueue", "entry"]);
        Ok(PlayQueue {
            current: r["playQueue"]["current"].as_str().map(Into::into).or_else(|| {
                r["playQueue"]["current"].as_i64().map(|i| i.to_string())
            }),
            ids: songs.iter().map(|s| s.id.clone()).collect(),
            songs,
            position_ms: r["playQueue"]["position"].as_i64().unwrap_or(0),
        })
    }

    async fn save_play_queue(&self, ids: &[String], current: &str, position_ms: i64) -> Result<()> {
        let mut p: Params = ids.iter().map(|id| ("id", id.clone())).collect();
        p.push(("current", current.into()));
        p.push(("position", position_ms.to_string()));
        self.call("savePlayQueue", &p).await.map(drop)
    }

    async fn lyrics(&self, id: &str) -> Result<Option<Lyrics>> {
        let r = self.call("getLyricsBySongId", &[("id", id.into())]).await?;
        let first = &r["lyricsList"]["structuredLyrics"][0];
        let Some(lines) = first["line"].as_array() else { return Ok(None) };
        if first["synced"].as_bool().unwrap_or(false) {
            let content = lines
                .iter()
                .map(|l| {
                    let ms = l["start"].as_i64().unwrap_or(0);
                    format!("[{:02}:{:02}.{:02}] {}", ms / 60000, (ms / 1000) % 60, (ms % 1000) / 10, l["value"].as_str().unwrap_or("").trim())
                })
                .collect::<Vec<_>>()
                .join("\n");
            Ok(Some(Lyrics { kind: LyricsKind::Synced, content }))
        } else {
            let content = lines.iter().map(|l| l["value"].as_str().unwrap_or("")).collect::<Vec<_>>().join("\n");
            Ok(Some(Lyrics { kind: LyricsKind::Plain, content }))
        }
    }

    async fn radios(&self) -> Result<Vec<SongData>> {
        let r = self.call("getInternetRadioStations", &[]).await?;
        #[derive(Deserialize, Default)]
        #[serde(default, rename_all = "camelCase")]
        struct WRadio {
            #[serde(deserialize_with = "de_id")]
            id: String,
            name: String,
            stream_url: String,
        }
        Ok(get::<Vec<WRadio>>(&r, &["internetRadioStations", "internetRadioStation"])
            .into_iter()
            .map(|r| SongData { id: r.id, title: r.name, radio_stream_url: r.stream_url, duration: -1, ..Default::default() })
            .collect())
    }

    async fn create_radio(&self, name: &str, url: &str) -> Result<()> {
        self.call("createInternetRadioStation", &[("name", name.into()), ("streamUrl", url.into())]).await.map(drop)
    }

    async fn update_radio(&self, id: &str, name: &str, url: &str) -> Result<()> {
        self.call("updateInternetRadioStation", &[("id", id.into()), ("name", name.into()), ("streamUrl", url.into())])
            .await
            .map(drop)
    }

    async fn delete_radio(&self, id: &str) -> Result<()> {
        self.call("deleteInternetRadioStation", &[("id", id.into())]).await.map(drop)
    }

    async fn create_playlist(&self, name: &str, songs: &[String]) -> Result<String> {
        let mut p: Params = songs.iter().map(|id| ("songId", id.clone())).collect();
        p.push(("name", name.into()));
        let r = self.call("createPlaylist", &p).await?;
        Ok(get::<WPlaylist>(&r, &["playlist"]).id)
    }

    async fn update_playlist(&self, id: &str, name: Option<&str>, add: &[String], remove: &[u32]) -> Result<()> {
        let mut p: Params = vec![("playlistId", id.into())];
        if let Some(n) = name {
            p.push(("name", n.into()));
        }
        p.extend(add.iter().map(|s| ("songIdToAdd", s.clone())));
        p.extend(remove.iter().map(|i| ("songIndexToRemove", i.to_string())));
        self.call("updatePlaylist", &p).await.map(drop)
    }

    async fn delete_playlist(&self, id: &str) -> Result<()> {
        self.call("deletePlaylist", &[("id", id.into())]).await.map(drop)
    }

    async fn libraries(&self) -> Result<(bool, Vec<Ref>)> {
        let r = self.call("getMusicFolders", &[]).await?;
        Ok((true, get::<Vec<WRef>>(&r, &["musicFolders", "musicFolder"]).into_iter().map(|f| Ref { id: f.id, name: f.name }).collect()))
    }

    async fn song_details(&self, id: &str) -> Result<Vec<(String, String)>> {
        let r = self.call("getSong", &[("id", id.into())]).await?;
        let s: WSong = get(&r, &["song"]);
        let gain = s.replay_gain.as_ref().map(|g| (g.track_gain, g.album_gain)).unwrap_or_default();
        let rows = [
            ("Album", s.album.clone()),
            ("Display Artist", s.artist.clone()),
            ("Artists", s.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ")),
            ("Genres", s.genres.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ")),
            ("Track Number", s.track.to_string()),
            ("Disc Number", s.disc_number.to_string()),
            ("Year", s.year.to_string()),
            ("Duration", crate::config::display_time(s.duration.into())),
            ("Suffix", s.suffix.clone()),
            ("Size", glib_size(s.size)),
            ("Bit Rate", format!("{} kbps", s.bit_rate)),
            ("Bit Depth", s.bit_depth.to_string()),
            ("Sampling Rate", format!("{} Hz", s.sampling_rate)),
            ("Channel Count", s.channel_count.to_string()),
            ("BPM", s.bpm.to_string()),
            ("Track Gain", gain.0.to_string()),
            ("Album Gain", gain.1.to_string()),
            ("MusicBrainz ID", s.music_brainz_id.clone()),
            ("Path", s.path.clone()),
        ];
        Ok(rows.into_iter().filter(|(_, v)| !v.is_empty() && v != "0").map(|(k, v)| (k.to_string(), v)).collect())
    }

    async fn server_info(&self) -> Result<ServerInfo> {
        let ping = self.call("ping", &[]).await?;
        let avatar = self.http.get(self.url("getAvatar", &[("username", self.conn.user.clone())])).send().await.ok();
        let avatar = match avatar {
            Some(r) if r.status().is_success() => r.bytes().await.ok().map(|b| b.to_vec()).filter(|b| !b.is_empty()),
            _ => None,
        };
        Ok(ServerInfo {
            username: self.conn.user.clone(),
            title: format!("{} {}", ping["type"].as_str().unwrap_or("Subsonic"), ping["serverVersion"].as_str().unwrap_or("")),
            link: self.base(),
            avatar,
        })
    }

    async fn download(&self, id: &str, dest: &std::path::Path) -> Result<()> {
        let bytes = self.http.get(self.url("download", &[("id", id.into())])).send().await?.error_for_status()?.bytes().await?;
        tokio::fs::write(dest, bytes).await?;
        Ok(())
    }
}

fn glib_size(bytes: i64) -> String {
    if bytes <= 0 { String::new() } else { format!("{:.1} MB", bytes as f64 / 1_048_576.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_token() {
        // Example from the Subsonic API docs.
        assert_eq!(token("sesame", "c19b2d"), "26719a1196d2a940705a59634eb18eab");
    }

    #[test]
    fn parses_album() {
        let v = serde_json::json!({"album": {"id": 5, "name": "A", "artistId": "ar", "artist": "X",
            "song": [{"id": "s1", "title": "T", "replayGain": {"trackGain": -3.5}, "starred": "2020"}]}});
        let a: AlbumData = get::<WAlbum>(&v, &["album"]).into();
        assert_eq!(a.id, "5");
        assert_eq!(a.artists[0].name, "X");
        let s = &a.songs.unwrap()[0];
        assert!(s.starred);
        assert_eq!(s.track_gain, -3.5);
    }
}
