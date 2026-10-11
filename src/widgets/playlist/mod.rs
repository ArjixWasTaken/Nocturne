pub mod button;
pub mod dialog;
pub mod page;
pub mod row;
pub mod selector_row;

pub use button::PlaylistButton;
pub use dialog::PlaylistDialog;
pub use page::PlaylistPage;
pub use row::PlaylistRow;
pub use selector_row::PlaylistSelectorRow;

use crate::{integration, models::Playlist, runtime::spawn_local};
use gtk::{gdk, glib, prelude::*};

/// Call `update(widget, model)` now and whenever any property of `model` changes (weakly held).
pub(crate) fn watch<W: IsA<glib::Object>>(widget: &W, model: &Playlist, update: impl Fn(&W, &Playlist) + 'static) {
    update(widget, model);
    let w = widget.downgrade();
    model.connect_notify_local(None, move |m, _| {
        if let Some(o) = w.upgrade() {
            update(&o, m);
        }
    });
}

/// Show the cover, or a placeholder icon once loading is over (`done`).
pub(crate) fn apply_cover(img: &gtk::Image, p: Option<gdk::Paintable>, size: i32, done: bool) {
    match p {
        Some(p) => {
            img.set_paintable(Some(&p));
            img.set_pixel_size(size);
        }
        None if done => {
            img.set_icon_name(Some("music-note-symbolic"));
            img.set_pixel_size(-1);
        }
        None => {}
    }
}

/// Verify the playlist (optionally forced) and load its cover; then call `done`.
pub(crate) fn load(id: &str, force: bool, done: impl FnOnce() + 'static) {
    let id = id.to_string();
    spawn_local(async move {
        let i = integration::get();
        i.verify_playlist(&id, force).await;
        i.load_cover(&id).await;
        done();
    });
}

pub(crate) fn songs_label(n: i32) -> String {
    if n == 1 { "1 Song".into() } else { format!("{n} Songs") }
}
