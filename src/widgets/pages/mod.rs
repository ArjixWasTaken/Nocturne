pub mod albums;
pub mod albums_all;
pub mod artists;
pub mod home;
pub mod login;
pub mod playback;
pub mod playlists;
pub mod radios;
pub mod setup;
pub mod songs_all;
pub mod starred;
pub mod welcome;

pub use albums::AlbumsPage;
pub use albums_all::AlbumsAllPage;
pub use artists::ArtistsPage;
pub use home::HomePage;
pub use login::LoginDialog;
pub use playback::PlaybackPage;
pub use playlists::PlaylistsPage;
pub use radios::RadiosPage;
pub use setup::SetupPage;
pub use songs_all::SongsAllPage;
pub use starred::StarredPage;
pub use welcome::WelcomePage;

use crate::{
    backend::SearchCounts,
    config::settings,
    integration,
    runtime::spawn_local,
    widgets::{album, artist, playlist, song},
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

/// Common behaviour of the root pages (`load` on first show, `reload`, `reset` on logout).
pub trait LibraryPage {
    fn load(&self);
    fn reload(&self);
    fn reset(&self);
    /// Reload the next time the page is shown.
    fn set_lazy_reload(&self);
}

macro_rules! dispatch {
    ($page:expr, $method:ident) => {{
        let p = $page;
        if let Some(p) = p.downcast_ref::<HomePage>() { p.$method() }
        else if let Some(p) = p.downcast_ref::<AlbumsPage>() { p.$method() }
        else if let Some(p) = p.downcast_ref::<AlbumsAllPage>() { p.$method() }
        else if let Some(p) = p.downcast_ref::<ArtistsPage>() { p.$method() }
        else if let Some(p) = p.downcast_ref::<SongsAllPage>() { p.$method() }
        else if let Some(p) = p.downcast_ref::<PlaylistsPage>() { p.$method() }
        else if let Some(p) = p.downcast_ref::<StarredPage>() { p.$method() }
        else if let Some(p) = p.downcast_ref::<RadiosPage>() { p.$method() }
    }};
}

pub fn load(page: &adw::NavigationPage) {
    dispatch!(page, load)
}
pub fn reload(page: &adw::NavigationPage) {
    dispatch!(page, reload)
}
pub fn reset(page: &adw::NavigationPage) {
    dispatch!(page, reset)
}
pub fn set_lazy_reload(page: &adw::NavigationPage) {
    dispatch!(page, set_lazy_reload)
}

// ---------- widget helpers ----------

pub(crate) use crate::widgets::containers::wrapbox::{children, wrap_box_append as wrap_append};

/// Remove every child of an `Adw.WrapBox`.
pub(crate) fn wrap_clear(wrap: &gtk::Widget) {
    for c in children(wrap) {
        c.unparent();
    }
}

pub(crate) fn any_visible(containers: &[&gtk::Widget]) -> bool {
    containers.iter().any(|c| children(*c).iter().any(|w| w.is_visible()))
}

/// `(main_stack, list)` inside a `NocturneSongQueue`.
pub(crate) fn queue_parts(q: &song::SongQueue) -> (gtk::Stack, gtk::ListBox) {
    let list = q.list();
    let stack: gtk::Stack = list.parent().and_downcast().expect("song queue stack");
    (stack, list)
}

/// Call `f` when the content does not fill the scrolled window (to load more).
pub(crate) fn on_short_content(sw: &gtk::ScrolledWindow, f: impl Fn() + 'static) {
    let f = Rc::new(f);
    sw.vadjustment().connect_upper_notify(move |adj| {
        let (adj, f) = (adj.clone(), f.clone());
        glib::timeout_add_local_once(std::time::Duration::from_secs(1), move || {
            if adj.upper() <= adj.page_size() {
                f();
            }
        });
    });
}

// ---------- searchable list pages (albums / artists / songs / playlists) ----------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Album,
    Artist,
    Song,
    Playlist,
}

impl Kind {
    fn widgets(self, id: &str) -> (gtk::Widget, gtk::Widget) {
        match self {
            Kind::Album => (album::AlbumRow::new(id).upcast(), album::AlbumButton::new(id).upcast()),
            Kind::Artist => (artist::ArtistRow::new(id).upcast(), artist::ArtistButton::new(id).upcast()),
            Kind::Song => (song::SongRow::new(id).upcast(), song::SongButton::new(id).upcast()),
            Kind::Playlist => (playlist::PlaylistRow::new(id).upcast(), playlist::PlaylistButton::new(id).upcast()),
        }
    }
}

/// Template children shared by the searchable list pages.
#[derive(Clone)]
pub struct ListParts {
    pub main_stack: gtk::Stack,
    pub list: gtk::ListBox,
    /// Inner stack of a `NocturneSongQueue`, if the list is one.
    pub list_stack: Option<gtk::Stack>,
    pub wrap: gtk::Widget,
    pub end_stack: gtk::Stack,
    pub entry: gtk::SearchEntry,
}

#[derive(Default)]
pub struct Lister {
    pub offset: Cell<u32>,
    pub searching: Cell<bool>,
    pub generation: Cell<u32>,
    pub lazy: Cell<bool>,
    pub items: RefCell<HashMap<String, (gtk::Widget, gtk::Widget)>>,
}

impl Lister {
    pub fn reset(&self, p: &ListParts) {
        self.offset.set(0);
        p.list.remove_all();
        wrap_clear(&p.wrap);
        self.items.borrow_mut().clear();
    }

    pub fn load(self: &Rc<Self>, kind: Kind, p: &ListParts) {
        if self.items.borrow().is_empty() {
            self.on_search(kind, p);
        }
    }

