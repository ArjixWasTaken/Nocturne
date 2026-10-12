//! `NocturneLyricEditRow` (lyrics/edit_row.blp): one timed line in the lyrics editor.
use crate::{application::Application, integration};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::Cell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/lyrics/edit_row.ui")]
    pub struct LyricEditRow {
        #[template_child]
        pub ts_button: TemplateChild<gtk::MenuButton>,
        pub ms: Cell<i64>,
        pub invalid: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LyricEditRow {
        const NAME: &'static str = "NocturneLyricEditRow";
        type Type = super::LyricEditRow;
        type ParentType = adw::EntryRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    fn popdown(b: &gtk::Button) {
        if let Some(p) = b.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>() {
            p.popdown();
        }
    }

    #[gtk::template_callbacks]
    impl LyricEditRow {
        #[template_callback]
        fn go_to_timestamp(&self, button: &gtk::Button) {
            popdown(button);
            if let Some(p) = Application::get().player() {
                p.seek(self.ms.get() as f64 / 1000.0);
            }
        }
        #[template_callback]
        fn set_current_timestamp(&self, button: &gtk::Button) {
            popdown(button);
            self.obj().set_current_timestamp();
        }
        #[template_callback]
        fn remove(&self, button: &gtk::Button) {
            popdown(button);
            let obj = self.obj();
            let dialog = obj.ancestor(super::super::LyricsDialog::static_type()).and_downcast::<super::super::LyricsDialog>();
            if let Some(lb) = obj.parent().and_downcast::<gtk::ListBox>() {
                lb.remove(&*obj);
            }
            if let Some(d) = dialog {
                d.update_visibility();
            }
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
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget, gtk::Editable;
}

/// `mm:ss.xx` (`h:mm:ss.xx` past an hour), as used in LRC tags.
pub fn format_ms(ms: i64) -> String {
    let ms = ms.max(0);
    let (h, m, s) = (ms / 3_600_000, (ms % 3_600_000) / 60_000, (ms % 60_000) as f64 / 1000.0);
    if h > 0 { format!("{h}:{m:02}:{s:05.2}") } else { format!("{m:02}:{s:05.2}") }
}

impl LyricEditRow {
    pub fn new(ms: i64, content: &str, invalid: bool) -> Self {
        let obj: Self = glib::Object::builder().property("text", content).build();
        obj.imp().ms.set(ms);
        obj.imp().invalid.set(invalid);
        if invalid {
            obj.add_css_class("error");
        }
        obj.show_timestamp();
        obj
    }

    pub fn ms(&self) -> i64 {
        self.imp().ms.get()
    }

    pub fn is_invalid(&self) -> bool {
        self.imp().invalid.get()
    }

    pub fn ts_button(&self) -> gtk::MenuButton {
        self.imp().ts_button.get()
    }

    /// Set the timestamp to the current playback position.
    pub fn set_current_timestamp(&self) {
        let ps = integration::get().current_state().positionSeconds();
        self.imp().ms.set((ps * 1000.0) as i64);
        self.imp().invalid.set(false);
        self.show_timestamp();
        if let Some(lb) = self.parent().and_downcast::<gtk::ListBox>() {
            lb.invalidate_sort();
        }
    }

    fn show_timestamp(&self) {
        if self.is_invalid() {
            self.set_title("No Timestamp");
        } else {
            self.set_title(&format_ms(self.ms()));
            self.remove_css_class("error");
        }
    }
}
