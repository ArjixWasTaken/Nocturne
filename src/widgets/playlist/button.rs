//! `NocturnePlaylistButton` (playlist/button.blp). STUB: to be ported.
#![allow(unused_imports)]
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/playlist/button.ui")]
    pub struct PlaylistButton {}

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistButton {
        const NAME: &'static str = "NocturnePlaylistButton";
        type Type = super::PlaylistButton;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

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
    /// Widget for the model with `id` (TODO: port).
    pub fn new(_id: &str) -> Self {
        glib::Object::new()
    }
}
