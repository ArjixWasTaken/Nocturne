//! `NocturneAlbumsPage` (pages/albums.blp): album lists (random, newest, …) picked by the page tag.
use super::{LibraryPage, on_short_content, wrap_append};
use crate::{backend::AlbumListKind, config::settings, integration, runtime::spawn_local, widgets::album::AlbumButton};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/pages/albums.ui")]
    pub struct AlbumsPage {
        #[template_child]
        pub list_el: TemplateChild<crate::widgets::containers::Wrapbox>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub end_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub scrolledwindow: TemplateChild<gtk::ScrolledWindow>,
        pub offset: Cell<u32>,
        pub loading: Cell<bool>,
        pub lazy: Cell<bool>,
        pub buttons: RefCell<HashMap<String, gtk::Widget>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AlbumsPage {
        const NAME: &'static str = "NocturneAlbumsPage";
        type Type = super::AlbumsPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AlbumsPage {
        fn constructed(&self) {
            self.parent_constructed();
            let weak = self.obj().downgrade();
            on_short_content(&self.scrolledwindow, move || {
                if let Some(o) = weak.upgrade() {
                    o.load_albums();
                }
            });
        }
    }
    impl WidgetImpl for AlbumsPage {}
    impl NavigationPageImpl for AlbumsPage {
        fn showing(&self) {
            self.parent_showing();
            if self.lazy.get() {
                self.obj().reload();
            }
        }
    }
}

glib::wrapper! {
    pub struct AlbumsPage(ObjectSubclass<imp::AlbumsPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl LibraryPage for AlbumsPage {
    fn load(&self) {
        let imp = self.imp();
        if !imp.buttons.borrow().is_empty() {
            return;
        }
        imp.main_stack.set_visible_child_name("loading");
        imp.offset.set(0);
        imp.loading.set(false);
        imp.lazy.set(false);
        self.reset();
        imp.end_stack.set_visible_child_name("loading");
        self.load_albums();
    }

    fn reload(&self) {
        self.reset();
        self.load();
    }

    fn reset(&self) {
        self.imp().list_el.remove_all();
        self.imp().buttons.borrow_mut().clear();
    }

    fn set_lazy_reload(&self) {
        self.imp().lazy.set(true);
    }
}

#[gtk::template_callbacks]
impl AlbumsPage {
    fn load_albums(&self) {
        let imp = self.imp();
        if imp.loading.get() || integration::current().is_none() {
            return;
        }
        imp.loading.set(true);
        let obj = self.clone();
        spawn_local(async move {
            let imp = obj.imp();
            let integ = integration::get();
            let tag = obj.tag().unwrap_or_default();
            let kind = AlbumListKind::from_name(tag.split('-').nth(1).unwrap_or("random"));
            let hide_singles = settings().boolean("hide-singles");
            let albums = integ.album_list(kind, 20, imp.offset.get()).await;
            let inner = imp.list_el.list();
            let mut skipped = 0;
            for id in &albums {
                let skip = hide_singles && integ.album(id).songCount() <= 1;
                skipped += skip as usize;
                let mut buttons = imp.buttons.borrow_mut();
                if let Some(b) = buttons.get(id) {
                    b.set_visible(!skip);
                } else if !skip {
                    let b: gtk::Widget = AlbumButton::new(id).upcast();
                    wrap_append(&inner, &b);
                    buttons.insert(id.clone(), b);
                }
            }
            imp.end_stack.set_visible_child_name(if albums.len() - skipped < 20 { "end" } else { "loading" });
            imp.offset.set(imp.offset.get() + 20);
            imp.loading.set(false);
            let visible = imp.buttons.borrow().values().any(|b| b.is_visible());
            imp.main_stack.set_visible_child_name(if visible { "content" } else { "no-content" });
        });
    }

    #[template_callback]
    fn scroll_edge_reached(&self, _sw: &gtk::ScrolledWindow, pos: gtk::PositionType) {
        if pos == gtk::PositionType::Bottom && self.imp().end_stack.visible_child_name().as_deref() == Some("loading") {
            self.load_albums();
        }
    }
}
