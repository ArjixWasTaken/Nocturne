//! `NocturnePlaylistButton` (playlist/button.blp).
use crate::widgets::containers::context;
use crate::{integration, models::Playlist};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/dev/arjix/Nocturne/playlist/button.ui")]
    #[properties(wrapper_type = super::PlaylistButton)]
    pub struct PlaylistButton {
        #[template_child]
        pub play_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub cover_button_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub cover_el: TemplateChild<gtk::Image>,
        #[template_child]
        pub name_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub name_label_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub song_count_label_el: TemplateChild<gtk::Label>,
        #[property(get, set)]
        pub model: RefCell<Option<Playlist>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistButton {
        const NAME: &'static str = "NocturnePlaylistButton";
        type Type = super::PlaylistButton;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl PlaylistButton {
        #[template_callback]
        fn show_popover(&self, #[rest] args: &[glib::Value]) {
            let Some(m) = self.model.borrow().clone() else { return };
            let (_, x, y) = context::gesture_point(args);
            context::popup_items(&*self.obj(), &context::playlist_items(), &m.id(), x, y);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PlaylistButton {}
    impl WidgetImpl for PlaylistButton {}
    impl BoxImpl for PlaylistButton {}
}

glib::wrapper! {
    pub struct PlaylistButton(ObjectSubclass<imp::PlaylistButton>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PlaylistButton {
    pub fn new(id: &str) -> Self {
        let model = integration::get().playlist(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        let imp = obj.imp();
        let target = id.to_variant();
        for b in [&*imp.play_el, &*imp.cover_button_el, &*imp.name_el] {
            b.set_action_target_value(Some(&target));
        }
        context::track_button_size(Some(&imp.cover_el), None, &imp.name_label_el);
        super::watch(&obj, &model, |o, m| {
            let imp = o.imp();
            let name = m.name();
            imp.name_el.set_tooltip_text(Some(&name));
            imp.name_label_el.set_label(&name);
            imp.cover_button_el.set_tooltip_text(Some(&name));
            o.set_widget_name(&name);
            imp.song_count_label_el.set_label(&super::songs_label(m.songCount()));
            imp.song_count_label_el.set_visible(m.songCount() > 0);
            super::apply_cover(&imp.cover_el, m.gdkPaintable(), imp.cover_el.height_request(), false);
        });
        let (w, m) = (obj.downgrade(), model.clone());
        super::load(id, false, move || {
            if let Some(o) = w.upgrade() {
                let c = &o.imp().cover_el;
                super::apply_cover(c, m.gdkPaintable(), c.height_request(), true);
            }
        });
        obj
    }
}
