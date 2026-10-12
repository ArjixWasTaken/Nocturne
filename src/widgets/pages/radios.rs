//! `NocturneRadiosPage` (pages/radios.blp): internet radio stations.
use super::LibraryPage;
use crate::{integration, runtime::spawn_local, widgets::song::SongRow};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/pages/radios.ui")]
    pub struct RadiosPage {
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub list_el: TemplateChild<gtk::ListBox>,
        pub rows: RefCell<Vec<(String, gtk::Widget)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RadiosPage {
        const NAME: &'static str = "NocturneRadiosPage";
        type Type = super::RadiosPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RadiosPage {}
    impl WidgetImpl for RadiosPage {}
    impl NavigationPageImpl for RadiosPage {}
}

glib::wrapper! {
    pub struct RadiosPage(ObjectSubclass<imp::RadiosPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl LibraryPage for RadiosPage {
    fn load(&self) {
        let Some(integ) = integration::current() else { return };
        self.imp().main_stack.set_visible_child_name("loading");
        let obj = self.clone();
        spawn_local(async move {
            let radios = integ.radios().await;
            obj.reset();
            let imp = obj.imp();
            for id in radios {
                let row: gtk::Widget = SongRow::new(&id).upcast();
                imp.list_el.append(&row);
                imp.rows.borrow_mut().push((id, row));
            }
            obj.update_visibility();
        });
    }

    fn reload(&self) {
        self.load();
    }

    fn reset(&self) {
        self.imp().list_el.remove_all();
        self.imp().rows.borrow_mut().clear();
    }

    fn set_lazy_reload(&self) {}
}

#[gtk::template_callbacks]
impl RadiosPage {
    fn update_visibility(&self) {
        let imp = self.imp();
        let any = imp.rows.borrow().iter().any(|(_, r)| r.is_visible());
        imp.main_stack.set_visible_child_name(if any { "content" } else { "no-content" });
    }

    #[template_callback]
    fn on_search(&self, entry: &gtk::SearchEntry) {
        let query = entry.text().to_lowercase();
        if let Some(integ) = integration::current() {
            for (id, row) in self.imp().rows.borrow().iter() {
                row.set_visible(integ.song(id).title().to_lowercase().contains(&query));
            }
        }
        self.update_visibility();
    }
}
