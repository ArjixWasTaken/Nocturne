//! `NocturneLyricsDialog` (lyrics/dialog.blp). STUB: to be ported.
#![allow(unused_imports)]
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/lyrics/dialog.ui")]
    pub struct LyricsDialog {}

    #[glib::object_subclass]
    impl ObjectSubclass for LyricsDialog {
        const NAME: &'static str = "NocturneLyricsDialog";
        type Type = super::LyricsDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LyricsDialog {}
    impl WidgetImpl for LyricsDialog {}
    impl AdwDialogImpl for LyricsDialog {}
}

glib::wrapper! {
    pub struct LyricsDialog(ObjectSubclass<imp::LyricsDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl LyricsDialog {
    /// Lyrics editor for the current song (TODO: port).
    pub fn new() -> Self {
        glib::Object::new()
    }
}
