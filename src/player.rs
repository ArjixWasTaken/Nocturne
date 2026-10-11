//! GStreamer playback driven by [`CurrentState`]: setting `songId` plays it.
use crate::{config::settings, integration, models::CurrentState, runtime::spawn_local};
use gst::prelude::*;
use gtk::{gio, glib, prelude::*};
use std::{cell::RefCell, rc::Rc};

pub const EQ_BANDS: u32 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Consecutive,
    RepeatAll,
    RepeatOne,
}

impl Mode {
    pub fn current() -> Self {
        match settings().string("playback-mode").as_str() {
            "repeat-all" => Self::RepeatAll,
            "repeat-one" => Self::RepeatOne,
            _ => Self::Consecutive,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Next,
    Previous,
    End,
}

struct Inner {
    playbin: gst::Element,
    spectrum: gst::Element,
    rg_volume: gst::Element,
    state: CurrentState,
    last_song: RefCell<String>,
    preloaded: RefCell<String>,
    pause_next_change: std::cell::Cell<bool>,
    _bus_watch: RefCell<Option<gst::bus::BusWatchGuard>>,
    /// Called on metadata/playback changes (MPRIS).
    on_changed: RefCell<Vec<Box<dyn Fn()>>>,
    /// Ready-to-play URI for the next queue entry (needed synchronously in `about-to-finish`).
    next_uri: RefCell<(String, String)>,
}

/// Cheap clonable handle.
#[derive(Clone)]
pub struct Player(Rc<Inner>);

fn make(name: &str) -> gst::Element {
    gst::ElementFactory::make(name).build().unwrap_or_else(|_| panic!("missing GStreamer element {name}"))
}

impl Player {
    pub fn new(state: CurrentState) -> Self {
        let _ = gst::init();
        let playbin = make("playbin3");
        if let Ok(sink) = gst::ElementFactory::make("gtk4paintablesink").build() {
            playbin.set_property("video-sink", &sink);
        }

        let eq = make("equalizer-nbands");
        eq.set_property("num-bands", EQ_BANDS);
        let rg_volume = make("rgvolume");
        rg_volume.set_property("album-mode", true);
        let rg_limiter = make("rglimiter");
        let spectrum = make("spectrum");
        spectrum.set_property("threshold", -60i32);
        spectrum.set_property("post-messages", true);
        spectrum.set_property("message-magnitude", true);
        spectrum.set_property("multi-channel", true);
        spectrum.set_property("interval", 50_000_000u64);

        let bin = gst::Bin::with_name("audio-filter-bin");
        bin.add_many([&eq, &rg_volume, &rg_limiter, &spectrum]).unwrap();
        gst::Element::link_many([&eq, &rg_volume, &rg_limiter, &spectrum]).unwrap();
        bin.add_pad(&gst::GhostPad::with_target(&eq.static_pad("sink").unwrap()).unwrap()).unwrap();
        bin.add_pad(&gst::GhostPad::with_target(&spectrum.static_pad("src").unwrap()).unwrap()).unwrap();
        playbin.set_property("audio-filter", &bin);
        playbin.set_property("buffer-duration", 5 * gst::ClockTime::SECOND.nseconds() as i64);
        playbin.set_property("buffer-size", 10 * 1024 * 1024i32);

        let s = settings();
        let child_proxy = eq.dynamic_cast_ref::<gst::ChildProxy>().unwrap();
        for i in 0..EQ_BANDS {
            if let Some(band) = child_proxy.child_by_index(i) {
                s.bind(&format!("eq-band-{i}"), &band, "gain").build();
            }
        }
        s.bind("visualizer-bar-n", &spectrum, "bands").mapping(|v, _| v.get::<i32>().map(|n| (n.max(2) as u32).to_value())).set_mapping(|v, _| v.get::<u32>().ok().map(|n| (n as i32).to_variant())).build();

        let player = Self(Rc::new(Inner {
            playbin,
            spectrum,
            rg_volume,
            state,
            last_song: Default::default(),
            preloaded: Default::default(),
            pause_next_change: Default::default(),
            _bus_watch: Default::default(),
            on_changed: Default::default(),
            next_uri: Default::default(),
        }));
        player.setup();
        player
    }

    fn weak(&self) -> std::rc::Weak<Inner> {
        Rc::downgrade(&self.0)
    }

    fn setup(&self) {
        let pb = &self.0.playbin;

        // TLS "trust server"
        pb.connect("source-setup", false, |args| {
            let source = args[1].get::<gst::Element>().ok()?;
            if source.has_property("ssl-strict") {
                // Runs on a streaming thread: read from GSettings directly.
                let trust = gio::Settings::new(crate::config::APP_ID).boolean("integration-trust-server");
                source.set_property("ssl-strict", !trust);
            }
            None
        });

        // Gapless: hand playbin the next URI (pre-resolved on the main thread).
        let next = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        {
            let next = next.clone();
            pb.connect("about-to-finish", false, move |args| {
                let uri = next.lock().unwrap().clone();
                if !uri.is_empty() {
                    args[0].get::<gst::Element>().unwrap().set_property("uri", &uri);
                }
                None
            });
        }

        // Volume: settings value is cubic.
        let s = settings();
        pb.set_property("volume", s.double("volume").powi(3));
        let weak = self.weak();
        s.connect_changed(Some("volume"), move |s, _| {
            if let Some(i) = weak.upgrade() {
                let v = s.double("volume").powi(3);
                if (i.playbin.property::<f64>("volume") - v).abs() > 1e-4 {
                    i.playbin.set_property("volume", v);
                }
            }
        });

        // Bus
        let weak = self.weak();
        let watch = pb
            .bus()
            .unwrap()
            .add_watch_local(move |_, msg| {
                if let Some(i) = weak.upgrade() {
                    Player(i).on_message(msg);
                }
                glib::ControlFlow::Continue
            })
            .unwrap();
        self.0._bus_watch.replace(Some(watch));

        // Position polling
        let weak = self.weak();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            let Some(i) = weak.upgrade() else { return glib::ControlFlow::Break };
            if !i.state.seeking()
                && let Some(pos) = i.playbin.query_position::<gst::ClockTime>()
            {
                i.state.set_positionSeconds(pos.nseconds() as f64 / 1e9);
            }
            glib::ControlFlow::Continue
        });

        // React to song changes
        let weak = self.weak();
        self.0.state.connect_notify_local(Some("songId"), move |_, _| {
            if let Some(i) = weak.upgrade() {
                let p = Player(i);
                spawn_local(async move { p.song_changed().await });
            }
        });

        // Keep the pre-resolved next URI fresh.
        let weak = self.weak();
        let update_next = move || {
            if let Some(i) = weak.upgrade() {
                let p = Player(i);
                let next = next.clone();
                spawn_local(async move {
                    let id = p.next_queue_id().unwrap_or_default();
                    let uri = if id.is_empty() { String::new() } else { integration::get().stream_url(&id).await };
                    *next.lock().unwrap() = uri.clone();
                    p.0.next_uri.replace((id, uri));
                });
            }
        };
        let u = update_next.clone();
        self.0.state.connect_notify_local(Some("songId"), move |_, _| u());
        let u = update_next.clone();
        self.0.state.queueModel().connect_items_changed(move |_, _, _, _| u());
        settings().connect_changed(Some("playback-mode"), move |_, _| update_next());
    }

    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.0.on_changed.borrow_mut().push(Box::new(f));
    }

    fn emit_changed(&self) {
        for f in self.0.on_changed.borrow().iter() {
            f();
        }
    }

    pub fn state(&self) -> CurrentState {
        self.0.state.clone()
    }

    pub fn playbin(&self) -> gst::Element {
        self.0.playbin.clone()
    }

    pub fn is_playing(&self) -> bool {
        self.0.playbin.current_state() == gst::State::Playing
    }

    pub fn play(&self) {
        if self.0.state.songId().is_empty() {
            self.change(Change::Next);
        } else {
            let _ = self.0.playbin.set_state(gst::State::Playing);
        }
    }

    pub fn pause(&self) {
        let _ = self.0.playbin.set_state(gst::State::Paused);
    }

    pub fn toggle(&self) {
        if self.is_playing() { self.pause() } else { self.play() }
    }

    pub fn stop(&self) {
        self.0.state.set_songId("");
    }

    pub fn seek(&self, seconds: f64) {
        let _ = self.0.playbin.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
            gst::ClockTime::from_nseconds((seconds.max(0.0) * 1e9) as u64),
        );
        self.0.state.set_positionSeconds(seconds);
    }

    pub fn position(&self) -> f64 {
        self.0.state.positionSeconds()
    }

    /// Linear 0..1 volume as stored in settings.
    pub fn set_volume(&self, v: f64) {
        let _ = settings().set_double("volume", v.clamp(0.0, 1.0));
    }

    pub fn volume(&self) -> f64 {
        settings().double("volume")
    }

    /// Start the next song paused (used when restoring a session).
    pub fn pause_next_change(&self) {
        self.0.pause_next_change.set(true);
    }

    pub fn next(&self) {
        self.change(Change::Next);
    }

    pub fn previous(&self) {
        self.change(Change::Previous);
    }

    fn next_queue_id(&self) -> Option<String> {
        let ids = self.0.state.queue_ids();
        let cur = self.0.state.songId();
        let idx = ids.iter().position(|i| *i == cur);
        match Mode::current() {
            Mode::RepeatOne => Some(cur).filter(|c| !c.is_empty()),
            Mode::RepeatAll => idx.and_then(|i| ids.get(i + 1)).or(ids.first()).cloned(),
            Mode::Consecutive => idx.and_then(|i| ids.get(i + 1)).cloned(),
        }
    }

    pub fn change(&self, action: Change) {
        let state = &self.0.state;
        let current = state.songId();
        if !current.is_empty() {
            let song = integration::get().song(&current);
            let pos = state.positionSeconds();
            if action == Change::End || (song.duration() > 0 && pos >= song.duration() as f64 / 2.0) {
                spawn_local(async move { integration::get().scrobble(&current, true).await });
            }
        }
        let current = state.songId();
        let mut mode = Mode::current();
        if action != Change::End && mode == Mode::RepeatOne {
            mode = Mode::Consecutive;
        }
        if action == Change::Previous && state.positionSeconds() > 5.0 {
            self.seek(0.0);
            return;
        }
        state.set_magnitudes(vec![]);
        let ids = state.queue_ids();
        if ids.is_empty() {
            state.set_songId("");
            return;
        }
        if current.is_empty() {
            state.set_songId(ids[0].as_str());
            return;
        }
        let step: isize = if action == Change::Previous { -1 } else { 1 };
        let next = ids.iter().position(|i| *i == current).map_or(0, |i| i as isize + step);
        match mode {
            Mode::RepeatOne => self.seek(0.0),
            Mode::Consecutive => {
                if next < 0 {
                    state.set_songId(ids[0].as_str());
                } else if (next as usize) < ids.len() {
                    state.set_songId(ids[next as usize].as_str());
                } else if settings().boolean("auto-play") {
                    if let Some(app) = gio::Application::default() {
                        app.activate_action("generate_auto_play_queue", Some(&true.to_variant()));
                    }
                }
            }
            Mode::RepeatAll => {
                if next >= 0 && (next as usize) < ids.len() {
                    state.set_songId(ids[next as usize].as_str());
                } else {
                    state.set_songId(ids[0].as_str());
                    self.seek(0.0);
                }
            }
        }
    }

    async fn song_changed(&self) {
        let i = &self.0;
        let id = i.state.songId();
        let preloaded = i.preloaded.take();
        if id.is_empty() {
            let _ = i.playbin.set_state(gst::State::Null);
            i.last_song.replace(String::new());
            self.emit_changed();
            return;
        }
        if *i.last_song.borrow() == id {
            return;
        }
        i.last_song.replace(id.clone());
        let integration = integration::get();
        {
            let id = id.clone();
            let integ = integration.clone();
            spawn_local(async move { integ.scrobble(&id, false).await });
        }
        i.state.set_positionSeconds(0.0);
        let song = integration.verify_song(&id, false).await;
        i.state.set_displaySongTitle(song.title());
        i.state.set_displaySongArtist(song.artists().0.first().map(|a| a.name.clone()).unwrap_or(song.artist()));
        self.apply_gain(&song);
        {
            let integ = integration.clone();
            let id = id.clone();
            spawn_local(async move { integ.load_cover(&id).await });
        }

        if preloaded != id {
            let uri = integration.stream_url(&id).await;
            if i.state.songId() != id {
                return; // changed again while resolving
            }
            if uri.is_empty() {
                let _ = i.playbin.set_state(gst::State::Null);
                return;
            }
            let _ = i.playbin.set_state(gst::State::Ready);
            i.playbin.set_property("uri", &uri);
        }
        let target = if i.pause_next_change.replace(false) { gst::State::Paused } else { gst::State::Playing };
        let _ = i.playbin.set_state(target);
        self.emit_changed();
    }

    fn apply_gain(&self, song: &crate::models::Song) {
        if !settings().boolean("use-gain") {
            self.0.rg_volume.set_property("fallback-gain", 0.0f64);
            return;
        }
        let (gain, album_mode) = if song.trackGain() != 0.0 { (song.trackGain(), false) } else { (song.albumGain(), true) };
        self.0.rg_volume.set_property("fallback-gain", gain);
        self.0.rg_volume.set_property("album-mode", album_mode);
    }

    fn on_message(&self, msg: &gst::Message) {
        use gst::MessageView as M;
        let i = &self.0;
        let from_playbin = msg.src().is_some_and(|s| s == i.playbin.upcast_ref::<gst::Object>());
        match msg.view() {
            M::Eos(_) if from_playbin => self.change(Change::End),
            M::Error(e) => tracing::error!("gstreamer: {} ({:?})", e.error(), e.debug()),
            M::StateChanged(s) if from_playbin => {
                if s.pending() == gst::State::VoidPending && s.current() != gst::State::Ready && !i.state.seeking() {
                    let playing = s.current() == gst::State::Playing;
                    i.state.set_buttonState(if playing { "pause" } else { "play" });
                    self.emit_changed();
                    let id = i.state.songId();
                    if !id.is_empty() && let Some(integ) = integration::current() {
                        let pos = i.state.positionSeconds();
                        let report = if playing { crate::backend::PlaybackReport::Play } else { crate::backend::PlaybackReport::Pause };
                        spawn_local(async move {
                            let _ = integ.call(move |b| async move { b.playback_report(&id, report, pos).await }).await;
                        });
                    }
                }
            }
            M::StreamStart(_) => {
                // A gapless transition happened: the preloaded track is now playing.
                let (next_id, _) = i.next_uri.borrow().clone();
                let cur = i.state.songId();
                if !next_id.is_empty() && i.last_song.borrow().as_str() == cur && self.playbin_uri_matches_next() {
                    i.preloaded.replace(next_id.clone());
                    i.state.set_songId(next_id.as_str());
                }
            }
            M::StreamCollection(c) => {
                let has_video = c.stream_collection().iter().any(|s| s.stream_type().contains(gst::StreamType::VIDEO));
                i.state.set_videoId(if has_video { i.state.songId() } else { String::new() });
            }
            M::Tag(t) => self.on_tag(&t.tags()),
            M::Element(e) if msg.src().is_some_and(|s| s == i.spectrum.upcast_ref::<gst::Object>()) => {
                if let Some(st) = e.structure()
                    && st.name() == "spectrum"
                    && settings().boolean("show-visualizer")
                {
                    i.state.set_magnitudes(spectrum_magnitudes(st, settings().double("volume")));
                }
            }
            _ => {}
        }
    }

    fn playbin_uri_matches_next(&self) -> bool {
        let uri: Option<String> = self.0.playbin.property("current-uri");
        let (_, next_uri) = self.0.next_uri.borrow().clone();
        !next_uri.is_empty() && uri.as_deref() == Some(next_uri.as_str())
    }

    fn on_tag(&self, tags: &gst::TagList) {
        let state = &self.0.state;
        let id = state.songId();
        let Some(integ) = integration::current() else { return };
        if id.is_empty() || integ.song(&id).radioStreamUrl().is_empty() {
            return;
        }
        let clean = |s: Option<String>| s.filter(|s| s != "null").unwrap_or_default();
        let mut title = clean(tags.get::<gst::tags::Title>().map(|v| v.get().to_string()));
        let mut artist = clean(tags.get::<gst::tags::Artist>().map(|v| v.get().to_string()));
        if !title.is_empty() && artist.is_empty() && let Some((a, t)) = title.clone().split_once(" - ") {
            artist = a.to_string();
            title = t.to_string();
        }
        let mut changed = false;
        if !title.trim().is_empty() && state.displaySongTitle() != title.trim() {
            state.set_displaySongTitle(title.trim());
            changed = true;
        }
        if !artist.trim().is_empty() && state.displaySongArtist() != artist.trim() {
            state.set_displaySongArtist(artist.trim());
            changed = true;
        }
        if changed {
            self.emit_changed();
        }
    }
}

