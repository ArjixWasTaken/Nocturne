//! `NocturneArtistButton` (artist/button.blp).
use crate::widgets::containers::context::{self, Formatters};
use crate::{integration, models::Artist, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/dev/arjix/Nocturne/artist/button.ui")]
    #[properties(wrapper_type = super::ArtistButton)]
    pub struct ArtistButton {
        #[template_child]
        pub avatar_el: TemplateChild<adw::Avatar>,
        #[template_child]
        pub name_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub album_count_el: TemplateChild<gtk::Label>,
        #[property(get, set)]
        pub model: RefCell<Option<Artist>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ArtistButton {
        const NAME: &'static str = "NocturneArtistButton";
        type Type = super::ArtistButton;
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
    impl ArtistButton {
        #[template_callback(function)]
        #[allow(non_snake_case)]
        fn format_gdkPaintable(p: Option<gdk::Paintable>) -> Option<gdk::Paintable> {
            p
        }
        #[template_callback]
        fn show_popover(&self, #[rest] args: &[glib::Value]) {
            let Some(m) = self.model.borrow().clone() else { return };
            let (_, x, y) = context::gesture_point(args);
            context::popup_items(&*self.obj(), &context::artist_items(), &m.id(), x, y);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ArtistButton {}
    impl WidgetImpl for ArtistButton {}
    impl ButtonImpl for ArtistButton {}
}

glib::wrapper! {
    pub struct ArtistButton(ObjectSubclass<imp::ArtistButton>)
        @extends gtk::Button, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl ArtistButton {
    pub fn new(id: &str) -> Self {
        let model = integration::get().artist(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        let imp = obj.imp();
        context::track_button_size(None, Some(&imp.avatar_el), &imp.name_el);
        let id = id.to_string();
        spawn_local(async move {
            let i = integration::get();
            i.verify_artist(&id, false).await;
            i.load_cover(&id).await;
        });
        obj
    }
}
