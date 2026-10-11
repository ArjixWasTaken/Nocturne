//! `NocturneAlbumRow` (album/row.blp).
use crate::widgets::containers::context::{self, Formatters};
use crate::{config, integration, models::Album, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/jeffser/Nocturne/album/row.ui")]
    #[properties(wrapper_type = super::AlbumRow)]
    pub struct AlbumRow {
        #[template_child]
        pub menu_button_el: TemplateChild<gtk::MenuButton>,
        #[property(get, set)]
        pub model: RefCell<Option<Album>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AlbumRow {
        const NAME: &'static str = "NocturneAlbumRow";
        type Type = super::AlbumRow;
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
    impl AlbumRow {
        #[template_callback(function)]
        fn format_cover_pixel_size(p: Option<gdk::Paintable>) -> i32 {
            if p.is_some() { 48 } else { -1 }
        }
        #[template_callback]
        fn on_context_button_active(&self, button: &gtk::MenuButton) {
            if let (Some(p), Some(m)) = (button.popover(), self.model.borrow().as_ref()) {
                p.set_child(Some(&context::container(&context::album_items(), &m.id())));
            }
        }
        #[template_callback]
        fn show_popover(&self, #[rest] args: &[glib::Value]) {
            let Some(m) = self.model.borrow().clone() else { return };
            let (_, x, y) = context::gesture_point(args);
            context::popup_items(&*self.obj(), &context::album_items(), &m.id(), x, y);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AlbumRow {}
    impl WidgetImpl for AlbumRow {}
    impl ListBoxRowImpl for AlbumRow {}
    impl PreferencesRowImpl for AlbumRow {}
    impl ActionRowImpl for AlbumRow {}
}

glib::wrapper! {
    pub struct AlbumRow(ObjectSubclass<imp::AlbumRow>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl AlbumRow {
    pub fn new(id: &str) -> Self {
        let model = integration::get().album(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        config::settings().bind("show-context-button", &*obj.imp().menu_button_el, "visible").build();
        let id = id.to_string();
        spawn_local(async move {
            let i = integration::get();
            i.verify_album(&id, false).await;
            i.load_cover(&id).await;
        });
        obj
    }
}
