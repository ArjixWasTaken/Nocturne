//! Local files backend: scans `library_dir`, indexes tags in memory.
use super::*;
use lofty::prelude::*;
use lofty::tag::ItemKey;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub const KIND: &str = "NocturneIntegrationLocal";
pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "oga", "ogg", "opus", "wav", "mp4", "mov", "m4v", "avi", "webm", "mkv"];

#[derive(Default)]
struct Library {
    songs: HashMap<String, SongData>,
    song_order: Vec<String>,
    /// album id -> (data, song ids)
    albums: HashMap<String, (AlbumData, Vec<String>)>,
    album_order: Vec<String>,
    /// artist id -> (data, album ids)
    artists: HashMap<String, (ArtistData, Vec<String>)>,
    artist_order: Vec<String>,
    album_artists: HashSet<String>,
}

impl Library {
    fn add(&mut self, mut song: SongData) {
        let id = song.id.clone();
        if song.album_id.is_empty() {
            song.album_id = format!("ALBUM:NO_ALBUM:{}", song.artists.first().map(|a| a.id.as_str()).unwrap_or(""));
        }
        let album_id = song.album_id.clone();
        let (album, ids) = self.albums.entry(album_id.clone()).or_insert_with(|| {
            self.album_order.push(album_id.clone());
            let a = AlbumData {
                id: album_id.clone(),
                name: if song.album.is_empty() { "No Album".into() } else { song.album.clone() },
                artist: song.artist.clone(),
                artist_id: song.artist_id.clone(),
                artists: if song.artist_id.is_empty() { vec![] } else { vec![Ref { id: song.artist_id.clone(), name: song.artist.clone() }] },
                year: song.year,
                cover_art: song.path.clone(),
                ..Default::default()
            };
            (a, vec![])
        });
        if !ids.contains(&id) {
            ids.push(id.clone());
            album.song_count += 1;
            album.duration += song.duration;
        }
        let mut credit = |aid: &str, name: &str| {
            let (artist, albums) = self.artists.entry(aid.to_string()).or_insert_with(|| {
                self.artist_order.push(aid.to_string());
                (ArtistData { id: aid.into(), name: name.into(), cover_art: song.path.clone(), ..Default::default() }, vec![])
            });
            if !albums.contains(&album_id) {
                albums.push(album_id.clone());
                artist.album_count += 1;
            }
        };
        if !song.artist_id.is_empty() {
            credit(&song.artist_id, &song.artist);
            self.album_artists.insert(song.artist_id.clone());
        }
        for a in &song.artists {
            if !a.id.is_empty() {
                credit(&a.id, &a.name);
            }
        }
        if !self.songs.contains_key(&id) {
            self.song_order.push(id.clone());
        }
        self.songs.insert(id, song);
    }
}

#[derive(Default)]
struct Marks {
    stars: HashSet<String>,
    ratings: HashMap<String, i32>,
}

impl Marks {
    fn song(&self, mut s: SongData) -> SongData {
        s.starred = self.stars.contains(&s.id);
        s.user_rating = self.ratings.get(&s.id).copied().unwrap_or(0);
        s
    }
    fn album(&self, mut a: AlbumData) -> AlbumData {
        a.starred = self.stars.contains(&a.id);
        a.user_rating = self.ratings.get(&a.id).copied().unwrap_or(0);
        a
    }
    fn artist(&self, mut a: ArtistData) -> ArtistData {
        a.starred = self.stars.contains(&a.id);
        a.user_rating = self.ratings.get(&a.id).copied().unwrap_or(0);
        a
    }
}

pub struct Local {
    conn: Connection,
    lib: RwLock<Library>,
    db: tokio::sync::OnceCell<crate::db::Db>,
}

fn dir() -> PathBuf {
    crate::config::integrations_dir().join(KIND)
}

async fn read_json(name: &str) -> Value {
    tokio::fs::read(dir().join(name)).await.ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}))
}

