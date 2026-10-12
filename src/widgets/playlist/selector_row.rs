//! `NocturnePlaylistSelectorRow` (playlist/selector_row.blp): a playlist in the "add to playlist" dialog.
use crate::{integration, models::Playlist};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/dev/arjix/Nocturne/playlist/selector_row.ui")]
    #[properties(wrapper_type = super::PlaylistSelectorRow)]
    pub struct PlaylistSelectorRow {
        #[template_child]
        pub cover_el: TemplateChild<gtk::Image>,
        #[property(get, set)]
        pub model: RefCell<Option<Playlist>>,
    }

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

    #[glib::derived_properties]
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

impl PlaylistSelectorRow {
    pub fn new(id: &str) -> Self {
        let model = integration::get().playlist(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        super::watch(&obj, &model, |o, m| {
            let name = m.name();
            o.set_title(&name);
            o.set_widget_name(&name);
            o.set_tooltip_text(Some(&format!("Add Songs to '{name}'")));
            o.set_subtitle(&super::songs_label(m.songCount()));
            super::apply_cover(&o.imp().cover_el, m.gdkPaintable(), 48, false);
        });
        let (w, m) = (obj.downgrade(), model.clone());
        super::load(id, false, move || {
            if let Some(o) = w.upgrade() {
                super::apply_cover(&o.imp().cover_el, m.gdkPaintable(), 48, true);
            }
        });
        obj
    }
}