    pub fn reload(self: &Rc<Self>, kind: Kind, p: &ListParts) {
        self.lazy.set(false);
        self.reset(p);
        self.load(kind, p);
    }

    pub fn on_search(self: &Rc<Self>, kind: Kind, p: &ListParts) {
        self.offset.set(0);
        self.searching.set(false);
        self.generation.set(self.generation.get() + 1);
        for (r, b) in self.items.borrow().values() {
            r.set_visible(false);
            b.set_visible(false);
        }
        p.end_stack.set_visible_child_name("loading");
        self.search(kind, p);
    }

    pub fn edge_reached(self: &Rc<Self>, kind: Kind, p: &ListParts, pos: gtk::PositionType) {
        if pos == gtk::PositionType::Bottom && p.end_stack.visible_child_name().as_deref() == Some("loading") {
            self.search(kind, p);
        }
    }

    pub fn search(self: &Rc<Self>, kind: Kind, p: &ListParts) {
        if self.searching.get() || integration::current().is_none() {
            return;
        }
        self.searching.set(true);
        let (st, p) = (self.clone(), p.clone());
        let generation = self.generation.get();
        spawn_local(async move {
            let integ = integration::get();
            let count = 30;
            let hide_singles = kind == Kind::Album && settings().boolean("hide-singles");
            loop {
                let offset = st.offset.get();
                let query = p.entry.text().to_string();
                let mut counts = SearchCounts::default();
                let pair = (count, offset);
                match kind {
                    Kind::Album => counts.album = pair,
                    Kind::Artist => counts.artist = pair,
                    Kind::Song => counts.song = pair,
                    Kind::Playlist => counts.playlist = pair,
                }
                let (ar, al, so, pl) = integ.search(&query, counts).await;
                if st.generation.get() != generation {
                    return;
                }
                if p.entry.text() != query {
                    continue;
                }
                let ids = match kind {
                    Kind::Album => al,
                    Kind::Artist => ar,
                    Kind::Song => so,
                    Kind::Playlist => pl,
                };
                let mut skipped = 0;
                for id in &ids {
                    let skip = hide_singles && integ.album(id).songCount() <= 1;
                    skipped += skip as usize;
                    let mut items = st.items.borrow_mut();
                    if let Some((r, b)) = items.get(id) {
                        r.set_visible(!skip);
                        b.set_visible(!skip);
                    } else if !skip {
                        let (r, b) = kind.widgets(id);
                        p.list.append(&r);
                        wrap_append(&p.wrap, &b);
                        items.insert(id.clone(), (r, b));
                    }
                }
                let shown = ids.len() - skipped;
                p.end_stack.set_visible_child_name(if shown < count as usize { "end" } else { "loading" });
                st.offset.set(offset + count);
                if !(ids.len() >= count as usize && skipped > 0) {
                    break;
                }
            }
            st.searching.set(false);
            let visible = any_visible(&[p.list.upcast_ref(), &p.wrap]);
            p.main_stack.set_visible_child_name(if visible { "content" } else { "no-content" });
            if let Some(s) = &p.list_stack {
                s.set_visible_child_name(if visible { "content" } else { "no-content" });
            }
        });
    }
}

/// Bind a toggle group's `active-name` (Adw.ToggleGroup, libadwaita 1.7) to a setting.
pub(crate) fn bind_view_mode(toggle: &gtk::Widget, key: &str) {
    settings().bind(key, toggle, "active-name").build();
}

/// Implements [`LibraryPage`] plus the template callbacks of a searchable list page.
macro_rules! list_page {
    ($Ty:ident, $kind:expr) => {
        impl $crate::widgets::pages::LibraryPage for $Ty {
            fn load(&self) {
                self.imp().lister.load($kind, &self.parts())
            }
            fn reload(&self) {
                self.imp().lister.reload($kind, &self.parts())
            }
            fn reset(&self) {
                self.imp().lister.reset(&self.parts())
            }
            fn set_lazy_reload(&self) {
                self.imp().lister.lazy.set(true)
            }
        }

        #[gtk::template_callbacks]
        impl $Ty {
            #[template_callback]
            fn on_search(&self, _entry: &gtk::SearchEntry) {
                self.imp().lister.on_search($kind, &self.parts())
            }
            #[template_callback]
            fn scroll_edge_reached(&self, pos: gtk::PositionType, _sw: &gtk::ScrolledWindow) {
                self.imp().lister.edge_reached($kind, &self.parts(), pos)
            }
        }
    };
}
pub(crate) use list_page;

/// Set an enum property by nick on an object whose type has no Rust bindings.
pub(crate) fn set_enum_property(obj: &impl IsA<glib::Object>, prop: &str, nick: &str) {
    let obj = obj.as_ref();
    let Some(pspec) = obj.find_property(prop) else { return };
    if let Some(v) = glib::EnumClass::with_type(pspec.value_type()).and_then(|c| c.to_value_by_nick(nick)) {
        obj.set_property_from_value(prop, &v);
    }
}

/// Average colour of a texture, for subtle gradients.
pub(crate) fn average_color(tex: &gtk::gdk::Texture) -> (u8, u8, u8) {
    let (w, h) = (tex.width() as usize, tex.height() as usize);
    let stride = w * 4;
    let mut buf = vec![0u8; stride * h];
    tex.download(&mut buf, stride);
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for px in buf.chunks_exact(4).step_by(7) {
        // GDK_MEMORY_DEFAULT is premultiplied BGRA (little endian ARGB32).
        b += px[0] as u64;
        g += px[1] as u64;
        r += px[2] as u64;
        n += 1;
    }
    let n = n.max(1);
    ((r / n) as u8, (g / n) as u8, (b / n) as u8)
}
