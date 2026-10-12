//! `NocturnePlaylistPage` (playlist/page.blp).
use crate::widgets::containers::{context::{self, ItemsExt}, wrapbox::wrap_box_append};
use crate::widgets::song::SongQueue;
use crate::{config, integration, models::Playlist};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::{Cell, RefCell};

const PAGE: usize = 50;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/dev/arjix/Nocturne/playlist/page.ui")]
    #[properties(wrapper_type = super::PlaylistPage)]
    pub struct PlaylistPage {
        #[template_child]
        pub clamp_el: TemplateChild<adw::Clamp>,
        #[template_child]
        pub cover_el: TemplateChild<gtk::Image>,
        #[template_child]
        pub name_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub song_count_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub duration_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub song_list_el: TemplateChild<SongQueue>,
        /// `Adw.WrapBox`
        #[template_child]
        pub context_wrap_el: TemplateChild<gtk::Widget>,
        #[property(get, set)]
        pub model: RefCell<Option<Playlist>>,
        pub song_ids: RefCell<Vec<String>>,
        pub offset: Cell<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistPage {
        const NAME: &'static str = "NocturnePlaylistPage";
        type Type = super::PlaylistPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            SongQueue::ensure_type();
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl PlaylistPage {
        #[template_callback]
        fn scroll_edge_reached(&self, _sw: &gtk::ScrolledWindow, pos: gtk::PositionType) {
            if pos == gtk::PositionType::Bottom {
                self.obj().load_song_rows();
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PlaylistPage {}
    impl WidgetImpl for PlaylistPage {}
    impl NavigationPageImpl for PlaylistPage {}
}

glib::wrapper! {
    pub struct PlaylistPage(ObjectSubclass<imp::PlaylistPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PlaylistPage {
    pub fn new(id: &str) -> Self {
        let model = integration::get().playlist(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        let imp = obj.imp();
        imp.song_list_el.set_playlist_id(id);
        for b in context::buttons(&context::playlist_items().without(&["edit", "delete"]), id, true, None) {
            wrap_box_append(&*imp.context_wrap_el, &b);
        }
        super::watch(&obj, &model, |o, m| {
            let imp = o.imp();
            let name = m.name();
            imp.name_el.set_label(&name);
            imp.name_el.set_visible(!name.is_empty());
            o.set_title(if name.is_empty() { "Playlist" } else { &name });
            o.set_widget_name(&name);
            imp.song_count_el.set_label(&super::songs_label(m.songCount()));
            imp.song_count_el.set_visible(m.songCount() > 0);
            imp.duration_el.set_label(&config::display_time(m.duration() as i64));
            imp.duration_el.set_visible(m.duration() > 0);
        });
        let w = obj.downgrade();
        model.connect_notify_local(Some("gdkPaintable"), move |m, _| {
            if let Some(o) = w.upgrade() {
                o.update_cover(m, false);
            }
        });
        let w = obj.downgrade();
        model.connect_notify_local(Some("entry"), move |_, _| {
            if let Some(o) = w.upgrade() {
                o.update_song_list();
            }
        });
        obj.update_cover(&model, false);
        obj.update_song_list();
        obj.reload();
        obj
    }

    pub fn reload(&self) {
        let Some(m) = self.model() else { return };
        let w = self.downgrade();
        super::load(&m.id(), true, move || {
            if let Some(o) = w.upgrade() {
                o.update_cover(&m, true);
            }
        });
    }

    fn update_cover(&self, m: &Playlist, done: bool) {
        let p = m.gdkPaintable();
        if p.is_some() {
            context::tint_background(&*self.imp().clamp_el, p.as_ref());
        }
        super::apply_cover(&self.imp().cover_el, p, 240, done);
    }

    fn update_song_list(&self) {
        let Some(m) = self.model() else { return };
        let imp = self.imp();
        let ids = m.entry().0;
        imp.song_list_el.clear();
        imp.song_list_el.set_state(if ids.is_empty() { "no-content" } else { "content" });
        imp.song_ids.replace(ids);
        imp.offset.set(0);
        self.load_song_rows();
    }

    /// Append the next page of rows (playlists can hold thousands of songs).
    fn load_song_rows(&self) {
        let imp = self.imp();
        let ids = imp.song_ids.borrow();
        let start = imp.offset.get();
        if start >= ids.len() {
            return;
        }
        let end = (start + PAGE).min(ids.len());
        let origin = self.model().map(|m| m.id()).unwrap_or_default();
        imp.song_list_el.append_songs(&ids[start..end], &ids, Some(&origin));
        imp.offset.set(end);
    }
}
