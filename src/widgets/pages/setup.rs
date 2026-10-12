//! `NocturneSetupPage` (pages/setup.blp): integrated Navidrome setup, not ported.
#![allow(unused_imports)]
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/pages/setup.ui")]
    pub struct SetupPage {}

    #[glib::object_subclass]
    impl ObjectSubclass for SetupPage {
        const NAME: &'static str = "NocturneSetupPage";
        type Type = super::SetupPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    // The managed Navidrome setup is not ported; the page is hidden from the welcome screen.
    #[gtk::template_callbacks]
    impl SetupPage {
        #[template_callback]
        fn download_clicked(&self) {}
        #[template_callback]
        fn link_visited(&self) -> bool {
            false
        }
        #[template_callback]
        fn continue_clicked(&self) {}
    }

    impl ObjectImpl for SetupPage {}
    impl WidgetImpl for SetupPage {}
    impl NavigationPageImpl for SetupPage {}
}

glib::wrapper! {
    pub struct SetupPage(ObjectSubclass<imp::SetupPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
