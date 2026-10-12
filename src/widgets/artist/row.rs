//! `NocturneArtistRow` (artist/row.blp).
use crate::widgets::containers::context::{self, Formatters};
use crate::{config, integration, models::Artist, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/dev/arjix/Nocturne/artist/row.ui")]
    #[properties(wrapper_type = super::ArtistRow)]
    pub struct ArtistRow {
        #[template_child]
        pub avatar_el: TemplateChild<adw::Avatar>,
        #[template_child]
        pub menu_button_el: TemplateChild<gtk::MenuButton>,
        #[property(get, set)]
        pub model: RefCell<Option<Artist>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ArtistRow {
        const NAME: &'static str = "NocturneArtistRow";
        type Type = super::ArtistRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
            Formatters::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl ArtistRow {
        #[template_callback]
        fn on_context_button_active(&self, _pspec: glib::ParamSpec, button: &gtk::MenuButton) {
            if let (Some(p), Some(m)) = (button.popover(), self.model.borrow().as_ref()) {
                p.set_child(Some(&context::container(&context::artist_items(), &m.id())));
            }
        }
        #[template_callback]
        fn show_popover(&self, #[rest] args: &[glib::Value]) {
            let Some(m) = self.model.borrow().clone() else { return };
            let (_, x, y) = context::gesture_point(args);
            context::popup_items(&*self.obj(), &context::artist_items(), &m.id(), x, y);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ArtistRow {}
    impl WidgetImpl for ArtistRow {}
    impl ListBoxRowImpl for ArtistRow {}
    impl PreferencesRowImpl for ArtistRow {}
    impl ActionRowImpl for ArtistRow {}
}

glib::wrapper! {
    pub struct ArtistRow(ObjectSubclass<imp::ArtistRow>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl ArtistRow {
    pub fn new(id: &str) -> Self {
        let model = integration::get().artist(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        config::settings().bind("show-context-button", &*obj.imp().menu_button_el, "visible").build();
        let id = id.to_string();
        spawn_local(async move {
            let i = integration::get();
            i.verify_artist(&id, false).await;
            i.load_cover(&id).await;
        });
        obj
    }
}
