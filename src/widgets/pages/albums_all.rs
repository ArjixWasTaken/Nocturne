//! `NocturneAlbumsAllPage` (pages/albums_all.blp): searchable album list/grid.
use super::{Kind, ListParts, Lister, bind_view_mode, list_page, on_short_content};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::rc::Rc;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/pages/albums_all.ui")]
    pub struct AlbumsAllPage {
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
    impl ObjectSubclass for AlbumsAllPage {
        const NAME: &'static str = "NocturneAlbumsAllPage";
        type Type = super::AlbumsAllPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AlbumsAllPage {
        fn constructed(&self) {
            self.parent_constructed();
            bind_view_mode(&self.toggle_group_el, "default-view-mode-albums");
            let weak = self.obj().downgrade();
            on_short_content(&self.scrolledwindow, move || {
                if let Some(o) = weak.upgrade() {
                    o.imp().lister.search(Kind::Album, &o.parts());
                }
            });
        }
    }
    impl WidgetImpl for AlbumsAllPage {}
    impl NavigationPageImpl for AlbumsAllPage {
        fn showing(&self) {
            self.parent_showing();
            if self.lister.lazy.get() {
                self.lister.reload(Kind::Album, &self.obj().parts());
            }
        }
    }
}

glib::wrapper! {
    pub struct AlbumsAllPage(ObjectSubclass<imp::AlbumsAllPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl AlbumsAllPage {
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

list_page!(AlbumsAllPage, Kind::Album);
