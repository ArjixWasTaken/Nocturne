//! `NocturnePlaylistRow` (playlist/row.blp).
use crate::widgets::containers::context;
use crate::{config, integration, models::Playlist};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/jeffser/Nocturne/playlist/row.ui")]
    #[properties(wrapper_type = super::PlaylistRow)]
    pub struct PlaylistRow {
        #[template_child]
        pub cover_el: TemplateChild<gtk::Image>,
        #[template_child]
        pub menu_button_el: TemplateChild<gtk::MenuButton>,
        #[property(get, set)]
        pub model: RefCell<Option<Playlist>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistRow {
        const NAME: &'static str = "NocturnePlaylistRow";
        type Type = super::PlaylistRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl PlaylistRow {
        #[template_callback]
        fn on_context_button_active(&self, button: &gtk::MenuButton) {
            if let (Some(p), Some(m)) = (button.popover(), self.model.borrow().as_ref()) {
                p.set_child(Some(&context::container(&context::playlist_items(), &m.id())));
            }
        }
        #[template_callback]
        fn show_popover(&self, #[rest] args: &[glib::Value]) {
            let Some(m) = self.model.borrow().clone() else { return };
            let (_, x, y) = context::gesture_point(args);
            context::popup_items(&*self.obj(), &context::playlist_items(), &m.id(), x, y);
        }
    }

    #[glib::derived_properties]
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
    pub fn new(id: &str) -> Self {
        let model = integration::get().playlist(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        obj.set_action_target_value(Some(&id.to_variant()));
        config::settings().bind("show-context-button", &*obj.imp().menu_button_el, "visible").build();
        super::watch(&obj, &model, |o, m| {
            o.set_title(&m.name());
            o.set_widget_name(&m.name());
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
