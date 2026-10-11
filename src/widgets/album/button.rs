//! `NocturneAlbumButton` (album/button.blp).
use crate::widgets::containers::context::{self, Formatters};
use crate::{integration, models::Album, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/jeffser/Nocturne/album/button.ui")]
    #[properties(wrapper_type = super::AlbumButton)]
    pub struct AlbumButton {
        #[template_child]
        pub artist_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub star_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub cover_el: TemplateChild<gtk::Image>,
        #[template_child]
        pub name_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub year_el: TemplateChild<gtk::Label>,
        #[property(get, set)]
        pub model: RefCell<Option<Album>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AlbumButton {
        const NAME: &'static str = "NocturneAlbumButton";
        type Type = super::AlbumButton;
        type ParentType = gtk::Box;

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
    impl AlbumButton {
        #[template_callback(function)]
        #[allow(non_snake_case)]
        fn format_gdkPaintable(p: Option<gdk::Paintable>) -> Option<gdk::Paintable> {
            p
        }
        #[template_callback]
        fn show_popover_image(&self, #[rest] args: &[glib::Value]) {
            self.popover(args, false);
        }
        #[template_callback]
        fn show_popover_name(&self, #[rest] args: &[glib::Value]) {
            self.popover(args, false);
        }
        #[template_callback]
        fn show_popover_artist(&self, #[rest] args: &[glib::Value]) {
            self.popover(args, true);
        }
    }

    impl AlbumButton {
        fn popover(&self, args: &[glib::Value], artist: bool) {
            let Some(m) = self.model.borrow().clone() else { return };
            let (w, x, y) = context::gesture_point(args);
            let w = w.unwrap_or_else(|| self.obj().clone().upcast());
            if artist {
                if !m.artistId().is_empty() {
                    context::popup_items(&w, &context::artist_items(), &m.artistId(), x, y);
                }
            } else if !m.id().is_empty() {
                context::popup_items(&w, &context::album_items(), &m.id(), x, y);
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AlbumButton {}
    impl WidgetImpl for AlbumButton {}
    impl BoxImpl for AlbumButton {}
}

glib::wrapper! {
    pub struct AlbumButton(ObjectSubclass<imp::AlbumButton>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl AlbumButton {
    pub fn new(id: &str) -> Self {
        let model = integration::get().album(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        let imp = obj.imp();
        context::track_star(&*imp.star_el, &model);
        context::track_button_size(Some(&imp.cover_el), None, &imp.name_el);
        let id = id.to_string();
        spawn_local(async move {
            let i = integration::get();
            i.verify_album(&id, false).await;
            i.load_cover(&id).await;
        });
        obj
    }

    /// Variant used in the artist page: show the year instead of the artist.
    pub fn set_artist_page_style(&self) {
        let imp = self.imp();
        imp.year_el.set_visible(true);
        imp.artist_el.set_visible(false);
        self.set_halign(gtk::Align::Center);
        imp.name_el.remove_css_class("title-3");
    }
}
