#![allow(non_snake_case)]
//! GObject data models. Type and property names match the Blueprint templates
//! (`$NocturneModelSong`, `.gdkPaintable`, …), so they must not be renamed.
use gtk::{gdk, gio, glib, prelude::*, subclass::prelude::*};

/// `{id, name}` reference to another item (artists of a song, similar artists, …).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Ref {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, glib::Boxed)]
#[boxed_type(name = "NocturneRefList")]
pub struct RefList(pub Vec<Ref>);

#[derive(Clone, Debug, Default, PartialEq, glib::Boxed)]
#[boxed_type(name = "NocturneIdList")]
pub struct IdList(pub Vec<String>);

macro_rules! model {
    ($modname:ident, $Name:ident, $gtype:literal, { $($(#[$attr:meta])* $field:ident : $ty:ty),* $(,)? }) => {
        mod $modname {
            use super::*;
            use std::cell::RefCell;

            #[derive(Default, glib::Properties)]
            #[properties(wrapper_type = super::$Name)]
            pub struct $Name {
                $( $(#[$attr])* pub $field: RefCell<$ty>, )*
            }

            #[glib::object_subclass]
            impl ObjectSubclass for $Name {
                const NAME: &'static str = $gtype;
                type Type = super::$Name;
            }

            #[glib::derived_properties]
            impl ObjectImpl for $Name {}
        }

        glib::wrapper! {
            pub struct $Name(ObjectSubclass<$modname::$Name>);
        }

        impl $Name {
            pub fn new(id: &str) -> Self {
                glib::Object::builder().property("id", id).build()
            }
        }
    };
}

model!(album_imp, Album, "NocturneModelAlbum", {
    #[property(get, set)] id: String,
    #[property(get, set, name = "coverArt")] cover_art: String,
    #[property(get, set, nullable, name = "gdkPaintable")] gdk_paintable: Option<gdk::Paintable>,
    #[property(get, set, nullable, name = "gdkPaintableBig")] gdk_paintable_big: Option<gdk::Paintable>,
    #[property(get, set)] name: String,
    #[property(get, set)] artist: String,
    #[property(get, set, name = "artistId")] artist_id: String,
    #[property(get, set, name = "songCount")] song_count: i32,
    #[property(get, set)] duration: i32,
    #[property(get, set)] artists: RefList,
    /// Song ids of the album.
    #[property(get, set)] song: IdList,
    #[property(get, set)] starred: bool,
    #[property(get, set, name = "userRating")] user_rating: i32,
    #[property(get, set)] year: i32,
    /// Whether the full metadata (songs) has been loaded.
    #[property(get, set)] verified: bool,
});

model!(artist_imp, Artist, "NocturneModelArtist", {
    #[property(get, set)] id: String,
    #[property(get, set, name = "coverArt")] cover_art: String,
    #[property(get, set, nullable, name = "gdkPaintable")] gdk_paintable: Option<gdk::Paintable>,
    #[property(get, set, nullable, name = "gdkPaintableBig")] gdk_paintable_big: Option<gdk::Paintable>,
    #[property(get, set)] name: String,
    #[property(get, set, name = "albumCount")] album_count: i32,
    /// Album ids of the artist.
    #[property(get, set)] album: IdList,
    #[property(get, set)] starred: bool,
    #[property(get, set)] biography: String,
    #[property(get, set, name = "similarArtist")] similar_artist: RefList,
    #[property(get, set, name = "userRating")] user_rating: i32,
    #[property(get, set)] verified: bool,
});

model!(playlist_imp, Playlist, "NocturneModelPlaylist", {
    #[property(get, set)] id: String,
    #[property(get, set, name = "coverArt")] cover_art: String,
    #[property(get, set, nullable, name = "gdkPaintable")] gdk_paintable: Option<gdk::Paintable>,
    #[property(get, set, nullable, name = "gdkPaintableBig")] gdk_paintable_big: Option<gdk::Paintable>,
    #[property(get, set)] name: String,
    #[property(get, set, name = "songCount")] song_count: i32,
    #[property(get, set)] duration: i32,
    #[property(get, set)] readonly: bool,
    /// Song ids of the playlist.
    #[property(get, set)] entry: IdList,
    #[property(get, set)] verified: bool,
});

model!(song_imp, Song, "NocturneModelSong", {
    #[property(get, set)] id: String,
    #[property(get, set, name = "coverArt")] cover_art: String,
    #[property(get, set, nullable, name = "gdkPaintable")] gdk_paintable: Option<gdk::Paintable>,
    #[property(get, set, nullable, name = "gdkPaintableBig")] gdk_paintable_big: Option<gdk::Paintable>,
    #[property(get, set)] title: String,
    #[property(get, set)] album: String,
    #[property(get, set)] artist: String,
    #[property(get, set)] duration: i32,
    #[property(get, set, name = "albumId")] album_id: String,
    #[property(get, set, name = "discNumber")] disc_number: i32,
    #[property(get, set, name = "albumGain")] album_gain: f64,
    #[property(get, set, name = "trackGain")] track_gain: f64,
    #[property(get, set, name = "artistId")] artist_id: String,
    #[property(get, set)] artists: RefList,
    #[property(get, set)] starred: bool,
    #[property(get, set)] track: i32,
    #[property(get, set, name = "isExternalFile")] is_external_file: bool,
    #[property(get, set, name = "userRating")] user_rating: i32,
    #[property(get, set)] deleted: bool,
    #[property(get, set, name = "radioStreamUrl")] radio_stream_url: String,
    #[property(get, set)] container: String,
    #[property(get, set)] codec: String,
    #[property(get, set)] path: String,
    #[property(get, set)] verified: bool,
});

model!(download_imp, SongDownload, "NocturneSongDownload", {
    #[property(get, set)] id: String,
    #[property(get, set, minimum = 0.0, maximum = 1.0)] progress: f64,
});

mod state_imp {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(glib::Properties)]
    #[properties(wrapper_type = super::CurrentState)]
    pub struct CurrentState {
        #[property(get, set, name = "songId")]
        pub song_id: RefCell<String>,
        #[property(get, set, name = "videoId")]
        pub video_id: RefCell<String>,
        #[property(get, set, name = "positionSeconds")]
        pub position_seconds: Cell<f64>,
        /// `play` or `pause` – names of the children of the play/pause state stacks.
        #[property(get, set, name = "buttonState")]
        pub button_state: RefCell<String>,
        #[property(get, set)]
        pub seeking: Cell<bool>,
        #[property(get, set, name = "queueOrigin")]
        pub queue_origin: RefCell<String>,
        /// `Gtk.StringObject` song ids.
        #[property(get, name = "queueModel")]
        pub queue_model: gio::ListStore,
        #[property(get, name = "generatedQueue")]
        pub generated_queue: gio::ListStore,
        #[property(get, set, name = "generatingQueue")]
        pub generating_queue: Cell<bool>,
        #[property(get, name = "downloadQueueModel")]
        pub download_queue_model: gio::ListStore,
        #[property(get, set, name = "displaySongTitle")]
        pub display_song_title: RefCell<String>,
        #[property(get, set, name = "displaySongArtist")]
        pub display_song_artist: RefCell<String>,
        /// Latest spectrum magnitudes (0..1) for the visualizer.
        pub magnitudes: RefCell<Vec<f32>>,
        pub accent_color: Cell<Option<(u8, u8, u8)>>,
    }

    impl Default for CurrentState {
        fn default() -> Self {
            Self {
                song_id: Default::default(),
                video_id: Default::default(),
                position_seconds: Default::default(),
                button_state: RefCell::new("play".into()),
                seeking: Default::default(),
                queue_origin: Default::default(),
                queue_model: gio::ListStore::new::<gtk::StringObject>(),
                generated_queue: gio::ListStore::new::<gtk::StringObject>(),
                generating_queue: Default::default(),
                download_queue_model: gio::ListStore::new::<super::SongDownload>(),
                display_song_title: Default::default(),
                display_song_artist: Default::default(),
                magnitudes: Default::default(),
                accent_color: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CurrentState {
        const NAME: &'static str = "NocturneModelCurrentState";
        type Type = super::CurrentState;
    }

    #[glib::derived_properties]
    impl ObjectImpl for CurrentState {}
}

glib::wrapper! {
    /// Global playback state; the player reacts to `songId` changes.
    pub struct CurrentState(ObjectSubclass<state_imp::CurrentState>);
}

impl Default for CurrentState {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl CurrentState {
    pub fn queue_ids(&self) -> Vec<String> {
        string_list(&self.queueModel())
    }
    pub fn set_queue_ids(&self, ids: &[String]) {
        set_string_list(&self.queueModel(), ids);
    }
    pub fn magnitudes(&self) -> Vec<f32> {
        self.imp().magnitudes.borrow().clone()
    }
    pub fn set_magnitudes(&self, m: Vec<f32>) {
        *self.imp().magnitudes.borrow_mut() = m;
    }
    pub fn accent_color(&self) -> Option<(u8, u8, u8)> {
        self.imp().accent_color.get()
    }
    pub fn set_accent_color(&self, c: Option<(u8, u8, u8)>) {
        self.imp().accent_color.set(c);
    }
}

/// Read the ids out of a `ListStore` of `Gtk.StringObject`.
pub fn string_list(store: &gio::ListStore) -> Vec<String> {
    store.iter::<gtk::StringObject>().flatten().map(|s| s.string().into()).collect()
}

pub fn set_string_list(store: &gio::ListStore, ids: &[String]) {
    let items: Vec<gtk::StringObject> = ids.iter().map(|id| gtk::StringObject::new(id)).collect();
    store.splice(0, store.n_items(), &items);
}
