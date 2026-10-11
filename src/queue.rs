//! Queue manipulation shared by actions and widgets.
use crate::{config::settings, integration, models::Playlist, runtime::spawn_local};
use gtk::prelude::*;

/// Remember where a playlist was left off before the queue is replaced.
pub fn save_playlist_resume() {
    let Some(integ) = integration::current() else { return };
    let state = integ.current_state();
    let origin = state.queueOrigin();
    if integ.lookup(&origin).is_some_and(|m| m.is::<Playlist>()) {
        let (song, pos) = (state.songId(), state.positionSeconds());
        spawn_local(async move { integ.save_playlist_resume(&origin, &song, pos).await });
    }
}

/// Replace the queue and start playing `current` (or the first song).
pub fn replace(songs: &[String], current: Option<&str>, origin: &str) {
    let integ = integration::get();
    save_playlist_resume();
    let state = integ.current_state();
    state.set_queueOrigin(origin);
    state.set_queue_ids(songs);
    let current = current.map(str::to_string).or_else(|| songs.first().cloned()).unwrap_or_default();
    // Force a restart even if the same song was already current.
    if state.songId() == current {
        state.set_songId("");
    }
    state.set_songId(current);
    if settings().boolean("auto-play") {
        generate_auto_play(false);
    }
}

/// Insert after the current song (or replace if nothing is playing).
pub fn play_next(songs: &[String]) {
    insert(songs, true);
}

/// Append to the end of the queue (or replace if nothing is playing).
pub fn play_later(songs: &[String]) {
    insert(songs, false);
}

fn insert(songs: &[String], next: bool) {
    let state = integration::get().current_state();
    let current = state.songId();
    let mut ids = state.queue_ids();
    if ids.is_empty() || current.is_empty() {
        return replace(songs, None, "");
    }
    save_playlist_resume();
    state.set_queueOrigin("");
    ids.retain(|id| *id == current || !songs.contains(id));
    let at = if next { ids.iter().position(|i| *i == current).map_or(0, |i| i + 1) } else { ids.len() };
    let new: Vec<String> = songs.iter().filter(|s| **s != current).cloned().collect();
    ids.splice(at..at, new);
    state.set_queue_ids(&ids);
}

/// Fill `generatedQueue` with songs similar to the current queue; with
/// `replace_on_finish`, play them right away.
pub fn generate_auto_play(replace_on_finish: bool) {
    let integ = integration::get();
    let state = integ.current_state();
    if state.generatingQueue() {
        return;
    }
    state.set_generatingQueue(true);
    spawn_local(async move {
        let queue = state.queue_ids();
        let seed = queue.last().cloned().unwrap_or_else(|| state.songId());
        let mut ids = if seed.is_empty() { integ.random_songs(20).await } else { integ.similar_songs(&seed, 20).await };
        ids.retain(|id| !queue.contains(id));
        crate::models::set_string_list(&state.generatedQueue(), &ids);
        state.set_generatingQueue(false);
        if replace_on_finish && !ids.is_empty() {
            replace(&ids, None, "");
        }
    });
}

/// Fisher–Yates shuffle helper.
pub fn shuffled(mut ids: Vec<String>) -> Vec<String> {
    use rand::seq::SliceRandom;
    ids.shuffle(&mut rand::rng());
    ids
}