async fn write_json(name: &str, v: &Value) -> Result<()> {
    tokio::fs::create_dir_all(dir()).await?;
    tokio::fs::write(dir().join(name), serde_json::to_vec(v)?).await?;
    Ok(())
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn matches(q: &str, fields: &[&str]) -> bool {
    fields.join(" ").to_lowercase().contains(q)
}

fn page<T>(v: impl IntoIterator<Item = T>, (count, offset): (u32, u32)) -> Vec<T> {
    v.into_iter().skip(offset as usize).take(count as usize).collect()
}

impl Local {
    pub fn new(conn: Connection) -> Result<Self> {
        Ok(Self { conn, lib: Default::default(), db: Default::default() })
    }

    async fn db(&self) -> Result<&crate::db::Db> {
        self.db.get_or_try_init(|| crate::db::open_for(KIND)).await
    }

    async fn marks(&self) -> Marks {
        let Ok(db) = self.db().await else { return Marks::default() };
        Marks {
            stars: sqlx::query_scalar::<_, String>("SELECT id FROM stars").fetch_all(db).await.unwrap_or_default().into_iter().collect(),
            ratings: sqlx::query_as::<_, (String, i32)>("SELECT id, rating FROM ratings").fetch_all(db).await.unwrap_or_default().into_iter().collect(),
        }
    }

    fn scan(root: &Path) -> Library {
        let mut lib = Library::default();
        let walker = walkdir::WalkDir::new(root).sort_by_file_name().into_iter().filter_entry(|e| {
            e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.')
        });
        for e in walker.flatten().filter(|e| e.file_type().is_file()) {
            let ext = e.path().extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            if let Some(mut s) = read_song(e.path(), true) {
                s.id = format!("SONG:{}", e.path().display());
                lib.add(s);
            }
        }
        lib
    }

    fn song_path(&self, id: &str) -> Option<String> {
        self.lib.read().unwrap().songs.get(id).map(|s| s.path.clone())
    }

    /// Path whose tags/folder provide the cover of a model id (or a raw path).
    fn cover_path(&self, id: &str) -> Option<String> {
        let lib = self.lib.read().unwrap();
        if let Some(s) = lib.songs.get(id) {
            return Some(s.path.clone());
        }
        if let Some((a, _)) = lib.albums.get(id) {
            return Some(a.cover_art.clone());
        }
        if let Some((a, _)) = lib.artists.get(id) {
            return Some(a.cover_art.clone());
        }
        (!id.contains(':') || id.starts_with('/')).then(|| id.to_string())
    }

    fn album_full(&self, id: &str, m: &Marks) -> Option<AlbumData> {
        let lib = self.lib.read().unwrap();
        let (a, ids) = lib.albums.get(id)?;
        let mut songs: Vec<SongData> = ids.iter().filter_map(|i| lib.songs.get(i)).map(|s| m.song(s.clone())).collect();
        songs.sort_by_key(|s| (s.disc_number, s.track));
        let mut a = m.album(a.clone());
        a.songs = Some(songs);
        Some(a)
    }

    fn albums_by_id(&self, ids: impl IntoIterator<Item = String>, m: &Marks) -> Vec<AlbumData> {
        let lib = self.lib.read().unwrap();
        ids.into_iter().filter_map(|i| lib.albums.get(&i).map(|(a, _)| m.album(a.clone()))).collect()
    }

    fn songs_by_id(&self, ids: impl IntoIterator<Item = String>, m: &Marks) -> Vec<SongData> {
        let lib = self.lib.read().unwrap();
        ids.into_iter().filter_map(|i| lib.songs.get(&i).map(|s| m.song(s.clone()))).collect()
    }

    fn playlist_data(&self, id: &str, p: &Value, m: Option<&Marks>) -> PlaylistData {
        let ids: Vec<String> = serde_json::from_value(p["songId"].clone()).unwrap_or_default();
        let entries = m.map(|m| self.songs_by_id(ids.clone(), m));
        PlaylistData {
            id: id.into(),
            name: p["name"].as_str().unwrap_or_default().into(),
            song_count: ids.len() as i32,
            duration: entries.as_ref().map(|e| e.iter().map(|s| s.duration).sum()).unwrap_or(0),
            cover_art: ids.first().and_then(|i| self.song_path(i)).unwrap_or_default(),
            entries,
            ..Default::default()
        }
    }

    async fn scrobble_rows(&self) -> Vec<(String, i64, f64, Option<String>, Option<String>)> {
        let Ok(db) = self.db().await else { return vec![] };
        sqlx::query_as("SELECT id, plays, last_play, album_id, artist_id FROM scrobble").fetch_all(db).await.unwrap_or_default()
    }
}

#[async_trait]
impl Backend for Local {
    fn kind(&self) -> &'static str {
        KIND
    }

    async fn ping(&self) -> Result<()> {
        let root = PathBuf::from(&self.conn.library_dir);
        if !root.is_dir() {
            bail!("Could not locate path, try again");
        }
        let lib = tokio::task::spawn_blocking(move || Self::scan(&root)).await?;
        *self.lib.write().unwrap() = lib;
        self.db().await?;
        Ok(())
    }

    async fn album_list(&self, kind: AlbumListKind, size: u32, offset: u32) -> Result<Vec<AlbumData>> {
        let m = self.marks().await;
        let mut ids: Vec<String> = self.lib.read().unwrap().album_order.clone();
        match kind {
            AlbumListKind::Random => {
                use rand::seq::SliceRandom;
                ids.shuffle(&mut rand::rng());
            }
            AlbumListKind::Newest => {
                let lib = self.lib.read().unwrap();
                let time = |id: &String| {
                    lib.albums.get(id).and_then(|(a, _)| std::fs::metadata(&a.cover_art).ok()).and_then(|m| m.modified().ok())
                };
                let mut keyed: Vec<_> = ids.drain(..).map(|i| (time(&i), i)).collect();
                keyed.sort_by(|a, b| b.0.cmp(&a.0));
                ids = keyed.into_iter().map(|x| x.1).collect();
            }
            AlbumListKind::Frequent | AlbumListKind::Recent => {
                let mut views: HashMap<String, (i64, f64)> = HashMap::new();
                for (_, plays, last, album, _) in self.scrobble_rows().await {
                    let e = views.entry(album.unwrap_or_default()).or_default();
                    e.0 += plays;
                    e.1 = e.1.max(last);
                }
                let mut v: Vec<_> = views.into_iter().collect();
                if kind == AlbumListKind::Frequent {
                    v.sort_by(|a, b| b.1.0.cmp(&a.1.0));
                } else {
                    v.sort_by(|a, b| b.1.1.total_cmp(&a.1.1));
                }
                ids = v.into_iter().map(|x| x.0).collect();
            }
            AlbumListKind::Starred => ids.retain(|i| m.stars.contains(i)),
            AlbumListKind::AlphabeticalByName => {}
        }
        Ok(page(self.albums_by_id(ids, &m), (size, offset)))
    }

    async fn artists(&self, size: u32) -> Result<Vec<ArtistData>> {
        let m = self.marks().await;
        let lib = self.lib.read().unwrap();
        let it = lib.artist_order.iter().filter(|i| lib.album_artists.contains(*i)).map(|i| m.artist(lib.artists[i].0.clone()));
        Ok(if size > 0 { it.take(size as usize).collect() } else { it.collect() })
    }

    async fn playlists(&self) -> Result<Vec<PlaylistData>> {
        let v = read_json("playlists.json").await;
        Ok(v.as_object().map(|o| o.iter().map(|(id, p)| self.playlist_data(id, p, None)).collect()).unwrap_or_default())
    }

    async fn starred(&self) -> Result<Starred> {
        let m = self.marks().await;
        let lib = self.lib.read().unwrap();
        let mut stars: Vec<&String> = m.stars.iter().collect();
        stars.sort();
        Ok(Starred {
            artists: stars.iter().filter_map(|i| lib.artists.get(*i)).map(|a| m.artist(a.0.clone())).collect(),
            albums: stars.iter().filter_map(|i| lib.albums.get(*i)).map(|a| m.album(a.0.clone())).collect(),
            songs: stars.iter().filter_map(|i| lib.songs.get(*i)).map(|s| m.song(s.clone())).collect(),
        })
    }

    async fn artist(&self, id: &str) -> Result<ArtistData> {
        let m = self.marks().await;
        let (a, albums) = self.lib.read().unwrap().artists.get(id).cloned().ok_or_else(|| anyhow::anyhow!("artist {id} not found"))?;
        let mut a = m.artist(a);
        a.albums = Some(self.albums_by_id(albums, &m));
        Ok(a)
    }

    async fn album(&self, id: &str) -> Result<AlbumData> {
        let m = self.marks().await;
        self.album_full(id, &m).ok_or_else(|| anyhow::anyhow!("album {id} not found"))
    }

    async fn playlist(&self, id: &str) -> Result<PlaylistData> {
        let v = read_json("playlists.json").await;
        if v[id].is_null() {
            bail!("playlist {id} not found");
        }
        Ok(self.playlist_data(id, &v[id], Some(&self.marks().await)))
    }

    async fn song(&self, id: &str) -> Result<SongData> {
        let m = self.marks().await;
        self.songs_by_id([id.to_string()], &m).pop().ok_or_else(|| anyhow::anyhow!("song {id} not found"))
    }

    async fn search(&self, query: &str, c: SearchCounts) -> Result<SearchResults> {
        let q = query.to_lowercase();
        let m = self.marks().await;
        let playlists = self.playlists().await?;
        let lib = self.lib.read().unwrap();
        Ok(SearchResults {
            artists: page(
                lib.artist_order.iter().filter(|i| lib.album_artists.contains(*i)).map(|i| &lib.artists[i].0).filter(|a| matches(&q, &[&a.name])).map(|a| m.artist(a.clone())),
                c.artist,
            ),
            albums: page(
                lib.album_order.iter().map(|i| &lib.albums[i].0).filter(|a| matches(&q, &[&a.name, &a.artist])).map(|a| m.album(a.clone())),
                c.album,
            ),
            songs: page(
                lib.song_order.iter().map(|i| &lib.songs[i]).filter(|s| matches(&q, &[&s.title, &s.artist, &s.album])).map(|s| m.song(s.clone())),
                c.song,
            ),
            playlists: page(playlists.into_iter().filter(|p| matches(&q, &[&p.name])), c.playlist),
        })
    }

    fn stream_url(&self, song: &str, _max_bitrate: i32) -> String {
        let path = self.song_path(song).unwrap_or_else(|| song.trim_start_matches("SONG:").to_string());
        url::Url::from_file_path(&path).map(Into::into).unwrap_or_else(|_| format!("file://{path}"))
    }

    async fn cover_art(&self, id: &str, _size: u32) -> Result<Option<Vec<u8>>> {
        let Some(path) = self.cover_path(id).filter(|p| !p.is_empty()) else { return Ok(None) };
        Ok(tokio::task::spawn_blocking(move || cover_from_file(Path::new(&path))).await?)
    }

    fn cover_art_url(&self, id: &str) -> String {
        // Folder images can be referenced directly; embedded ones have no URL.
        self.cover_path(id)
            .and_then(|p| folder_cover(Path::new(&p)))
            .and_then(|p| url::Url::from_file_path(p).ok())
            .map(Into::into)
            .unwrap_or_default()
    }

    async fn set_starred(&self, id: &str, starred: bool) -> Result<()> {
        let q = if starred { "INSERT OR IGNORE INTO stars (id) VALUES (?)" } else { "DELETE FROM stars WHERE id = ?" };
        sqlx::query(q).bind(id).execute(self.db().await?).await?;
        Ok(())
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

    async fn scrobble(&self, id: &str, submission: bool) -> Result<()> {
        let Some(s) = self.lib.read().unwrap().songs.get(id).cloned() else { return Ok(()) };
        if submission {
            sqlx::query(
                "INSERT INTO scrobble (id, plays, last_play, album_id, artist_id) VALUES (?, 1, ?, ?, ?)
                 ON CONFLICT (id) DO UPDATE SET plays = plays + 1, last_play = excluded.last_play,
                 album_id = excluded.album_id, artist_id = excluded.artist_id",
            )
            .bind(id)
            .bind(now())
            .bind(&s.album_id)
            .bind(&s.artist_id)
            .execute(self.db().await?)
            .await?;
        }
        Ok(())
    }

    async fn random_songs(&self, size: u32) -> Result<Vec<SongData>> {
        let m = self.marks().await;
        let ids: Vec<String> = {
            use rand::seq::IndexedRandom;
            let lib = self.lib.read().unwrap();
            lib.song_order.choose_multiple(&mut rand::rng(), size as usize).cloned().collect()
        };
        Ok(self.songs_by_id(ids, &m))
    }

    async fn top_songs(&self, artist_id: &str, count: u32) -> Result<Vec<SongData>> {
        let mut rows: Vec<(String, i64)> = {
            let rows = self.scrobble_rows().await;
            let lib = self.lib.read().unwrap();
            rows.into_iter()
                .filter(|(id, _, _, _, artist)| {
                    let a = artist.clone().filter(|a| !a.is_empty()).or_else(|| lib.songs.get(id).map(|s| s.artist_id.clone()));
                    a.as_deref() == Some(artist_id)
                })
                .map(|r| (r.0, r.1))
                .collect()
        };
        rows.sort_by(|a, b| b.1.cmp(&a.1));
        let m = self.marks().await;
        Ok(self.songs_by_id(rows.into_iter().take(count as usize).map(|r| r.0), &m))
    }

    async fn play_queue(&self) -> Result<PlayQueue> {
        let v = read_json("queue.json").await;
        let lib = self.lib.read().unwrap();
        let ids: Vec<String> = serde_json::from_value::<Vec<String>>(v["id"].clone()).unwrap_or_default().into_iter().filter(|i| lib.songs.contains_key(i)).collect();
        let cur = v["current"].as_str().filter(|c| ids.iter().any(|i| i == c)).map(String::from).or_else(|| ids.first().cloned());
        Ok(PlayQueue { current: cur, ids, songs: vec![], position_ms: v["position"].as_i64().unwrap_or(0) })
    }

    async fn save_play_queue(&self, ids: &[String], current: &str, position_ms: i64) -> Result<()> {
        let cur = if ids.iter().any(|i| i == current) { current.to_string() } else { ids.first().cloned().unwrap_or_default() };
        write_json("queue.json", &json!({"id": ids, "current": cur, "position": position_ms})).await
    }

    async fn lyrics(&self, id: &str) -> Result<Option<Lyrics>> {
        let Some(path) = self.song_path(id) else { return Ok(None) };
        let text = tokio::task::spawn_blocking(move || {
            let f = lofty::read_from_path(&path).ok()?;
            f.tags().iter().find_map(|t| t.get_string(&ItemKey::Lyrics).map(String::from))
        })
        .await?;
        Ok(text.filter(|t| !t.trim().is_empty()).map(|content| Lyrics {
            kind: if content.trim_start().starts_with('[') { LyricsKind::Synced } else { LyricsKind::Plain },
            content,
        }))
    }

    async fn radios(&self) -> Result<Vec<SongData>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT id, name, stream_url FROM radios").fetch_all(self.db().await?).await?;
        Ok(rows.into_iter().map(|(id, title, url)| SongData { id, title, radio_stream_url: url, duration: -1, ..Default::default() }).collect())
    }

    async fn create_radio(&self, name: &str, url: &str) -> Result<()> {
        self.update_radio(&format!("RADIO:{}", uuid::Uuid::new_v4()), name, url).await
    }

    async fn update_radio(&self, id: &str, name: &str, url: &str) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO radios (id, name, stream_url) VALUES (?, ?, ?)").bind(id).bind(name).bind(url).execute(self.db().await?).await?;
        Ok(())
    }

    async fn delete_radio(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM radios WHERE id = ?").bind(id).execute(self.db().await?).await?;
        Ok(())
    }

    async fn create_playlist(&self, name: &str, songs: &[String]) -> Result<String> {
        let id = format!("PLAYLIST:{}", uuid::Uuid::new_v4());
        let mut v = read_json("playlists.json").await;
        v[&id] = json!({"name": name, "songId": songs});
        write_json("playlists.json", &v).await?;
        Ok(id)
    }

    async fn update_playlist(&self, id: &str, name: Option<&str>, add: &[String], remove: &[u32]) -> Result<()> {
        let mut v = read_json("playlists.json").await;
        if v[id].is_null() {
            bail!("playlist {id} not found");
        }
        let mut songs: Vec<String> = serde_json::from_value(v[id]["songId"].clone()).unwrap_or_default();
        let mut remove = remove.to_vec();
        remove.sort_unstable_by(|a, b| b.cmp(a));
        for i in remove {
            if (i as usize) < songs.len() {
                songs.remove(i as usize);
            }
        }
        songs.extend_from_slice(add);
        v[id]["songId"] = json!(songs);
        if let Some(n) = name {
            v[id]["name"] = json!(n);
        }
        write_json("playlists.json", &v).await
    }

    async fn delete_playlist(&self, id: &str) -> Result<()> {
        let mut v = read_json("playlists.json").await;
        if let Some(o) = v.as_object_mut() {
            o.remove(id);
        }
        write_json("playlists.json", &v).await
    }

    async fn song_details(&self, id: &str) -> Result<Vec<(String, String)>> {
        let s = self.song(id).await?;
        let path = s.path.clone();
        let (props, genre, size) = tokio::task::spawn_blocking(move || {
            let f = lofty::read_from_path(&path).ok();
            let genre = f.as_ref().and_then(|f| f.primary_tag().or(f.first_tag())).and_then(|t| t.genre().map(|g| g.to_string()));
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            (f.map(|f| f.properties().clone()), genre, size)
        })
        .await?;
        let ext = Path::new(&s.path).extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        let rows = [
            ("Album", s.album.clone()),
            ("Display Artist", s.artist.clone()),
            ("Artists", s.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ")),
            ("Genres", genre.unwrap_or_default()),
            ("Track Number", s.track.to_string()),
            ("Disc Number", s.disc_number.to_string()),
            ("Year", s.year.to_string()),
            ("Duration", crate::config::display_time(s.duration.into())),
            ("Suffix", ext),
            ("Size", if size > 0 { format!("{:.1} MB", size as f64 / 1_048_576.0) } else { String::new() }),
            ("Bit Rate", props.as_ref().and_then(|p| p.audio_bitrate()).map(|b| format!("{b} kbps")).unwrap_or_default()),
            ("Bit Depth", props.as_ref().and_then(|p| p.bit_depth()).map(|b| b.to_string()).unwrap_or_default()),
            ("Sampling Rate", props.as_ref().and_then(|p| p.sample_rate()).map(|b| format!("{b} Hz")).unwrap_or_default()),
            ("Channel Count", props.as_ref().and_then(|p| p.channels()).map(|b| b.to_string()).unwrap_or_default()),
            ("Path", s.path.clone()),
        ];
        Ok(rows.into_iter().filter(|(_, v)| !v.is_empty() && v != "0").map(|(k, v)| (k.to_string(), v)).collect())
    }

    async fn server_info(&self) -> Result<ServerInfo> {
        Ok(ServerInfo {
            username: std::env::var("USER").unwrap_or_default(),
            title: "Local Files".into(),
            link: format!("file://{}", self.conn.library_dir),
            avatar: None,
        })
    }

    async fn download(&self, id: &str, dest: &Path) -> Result<()> {
        let path = self.song_path(id).ok_or_else(|| anyhow::anyhow!("song {id} not found"))?;
        tokio::fs::copy(path, dest).await?;
        Ok(())
    }
}

