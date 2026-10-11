//! `NocturneLyricEditRow` (lyrics/edit_row.blp). STUB: to be ported.
#![allow(unused_imports)]
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/lyrics/edit_row.ui")]
    pub struct LyricEditRow {}

    #[glib::object_subclass]
    impl ObjectSubclass for LyricEditRow {
        const NAME: &'static str = "NocturneLyricEditRow";
        type Type = super::LyricEditRow;
        type ParentType = adw::EntryRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LyricEditRow {}
    impl WidgetImpl for LyricEditRow {}
    impl ListBoxRowImpl for LyricEditRow {}
    impl PreferencesRowImpl for LyricEditRow {}
    impl EntryRowImpl for LyricEditRow {}
    impl EditableImpl for LyricEditRow {}
}

glib::wrapper! {
    pub struct LyricEditRow(ObjectSubclass<imp::LyricEditRow>)
        @extends adw::EntryRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Editable, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Actionable;
}
