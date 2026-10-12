//! `NocturneSetupPage` (pages/setup.blp). STUB: to be ported.
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
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
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
