//! `NocturnePlaylistPage` (playlist/page.blp). STUB: to be ported.
#![allow(unused_imports)]
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/playlist/page.ui")]
    pub struct PlaylistPage {}

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistPage {
        const NAME: &'static str = "NocturnePlaylistPage";
        type Type = super::PlaylistPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

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
    /// Widget for the model with `id` (TODO: port).
    pub fn new(_id: &str) -> Self {
        glib::Object::new()
    }
}
