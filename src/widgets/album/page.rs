//! `NocturneAlbumPage` (album/page.blp).
use crate::widgets::containers::{context::{self, Formatters, ItemsExt}, wrapbox::wrap_box_append};
use crate::widgets::song::{SongQueue, SongRow};
use crate::{integration, models::Album, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/dev/arjix/Nocturne/album/page.ui")]
    #[properties(wrapper_type = super::AlbumPage)]
    pub struct AlbumPage {
        #[template_child]
        pub clamp_el: TemplateChild<adw::Clamp>,
        #[template_child]
        pub star_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub song_list_el: TemplateChild<SongQueue>,
        /// `Adw.WrapBox`
        #[template_child]
        pub context_wrap_el: TemplateChild<gtk::Widget>,
        #[property(get, set)]
        pub model: RefCell<Option<Album>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AlbumPage {
        const NAME: &'static str = "NocturneAlbumPage";
        type Type = super::AlbumPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            SongQueue::ensure_type();
            klass.bind_template();
            klass.bind_template_callbacks();
            Formatters::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl AlbumPage {
        #[template_callback(function)]
        fn format_cover_pixel_size(p: Option<gdk::Paintable>) -> i32 {
            if p.is_some() { 240 } else { -1 }
        }
        #[template_callback]
        fn change_rating(&self, button: &gtk::Button) {
            if let Some(m) = self.model.borrow().as_ref() {
                context::change_rating(&*self.obj(), &m.id(), button);
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AlbumPage {}
    impl WidgetImpl for AlbumPage {}
    impl NavigationPageImpl for AlbumPage {}
}

glib::wrapper! {
    pub struct AlbumPage(ObjectSubclass<imp::AlbumPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

fn disc_of(row: &gtk::ListBoxRow) -> (i32, i32) {
    match row.downcast_ref::<SongRow>().and_then(|r| r.model()) {
        Some(m) => (m.discNumber(), m.track()),
        None => (row.widget_name().strip_prefix("disc-").and_then(|n| n.parse().ok()).unwrap_or(0), -1),
    }
}

impl AlbumPage {
    pub fn new(id: &str) -> Self {
        let model = integration::get().album(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        let imp = obj.imp();
        context::track_star(&*imp.star_el, &model);
        for b in context::buttons(&context::album_items().without(&["show-artist"]), id, true, None) {
            wrap_box_append(&*imp.context_wrap_el, &b);
        }
        imp.song_list_el.list().set_sort_func(|a, b| disc_of(a).cmp(&disc_of(b)).into());

        let w = obj.downgrade();
        model.connect_notify_local(Some("song"), move |_, _| {
            if let Some(o) = w.upgrade() {
                o.update_song_list();
            }
        });
        let w = obj.downgrade();
        model.connect_notify_local(Some("gdkPaintable"), move |m, _| {
            if let Some(o) = w.upgrade() {
                context::tint_background(&*o.imp().clamp_el, m.gdkPaintable().as_ref());
            }
        });
        context::tint_background(&*imp.clamp_el, model.gdkPaintable().as_ref());
        obj.update_song_list();
        obj.reload();
        obj
    }

    /// Re-fetch the album from the server.
    pub fn reload(&self) {
        let Some(id) = self.model().map(|m| m.id()) else { return };
        spawn_local(async move {
            let i = integration::get();
            i.verify_album(&id, true).await;
            i.load_cover(&id).await;
        });
    }

    fn update_song_list(&self) {
        let Some(m) = self.model() else { return };
        let ids = m.song().0;
        let queue = &*self.imp().song_list_el;
        if queue.rows().len() == ids.len() && !ids.is_empty() {
            return;
        }
        queue.set_songs(&ids, Some(&m.id()));
        if ids.is_empty() {
            return;
        }
        let integ = integration::get();
        let mut discs: Vec<i32> = ids.iter().map(|i| integ.song(i).discNumber()).filter(|d| *d > 0).collect();
        discs.sort();
        discs.dedup();
        if !discs.is_empty() {
            for d in discs {
                let label = gtk::Label::builder().label(format!("Disc {d}")).xalign(0.0).halign(gtk::Align::Start).css_classes(["title-2"]).build();
                let row = gtk::ListBoxRow::builder().child(&label).css_classes(["p10"]).activatable(false).name(format!("disc-{d}")).build();
                queue.list().append(&row);
            }
        }
        queue.list().invalidate_sort();
    }
}