fn split_artists(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for v in values {
        for a in v.split(';').map(str::trim).filter(|a| !a.is_empty()) {
            if !out.iter().any(|x| x == a) {
                out.push(a.to_string());
            }
        }
    }
    out
}

fn parse_year(s: &str) -> i32 {
    s.as_bytes()
        .windows(4)
        .enumerate()
        .find(|(i, w)| {
            w.iter().all(u8::is_ascii_digit)
                && (*i == 0 || !s.as_bytes()[i - 1].is_ascii_digit())
                && s.as_bytes().get(i + 4).is_none_or(|c| !c.is_ascii_digit())
        })
        .and_then(|(i, _)| s[i..i + 4].parse().ok())
        .unwrap_or(0)
}

fn parse_gain(s: Option<&str>) -> f64 {
    s.and_then(|s| s.split_whitespace().next()).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn read_song(path: &Path, with_ids: bool) -> Option<SongData> {
    let file = lofty::read_from_path(path).ok()?;
    let tag = file.primary_tag().or(file.first_tag());
    let get = |k: ItemKey| tag.and_then(|t| t.get_string(&k)).map(String::from);
    let artists = split_artists(tag.map(|t| t.get_strings(&ItemKey::TrackArtist).map(String::from).collect::<Vec<_>>()).unwrap_or_default());
    let album_artist = split_artists(get(ItemKey::AlbumArtist)).into_iter().next().or_else(|| artists.first().cloned()).unwrap_or_default();
    let album = tag.and_then(|t| t.album().map(|a| a.to_string())).unwrap_or_default();
    let path_s = path.to_string_lossy().into_owned();
    let year = get(ItemKey::Year).or_else(|| get(ItemKey::RecordingDate)).map(|y| parse_year(&y)).unwrap_or(0);
    let mut s = SongData {
        title: tag.and_then(|t| t.title().map(|t| t.to_string())).unwrap_or_else(|| path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()),
        album: album.clone(),
        artist: album_artist.clone(),
        artists: artists.iter().map(|a| Ref { id: format!("ARTIST:{a}"), name: a.clone() }).collect(),
        duration: file.properties().duration().as_secs() as i32,
        track: tag.and_then(|t| t.track()).unwrap_or(0) as i32,
        disc_number: tag.and_then(|t| t.disk()).unwrap_or(0) as i32,
        year,
        cover_art: path_s.clone(),
        album_gain: parse_gain(get(ItemKey::ReplayGainAlbumGain).as_deref()),
        track_gain: parse_gain(get(ItemKey::ReplayGainTrackGain).as_deref()),
        container: path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default(),
        path: path_s,
        ..Default::default()
    };
    if with_ids {
        s.artist_id = if album_artist.is_empty() { String::new() } else { format!("ARTIST:{album_artist}") };
        s.album_id = if album.is_empty() { String::new() } else { format!("ALBUM:{album}") };
    }
    Some(s)
}

/// Read tags from an audio file (used for local libraries and opened files).
pub fn song_from_file(path: &std::path::Path) -> Option<SongData> {
    read_song(path, false)
}

fn folder_cover(path: &Path) -> Option<PathBuf> {
    let dir = if path.is_dir() { path } else { path.parent()? };
    ["cover.jpg", "folder.jpg", "cover.png", "folder.png", "Cover.jpg", "Folder.jpg"].iter().map(|n| dir.join(n)).find(|p| p.is_file())
}

/// Embedded picture (front cover preferred), else cover/folder image beside the file.
pub fn cover_from_file(path: &Path) -> Option<Vec<u8>> {
    if let Ok(f) = lofty::read_from_path(path) {
        let pics: Vec<_> = f.tags().iter().flat_map(|t| t.pictures()).collect();
        let front = pics.iter().find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront).or(pics.first());
        if let Some(p) = front {
            return Some(p.data().to_vec());
        }
    }
    std::fs::read(folder_cover(path)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(split_artists(["A; B".into(), "B;C".into()]), vec!["A", "B", "C"]);
        assert_eq!(parse_year("2001-05-03"), 2001);
        assert_eq!(parse_year("12345"), 0);
        assert_eq!(parse_gain(Some("-6.20 dB")), -6.2);
    }

    #[test]
    fn indexes_by_name() {
        let mut lib = Library::default();
        let song = |id: &str, album: &str| SongData {
            id: id.into(),
            album: album.into(),
            album_id: if album.is_empty() { String::new() } else { format!("ALBUM:{album}") },
            artist: "X".into(),
            artist_id: "ARTIST:X".into(),
            artists: vec![Ref { id: "ARTIST:X".into(), name: "X".into() }, Ref { id: "ARTIST:Y".into(), name: "Y".into() }],
            duration: 10,
            ..Default::default()
        };
        lib.add(song("SONG:/a", "A"));
        lib.add(song("SONG:/b", "A"));
        lib.add(song("SONG:/c", ""));
        assert_eq!(lib.albums["ALBUM:A"].1.len(), 2);
        assert_eq!(lib.albums["ALBUM:A"].0.duration, 20);
        assert!(lib.albums.contains_key("ALBUM:NO_ALBUM:ARTIST:X"));
        assert_eq!(lib.artists["ARTIST:X"].0.album_count, 2);
        assert_eq!(lib.artists["ARTIST:Y"].0.album_count, 2);
        assert!(lib.album_artists.contains("ARTIST:X") && !lib.album_artists.contains("ARTIST:Y"));
    }
}