/// Convert a `spectrum` message into mirrored 0..1 magnitudes.
fn spectrum_magnitudes(st: &gst::StructureRef, volume: f64) -> Vec<f32> {
    let Ok(mag) = st.value("magnitude") else { return vec![] };
    let floats = |v: &glib::SendValue| -> Vec<f32> {
        if let Ok(a) = v.get::<gst::Array>() {
            a.iter().filter_map(|v| v.get::<f32>().ok()).collect()
        } else if let Ok(l) = v.get::<gst::List>() {
            l.iter().filter_map(|v| v.get::<f32>().ok()).collect()
        } else {
            v.get::<f32>().map(|f| vec![f]).unwrap_or_default()
        }
    };
    let channels: Vec<Vec<f32>> = match (mag.get::<gst::List>(), mag.get::<gst::Array>()) {
        (Ok(l), _) if l.iter().any(|v| v.get::<f32>().is_err()) => l.iter().map(floats).collect(),
        (Ok(l), _) => vec![l.iter().filter_map(|v| v.get::<f32>().ok()).collect()],
        (_, Ok(a)) if a.iter().any(|v| v.get::<f32>().is_err()) => a.iter().map(floats).collect(),
        (_, Ok(a)) => vec![a.iter().filter_map(|v| v.get::<f32>().ok()).collect()],
        _ => return vec![],
    };
    let norm = |m: &f32| ((60.0 - m.abs()) / 60.0 * volume as f32).max(0.0);
    let half = |c: &Vec<f32>| c[..c.len() / 2].to_vec();
    match channels.as_slice() {
        [] => vec![],
        [mono] => half(mono).iter().chain(half(mono).iter().rev()).map(norm).collect(),
        [l, r, ..] => half(l).iter().chain(half(r).iter().rev()).map(norm).collect(),
    }
}
