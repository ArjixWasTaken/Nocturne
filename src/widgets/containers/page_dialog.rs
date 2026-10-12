//! `NocturnePageDialog` (containers/page_dialog.blp): navigation view in a dialog (popout window).
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/containers/page_dialog.ui")]
    pub struct PageDialog {
        #[template_child]
        pub navigation_view: TemplateChild<adw::NavigationView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PageDialog {
        const NAME: &'static str = "NocturnePageDialog";
        type Type = super::PageDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PageDialog {}
    impl WidgetImpl for PageDialog {}
    impl AdwDialogImpl for PageDialog {}
}

glib::wrapper! {
    pub struct PageDialog(ObjectSubclass<imp::PageDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PageDialog {
    /// Dialog hosting a navigation page (used from the popout window).
    pub fn new(page: &impl IsA<adw::NavigationPage>) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().navigation_view.replace(&[page.clone().upcast()]);
        obj
    }

    /// Push another page onto this dialog.
    pub fn push(&self, page: &impl IsA<adw::NavigationPage>) {
        self.imp().navigation_view.push(page);
    }

    pub fn navigation_view(&self) -> adw::NavigationView {
        self.imp().navigation_view.get()
    }
}
