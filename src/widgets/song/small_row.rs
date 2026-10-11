//! `NocturneSongSmallRow` (song/small_row.blp).
use crate::widgets::containers::context::{self, Formatters};
use crate::{integration, models::Song, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib};
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/jeffser/Nocturne/song/small_row.ui")]
    #[properties(wrapper_type = super::SongSmallRow)]
    pub struct SongSmallRow {
        #[template_child]
        pub cover_el: TemplateChild<gtk::Image>,
        #[template_child]
        pub title_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub subtitle_el: TemplateChild<gtk::Label>,
        #[property(get, set)]
        pub model: RefCell<Option<Song>>,
        #[property(get, set)]
        pub show_album_name: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongSmallRow {
        const NAME: &'static str = "NocturneSongSmallRow";
        type Type = super::SongSmallRow;
        type ParentType = gtk::Button;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
            Formatters::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl SongSmallRow {
        #[template_callback(function)]
        fn format_subtitle(album: Option<String>, artist: Option<String>, show_album_name: bool) -> String {
            if show_album_name { album } else { artist }.unwrap_or_default()
        }
        #[template_callback(function)]
        fn format_pixel_size(p: Option<gdk::Paintable>) -> i32 {
            if p.is_some() { 48 } else { -1 }
        }
        #[template_callback]
        fn show_popover(&self, #[rest] args: &[glib::Value]) {
            let Some(m) = self.model.borrow().clone() else { return };
            let (_, x, y) = context::gesture_point(args);
            context::popup_items(&*self.obj(), &context::simple_song_items(&m.id()), &m.id(), x, y);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SongSmallRow {}
    impl WidgetImpl for SongSmallRow {}
    impl ButtonImpl for SongSmallRow {}
}

glib::wrapper! {
    pub struct SongSmallRow(ObjectSubclass<imp::SongSmallRow>)
        @extends gtk::Button, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl SongSmallRow {
    pub fn new(id: &str) -> Self {
        Self::with_album_name(id, false)
    }

    /// `show_album_name`: subtitle shows the album instead of the artist.
    pub fn with_album_name(id: &str, show_album_name: bool) -> Self {
        let model = integration::get().song(id);
        let obj: Self = glib::Object::builder().property("model", &model).property("show-album-name", show_album_name).build();
        let w = obj.downgrade();
        model.connect_notify_local(Some("deleted"), move |m, _| {
            if let Some(o) = w.upgrade()
                && m.deleted()
            {
                o.unparent();
            }
        });
        let id = id.to_string();
        spawn_local(async move {
            let i = integration::get();
            i.verify_song(&id, false).await;
            i.load_cover(&id).await;
        });
        obj
    }
}
