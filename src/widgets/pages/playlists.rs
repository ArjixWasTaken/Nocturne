//! `NocturnePlaylistsPage` (pages/playlists.blp): searchable playlist list/grid.
use super::{Kind, ListParts, Lister, bind_view_mode, list_page, on_short_content};
use adw::subclass::prelude::*;
use gtk::{CompositeTemplate, glib};
use std::rc::Rc;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/pages/playlists.ui")]
    pub struct PlaylistsPage {
        #[template_child]
        pub toggle_group_el: TemplateChild<gtk::Widget>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub list_el: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub wrapbox_el: TemplateChild<gtk::Widget>,
        #[template_child]
        pub end_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub scrolledwindow: TemplateChild<gtk::ScrolledWindow>,
        pub lister: Rc<Lister>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistsPage {
        const NAME: &'static str = "NocturnePlaylistsPage";
        type Type = super::PlaylistsPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlaylistsPage {
        fn constructed(&self) {
            self.parent_constructed();
            bind_view_mode(&self.toggle_group_el, "default-view-mode-playlists");
            let weak = self.obj().downgrade();
            on_short_content(&self.scrolledwindow, move || {
                if let Some(o) = weak.upgrade() {
                    o.imp().lister.search(Kind::Playlist, &o.parts());
                }
            });
        }
    }
    impl WidgetImpl for PlaylistsPage {}
    impl NavigationPageImpl for PlaylistsPage {
        fn showing(&self) {
            self.parent_showing();
            if self.lister.lazy.get() {
                self.lister.reload(Kind::Playlist, &self.obj().parts());
            }
        }
    }
}

glib::wrapper! {
    pub struct PlaylistsPage(ObjectSubclass<imp::PlaylistsPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PlaylistsPage {
    fn parts(&self) -> ListParts {
        let imp = self.imp();
        let (list, list_stack) = (imp.list_el.get(), None);
        ListParts {
            main_stack: imp.main_stack.get(),
            list,
            list_stack,
            wrap: imp.wrapbox_el.get(),
            end_stack: imp.end_stack.get(),
            entry: imp.search_entry.get(),
        }
    }
}

list_page!(PlaylistsPage, Kind::Playlist);
