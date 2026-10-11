//! `NocturneArtistPage` (artist/page.blp).
use super::ArtistButton;
use crate::widgets::album::AlbumButton;
use crate::widgets::containers::{Carousel, Wrapbox, context::{self, Formatters}, wrapbox::wrap_box_append};
use crate::widgets::song::SongSmallRow;
use crate::{integration, models::Artist, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib, pango};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/jeffser/Nocturne/artist/page.ui")]
    #[properties(wrapper_type = super::ArtistPage)]
    pub struct ArtistPage {
        #[template_child]
        pub clamp_el: TemplateChild<adw::Clamp>,
        #[template_child]
        pub avatar_el: TemplateChild<adw::Avatar>,
        #[template_child]
        pub name_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub biography_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub star_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub top_songs_wrapbox: TemplateChild<Wrapbox>,
        #[template_child]
        pub album_wrapbox: TemplateChild<Wrapbox>,
        #[template_child]
        pub artist_carousel: TemplateChild<Carousel>,
        #[template_child]
        pub rating_container: TemplateChild<gtk::Box>,
        /// `Adw.WrapBox`
        #[template_child]
        pub context_wrap_el: TemplateChild<gtk::Widget>,
        #[property(get, set)]
        pub model: RefCell<Option<Artist>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ArtistPage {
        const NAME: &'static str = "NocturneArtistPage";
        type Type = super::ArtistPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            Wrapbox::ensure_type();
            Carousel::ensure_type();
            klass.bind_template();
            klass.bind_template_callbacks();
            Formatters::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl ArtistPage {
        #[template_callback(function)]
        fn format_avatar_paintable(p: Option<gdk::Paintable>) -> Option<gdk::Paintable> {
            p
        }
        #[template_callback]
        fn on_biography_clicked(&self, _button: &gtk::Button) {
            let bio = self.model.borrow().as_ref().map(|m| m.biography()).unwrap_or_default();
            let label = &self.biography_el;
            if label.ellipsize() == pango::EllipsizeMode::None {
                label.set_text(&bio.replace('\n', " "));
                label.set_ellipsize(pango::EllipsizeMode::End);
            } else {
                label.set_text(&bio);
                label.set_ellipsize(pango::EllipsizeMode::None);
            }
        }
        #[template_callback]
        fn change_rating(&self, button: &gtk::Button) {
            if let Some(m) = self.model.borrow().as_ref() {
                context::change_rating(&*self.obj(), &m.id(), button);
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ArtistPage {}
    impl WidgetImpl for ArtistPage {}
    impl NavigationPageImpl for ArtistPage {}
}

glib::wrapper! {
    pub struct ArtistPage(ObjectSubclass<imp::ArtistPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ArtistPage {
    pub fn new(id: &str) -> Self {
        let model = integration::get().artist(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        let imp = obj.imp();
        context::track_star(&*imp.star_el, &model);
        for b in context::buttons(&context::artist_items(), id, true, None) {
            wrap_box_append(&*imp.context_wrap_el, &b);
        }
        let list = imp.top_songs_wrapbox.list();
        context::set_enum(&list, "justify", "fill");
        list.set_property("justify-last-line", true);
        list.set_property("child-spacing", 5i32);
        list.set_property("line-spacing", 5i32);

        let w = obj.downgrade();
        model.connect_notify_local(Some("album"), move |_, _| {
            if let Some(o) = w.upgrade() {
                o.update_album_list();
            }
        });
        let w = obj.downgrade();
        model.connect_notify_local(Some("similarArtist"), move |_, _| {
            if let Some(o) = w.upgrade() {
                o.update_artist_list();
            }
        });
        let w = obj.downgrade();
        model.connect_notify_local(Some("gdkPaintable"), move |m, _| {
            if let Some(o) = w.upgrade() {
                context::tint_background(&*o.imp().clamp_el, m.gdkPaintable().as_ref());
            }
        });
        context::tint_background(&*imp.clamp_el, model.gdkPaintable().as_ref());
        obj.update_album_list();
        obj.update_artist_list();
        obj.reload();

        let (w, id) = (obj.downgrade(), id.to_string());
        spawn_local(async move {
            let top = integration::get().top_songs(&id, 10).await;
            let Some(o) = w.upgrade() else { return };
            let wb = &o.imp().top_songs_wrapbox;
            wb.set_visible(top.len() > 5);
            if top.len() > 5 {
                wb.set_widgets(top.iter().map(|s| SongSmallRow::with_album_name(s, true)));
            }
        });
        obj
    }

    pub fn reload(&self) {
        let Some(id) = self.model().map(|m| m.id()) else { return };
        spawn_local(async move {
            let i = integration::get();
            i.verify_artist(&id, true).await;
            i.load_cover(&id).await;
        });
    }

    fn update_album_list(&self) {
        let Some(m) = self.model() else { return };
        let albums = m.album().0;
        if !albums.is_empty() {
            self.imp().album_wrapbox.set_widgets(albums.iter().map(|a| {
                let b = AlbumButton::new(a);
                b.set_artist_page_style();
                b
            }));
        }
    }

    fn update_artist_list(&self) {
        let Some(m) = self.model() else { return };
        self.imp().artist_carousel.set_widgets(m.similarArtist().0.iter().map(|a| ArtistButton::new(&a.id)));
    }
}
