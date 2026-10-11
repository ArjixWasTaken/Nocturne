//! MPRIS media controls via `mpris-server`.
use crate::{integration, player::Player, runtime::spawn_local};
use mpris_server::{Metadata, PlaybackStatus, Time, TrackId};
use gtk::prelude::*;
use std::rc::Rc;

pub fn start(player: Player, raise: impl Fn() + 'static) {
    spawn_local(async move {
        let mpris = match mpris_server::Player::builder("com.jeffser.Nocturne")
            .identity("Nocturne")
            .desktop_entry(crate::config::APP_ID)
            .can_play(true)
            .can_pause(true)
            .can_go_next(true)
            .can_go_previous(true)
            .can_seek(true)
            .can_control(true)
            .can_raise(true)
            .can_quit(true)
            .build()
            .await
        {
            Ok(p) => Rc::new(p),
            Err(e) => return tracing::warn!("mpris: {e}"),
        };

        let p = player.clone();
        mpris.connect_play_pause(move |_| p.toggle());
        let p = player.clone();
        mpris.connect_play(move |_| p.play());
        let p = player.clone();
        mpris.connect_pause(move |_| p.pause());
        let p = player.clone();
        mpris.connect_stop(move |_| p.stop());
        let p = player.clone();
        mpris.connect_next(move |_| p.next());
        let p = player.clone();
        mpris.connect_previous(move |_| p.previous());
        let p = player.clone();
        mpris.connect_seek(move |_, offset| p.seek(p.position() + offset.as_micros() as f64 / 1e6));
        let p = player.clone();
        mpris.connect_set_position(move |_, _, pos| p.seek(pos.as_micros() as f64 / 1e6));
        let p = player.clone();
        mpris.connect_set_volume(move |_, v| p.set_volume(v.cbrt()));
        mpris.connect_raise(move |_| raise());
        let p = player.clone();
        mpris.connect_quit(move |_| p.stop());

        let m = mpris.clone();
        let p = player.clone();
        player.connect_changed(move || {
            let (m, p) = (m.clone(), p.clone());
            spawn_local(async move {
                let status = if p.is_playing() { PlaybackStatus::Playing } else { PlaybackStatus::Paused };
                let _ = m.set_playback_status(status).await;
                let _ = m.set_metadata(metadata(&p)).await;
            });
        });

        // Keep the reported position in sync (MPRIS clients poll it).
        let m = mpris.clone();
        let p = player.clone();
        p.state().connect_notify_local(Some("positionSeconds"), move |s, _| {
            m.set_position(Time::from_micros((s.positionSeconds() * 1e6) as i64));
        });

        mpris.run().await;
    });
}

fn metadata(p: &Player) -> Metadata {
    let state = p.state();
    let id = state.songId();
    let Some(integ) = integration::current().filter(|_| !id.is_empty()) else { return Metadata::new() };
    let song = integ.song(&id);
    let mut m = Metadata::new();
    let safe: String = id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    if let Ok(t) = TrackId::try_from(format!("/com/jeffser/Nocturne/track/{safe}")) {
        m.set_trackid(Some(t));
    }
    m.set_title(Some(state.displaySongTitle()));
    m.set_artist(Some([state.displaySongArtist()]));
    m.set_album(Some(if song.radioStreamUrl().is_empty() { song.album() } else { song.title() }));
    if song.duration() > 0 {
        m.set_length(Some(Time::from_secs(song.duration().into())));
    }
    let art = integ.cover_art_url(&id);
    if !art.is_empty() {
        m.set_art_url(Some(art));
    }
    m
}
