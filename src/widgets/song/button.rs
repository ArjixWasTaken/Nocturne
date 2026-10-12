//! `NocturneSongButton` (song/button.blp).
use crate::widgets::containers::context::{self, Formatters};
use crate::{integration, models::Song, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/dev/arjix/Nocturne/song/button.ui")]
    #[properties(wrapper_type = super::SongButton)]
    pub struct SongButton {
        #[template_child]
        pub cover_el: TemplateChild<gtk::Image>,
        #[template_child]
        pub cover_button_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub play_indicator_el: TemplateChild<gtk::Image>,
        #[template_child]
        pub name_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub artist_el: TemplateChild<gtk::Button>,
        #[property(get, set)]
        pub model: RefCell<Option<Song>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongButton {
        const NAME: &'static str = "NocturneSongButton";
        type Type = super::SongButton;
        type ParentType = gtk::Box;

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
    impl SongButton {
        #[template_callback(function)]
        fn format_cover_paintable(p: Option<gdk::Paintable>) -> Option<gdk::Paintable> {
            p
        }
        #[template_callback]
        fn show_popover_image(&self, #[rest] args: &[glib::Value]) {
            let Some(m) = self.model.borrow().clone() else { return };
            let (w, x, y) = context::gesture_point(args);
            let w = w.unwrap_or_else(|| self.obj().clone().upcast());
            context::popup_items(&w, &context::simple_song_items(&m.id()), &m.id(), x, y);
        }
        #[template_callback]
        fn show_popover_artist(&self, #[rest] args: &[glib::Value]) {
            let Some(m) = self.model.borrow().clone() else { return };
            if m.artistId().is_empty() {
                return;
            }
            let (_, x, y) = context::gesture_point(args);
            context::popup_items(&*self.artist_el, &context::artist_items(), &m.artistId(), x, y);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SongButton {}
    impl WidgetImpl for SongButton {}
    impl BoxImpl for SongButton {}
}

glib::wrapper! {
    pub struct SongButton(ObjectSubclass<imp::SongButton>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl SongButton {
    pub fn new(id: &str) -> Self {
        let integ = integration::get();
        let model = integ.song(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        let imp = obj.imp();
        context::track_button_size(Some(&imp.cover_el), None, &imp.name_el);
        let state = integ.current_state();
        let w = obj.downgrade();
        let changed = move |current: &str| {
            let Some(o) = w.upgrade() else { return };
            let imp = o.imp();
            let playing = o.model().is_some_and(|m| m.id() == current);
            imp.play_indicator_el.set_visible(playing);
            imp.cover_button_el.set_sensitive(!playing);
            if playing { imp.cover_button_el.add_css_class("accent") } else { imp.cover_button_el.remove_css_class("accent") }
        };
        changed(&state.songId());
        state.connect_notify_local(Some("songId"), move |s, _| changed(&s.songId()));
        let id = id.to_string();
        spawn_local(async move {
            let i = integration::get();
            i.verify_song(&id, false).await;
            i.load_cover(&id).await;
        });
        obj
    }
}
