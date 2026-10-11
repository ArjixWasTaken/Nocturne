//! `NocturnePlaylistDialog` (playlist/dialog.blp). STUB: to be ported.
#![allow(unused_imports)]
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/playlist/dialog.ui")]
    pub struct PlaylistDialog {}

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistDialog {
        const NAME: &'static str = "NocturnePlaylistDialog";
        type Type = super::PlaylistDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlaylistDialog {}
    impl WidgetImpl for PlaylistDialog {}
    impl AdwDialogImpl for PlaylistDialog {}
}

glib::wrapper! {
    pub struct PlaylistDialog(ObjectSubclass<imp::PlaylistDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PlaylistDialog {
    /// Create (`None`) or edit (`Some(id)`) a playlist (TODO: port).
    pub fn new(_id: Option<&str>) -> Self {
        glib::Object::new()
    }
}

/// Ask which playlist to add `songs` to (TODO: port).
pub fn prompt_add_songs(_parent: &impl IsA<gtk::Widget>, _songs: Vec<String>) {}
