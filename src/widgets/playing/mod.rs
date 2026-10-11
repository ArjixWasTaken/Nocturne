pub mod control_page;
pub mod cover_art;
pub mod equalizer_page;
pub mod footer;
pub mod lyrics_page;
pub mod playback_mode_button;
pub mod popout_window;
pub mod queue_page;
pub mod volume_button;

pub use control_page::PlayingControlPage;
pub use cover_art::PlayingCoverArt;
pub use equalizer_page::EqualizerPage;
pub use footer::PlayingFooter;
pub use lyrics_page::PlayingLyricsPage;
pub use playback_mode_button::PlaybackModeButton;
pub use popout_window::PopoutWindow;
pub use queue_page::PlayingQueuePage;
pub use volume_button::VolumeButton;
pub mod palette;
pub mod spectrum;

pub use spectrum::Spectrum;

use crate::{application::Application, integration, models::{CurrentState, Song}};
use gtk::{glib, prelude::*};
use std::cell::RefCell;

/// Run `f` with the current state now (if logged in) and after every login.
pub(crate) fn on_login<W: IsA<glib::Object>>(widget: &W, f: impl Fn(&W, &CurrentState) + 'static) {
    let weak = widget.downgrade();
    let run = move || {
        if let (Some(w), Some(i)) = (weak.upgrade(), integration::current()) {
            f(&w, &i.current_state());
        }
    };
    run();
    if let Some(app) = gtk::gio::Application::default().and_downcast::<Application>() {
        app.connect_notify_local(Some("integration"), move |_, _| run());
    }
}

/// Run `f` with the state value of `prop` now and whenever it changes.
pub(crate) fn watch<W: IsA<glib::Object>>(state: &CurrentState, widget: &W, prop: &str, f: impl Fn(&W, &CurrentState) + 'static) {
    let weak = widget.downgrade();
    if let Some(w) = weak.upgrade() {
        f(&w, state);
    }
    state.connect_notify_local(Some(prop), move |s, _| {
        if let Some(w) = weak.upgrade() {
            f(&w, s);
        }
    });
}

/// Call `f` with the current song whenever it changes or any of its properties is notified.
pub(crate) fn follow_song<W: IsA<glib::Object>>(state: &CurrentState, widget: &W, f: impl Fn(&W, &Song) + 'static) {
    let f = std::rc::Rc::new(f);
    let handler: RefCell<Option<(Song, glib::SignalHandlerId)>> = RefCell::new(None);
    watch(state, widget, "songId", move |w, s| {
        if let Some((old, id)) = handler.take() {
            old.disconnect(id);
        }
        let Some(integ) = integration::current() else { return };
        let song = integ.song(&s.songId());
        let weak = w.downgrade();
        let f2 = f.clone();
        let id = song.connect_notify_local(None, move |song, _| {
            if let Some(w) = weak.upgrade() {
                f2(&w, song);
            }
        });
        f(w, &song);
        handler.replace(Some((song, id)));
    });
}

/// Seek after the user stops dragging a scale for 500ms (sets `seeking` meanwhile).
pub(crate) fn delayed_seek(scale: &gtk::Scale, value: f64) {
    let Some(state) = integration::current().map(|i| i.current_state()) else { return };
    state.set_seeking(true);
    let scale = scale.downgrade();
    glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
        let Some(scale) = scale.upgrade() else { return };
        if scale.value() != value {
            return;
        }
        state.set_seeking(false);
        if let Some(p) = Application::get().player() {
            p.seek(value);
        }
    });
}
