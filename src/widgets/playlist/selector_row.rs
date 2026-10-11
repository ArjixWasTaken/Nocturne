//! `NocturnePlaylistSelectorRow` (playlist/selector_row.blp). STUB: to be ported.
#![allow(unused_imports)]
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/playlist/selector_row.ui")]
    pub struct PlaylistSelectorRow {}

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistSelectorRow {
        const NAME: &'static str = "NocturnePlaylistSelectorRow";
        type Type = super::PlaylistSelectorRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlaylistSelectorRow {}
    impl WidgetImpl for PlaylistSelectorRow {}
    impl ListBoxRowImpl for PlaylistSelectorRow {}
    impl PreferencesRowImpl for PlaylistSelectorRow {}
    impl ActionRowImpl for PlaylistSelectorRow {}
}

glib::wrapper! {
    pub struct PlaylistSelectorRow(ObjectSubclass<imp::PlaylistSelectorRow>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}
