//! Online lyrics from LRCLIB.
use crate::backend::{Lyrics, LyricsKind};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrcLib {
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

pub async fn lrclib(title: &str, artist: &str, album: &str, duration: i32) -> Option<Lyrics> {
    let client = crate::backend::http_client(false).ok()?;
    let mut q = vec![("track_name", title.to_string()), ("artist_name", artist.to_string())];
    if !album.is_empty() {
        q.push(("album_name", album.to_string()));
    }
    if duration > 0 {
        q.push(("duration", duration.to_string()));
    }
    let r: LrcLib = client.get("https://lrclib.net/api/get").query(&q).send().await.ok()?.error_for_status().ok()?.json().await.ok()?;
    match (r.synced_lyrics, r.plain_lyrics) {
        (Some(s), _) if !s.is_empty() => Some(Lyrics { kind: LyricsKind::Synced, content: s }),
        (_, Some(p)) if !p.is_empty() => Some(Lyrics { kind: LyricsKind::Plain, content: p }),
        _ => None,
    }
}

/// Parse LRC into `(seconds, line)` sorted by time.
pub fn parse_lrc(content: &str) -> Vec<(f64, String)> {
    match lrc::Lyrics::from_str(content) {
        Ok(l) => l
            .get_timed_lines()
            .iter()
            .map(|(t, line)| (t.get_timestamp() as f64 / 1000.0, line.to_string()))
            .collect(),
        Err(_) => vec![],
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses() {
        let l = super::parse_lrc("[00:01.50]hello\n[00:03.00]world");
        assert_eq!(l, vec![(1.5, "hello".into()), (3.0, "world".into())]);
    }
}
