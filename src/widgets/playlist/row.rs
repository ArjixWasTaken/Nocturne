//! `NocturnePlaylistRow` (playlist/row.blp). STUB: to be ported.
#![allow(unused_imports)]
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/playlist/row.ui")]
    pub struct PlaylistRow {}

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistRow {
        const NAME: &'static str = "NocturnePlaylistRow";
        type Type = super::PlaylistRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlaylistRow {}
    impl WidgetImpl for PlaylistRow {}
    impl ListBoxRowImpl for PlaylistRow {}
    impl PreferencesRowImpl for PlaylistRow {}
    impl ActionRowImpl for PlaylistRow {}
}

glib::wrapper! {
    pub struct PlaylistRow(ObjectSubclass<imp::PlaylistRow>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl PlaylistRow {
    /// Widget for the model with `id` (TODO: port).
    pub fn new(_id: &str) -> Self {
        glib::Object::new()
    }
}
