//! `NocturneHomePage` (pages/home.blp): overview carousels and global search.
use super::{LibraryPage, average_color};
use crate::{
    backend::{AlbumListKind, SearchCounts},
    config::{data_dir, settings},
    integration,
    runtime::spawn_local,
    widgets::{album::AlbumButton, artist::ArtistButton, containers::{Carousel, Wrapbox}, playlist::PlaylistButton, song::SongSmallRow},
};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib};
use std::cell::Cell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/pages/home.ui")]
    pub struct HomePage {
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub search_toggle: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub main_clamp: TemplateChild<adw::Clamp>,
        #[template_child]
        pub main_container: TemplateChild<gtk::Box>,
        #[template_child]
        pub frequent_album_carousel: TemplateChild<Carousel>,
        #[template_child]
        pub welcome_container: TemplateChild<gtk::Box>,
        #[template_child]
        pub welcome_avatar: TemplateChild<adw::Avatar>,
        #[template_child]
        pub welcome_username_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub song_wrapbox: TemplateChild<Wrapbox>,
        #[template_child]
        pub album_carousel: TemplateChild<Carousel>,
        #[template_child]
        pub artist_carousel: TemplateChild<Carousel>,
        #[template_child]
        pub playlist_carousel: TemplateChild<Carousel>,
        #[template_child]
        pub refresh_button_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub refresh_stack_el: TemplateChild<gtk::Stack>,
        pub css_provider: gtk::CssProvider,
        pub loaded: Cell<bool>,
        pub lazy: Cell<bool>,
        pub searching: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HomePage {
        const NAME: &'static str = "NocturneHomePage";
        type Type = super::HomePage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for HomePage {
        fn constructed(&self) {
            self.parent_constructed();
            #[allow(deprecated)]
            self.main_clamp.style_context().add_provider(&self.css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            if let Some(rev) = self.search_bar.first_child().and_downcast::<gtk::Revealer>() {
                rev.set_transition_type(gtk::RevealerTransitionType::Crossfade);
            }
            let inner = self.song_wrapbox.list();
            inner.set_margin_start(10);
            inner.set_margin_end(10);
            crate::widgets::containers::context::set_enum(&inner, "justify", "fill");
            inner.set_property("justify-last-line", true);
            inner.set_property("child-spacing", 5i32);
            inner.set_property("line-spacing", 5i32);
        }
    }
    impl WidgetImpl for HomePage {}
    impl NavigationPageImpl for HomePage {
        fn showing(&self) {
            self.parent_showing();
            if self.lazy.get() {
                self.obj().reload();
            }
        }
    }
}

glib::wrapper! {
    pub struct HomePage(ObjectSubclass<imp::HomePage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for HomePage {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl LibraryPage for HomePage {
    fn load(&self) {
        let imp = self.imp();
        if imp.loaded.replace(true) {
            return;
        }
        self.search();
        self.search_mode_toggled(&imp.search_toggle);
        self.update_welcome_visibility();
    }

    fn reload(&self) {
        self.imp().loaded.set(false);
        self.imp().lazy.set(false);
        self.load();
    }

    fn reset(&self) {
        let imp = self.imp();
        for c in [&imp.frequent_album_carousel, &imp.album_carousel, &imp.artist_carousel, &imp.playlist_carousel] {
            c.remove_all(); c.set_visible(false);
        }
        imp.song_wrapbox.set_widgets(Vec::<gtk::Widget>::new());
    }

    fn set_lazy_reload(&self) {
        self.imp().lazy.set(true);
    }
}

fn widgets<T: IsA<gtk::Widget>>(ids: &[String], f: impl Fn(&str) -> T) -> Vec<gtk::Widget> {
    ids.iter().map(|id| f(id).upcast()).collect()
}

#[gtk::template_callbacks]
impl HomePage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Show or hide the search bar (`app.search`).
    pub fn toggle_search(&self) {
        let bar = &self.imp().search_bar;
        bar.set_search_mode(!bar.is_search_mode());
    }

    fn update_welcome_visibility(&self) {
        let imp = self.imp();
        let s = settings();
        let (mode, user) = (s.string("welcome-mode-home"), s.string("welcome-user-home"));
        imp.welcome_avatar.set_custom_image(None::<&gdk::Paintable>);
        imp.css_provider.load_from_string("clamp {background: none;}");
        if mode.is_empty() || user.is_empty() {
            imp.welcome_container.set_visible(false);
            return;
        }
        imp.welcome_container.set_visible(true);
        if let Ok(tex) = gdk::Texture::from_filename(data_dir().join("pfp")) {
            imp.welcome_avatar.set_custom_image(Some(&tex));
            let (r, g, b) = average_color(&tex);
            imp.css_provider.load_from_string(&format!(
                "clamp {{ transition: background .2s; background: linear-gradient(180deg, color-mix(in srgb, rgb({r},{g},{b}) 25%, transparent), transparent 30%); background-size: 100% 1000px; background-repeat: no-repeat; }}"
            ));
        }
        imp.welcome_username_label.set_label(&user);
    }

    fn search(&self) {
        let imp = self.imp();
        let Some(integ) = integration::current() else { return };
        if imp.searching.replace(true) {
            return;
        }
        imp.refresh_stack_el.set_visible_child_name("spinner");
        imp.refresh_button_el.set_sensitive(false);
        imp.main_stack.set_visible_child_name("loading");
        let obj = self.clone();
        spawn_local(async move {
            let imp = obj.imp();
            let s = settings();
            let n = |k: &str| s.int(k).max(0) as u32;
            let (n_freq, n_songs, n_albums, n_artists, n_playlists) =
                (n("n-frequent-albums-home"), n("n-songs-home"), n("n-albums-home"), n("n-artists-home"), n("n-playlists-home"));
            let (frequent, songs, mut albums, artists, mut playlists);
            if !imp.search_entry.text().is_empty() {
                let mut query = String::new();
                let mut res = Default::default();
                while query != imp.search_entry.text() {
                    query = imp.search_entry.text().into();
                    let counts = SearchCounts {
                        artist: (n_artists, 0),
                        album: (n_albums, 0),
                        song: (n_songs, 0),
                        playlist: (n_playlists, 0),
                    };
                    res = integ.search(&query, counts).await;
                }
                (artists, albums, songs, playlists) = res;
                frequent = vec![];
                if s.boolean("hide-singles") {
                    albums.retain(|id| integ.album(id).songCount() > 1);
                }
            } else {
                frequent = if n_freq > 0 { integ.album_list(AlbumListKind::Frequent, n_freq, 0).await } else { vec![] };
                songs = if n_songs > 0 { integ.random_songs(n_songs).await } else { vec![] };
                albums = if n_albums > 0 { integ.album_list(AlbumListKind::Random, n_albums, 0).await } else { vec![] };
                artists = if n_artists > 0 { integ.artists(n_artists).await } else { vec![] };
                playlists = integ.playlists().await;
                playlists.truncate(n_playlists as usize);
            }
            let has_results = [&frequent, &songs, &albums, &artists, &playlists].iter().any(|v| !v.is_empty());
            imp.frequent_album_carousel.set_widgets(widgets(&frequent, AlbumButton::new));
            imp.song_wrapbox.set_widgets(widgets(&songs, SongSmallRow::new));
            imp.album_carousel.set_widgets(widgets(&albums, AlbumButton::new));
            imp.artist_carousel.set_widgets(widgets(&artists, ArtistButton::new));
            imp.playlist_carousel.set_widgets(widgets(&playlists, PlaylistButton::new));
            imp.main_stack.set_visible_child_name(if has_results { "content" } else { "no-content" });
            imp.searching.set(false);
            imp.refresh_stack_el.set_visible_child_name("icon");
            imp.refresh_button_el.set_sensitive(true);
        });
    }

    #[template_callback]
    fn refresh(&self, _button: &gtk::Button) {
        self.reload();
    }

    #[template_callback]
    fn search_mode_toggled(&self, button: &gtk::ToggleButton) {
        let h = self.imp().header_bar.height();
        self.imp().main_container.set_margin_top(if button.is_active() { 0 } else if h > 0 { h } else { 46 });
    }

    #[template_callback]
    fn on_search(&self, _entry: &gtk::SearchEntry) {
        self.search();
    }
}
