//! `NocturneWrapbox` (containers/wrapbox.blp): header + `Adw.WrapBox` of widgets.
use super::context::Formatters;
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/jeffser/Nocturne/containers/wrapbox.ui")]
    #[properties(wrapper_type = super::Wrapbox)]
    pub struct Wrapbox {
        #[template_child]
        pub header_button: TemplateChild<gtk::Button>,
        /// `Adw.WrapBox` (libadwaita 1.7): managed through plain `GtkWidget` API.
        #[template_child]
        pub list_el: TemplateChild<gtk::Widget>,
        #[property(get, set)]
        pub header_label: RefCell<String>,
        #[property(get, set)]
        pub header_icon_name: RefCell<String>,
        #[property(get, set)]
        pub header_page_tag: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Wrapbox {
        const NAME: &'static str = "NocturneWrapbox";
        type Type = super::Wrapbox;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Formatters::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for Wrapbox {}
    impl WidgetImpl for Wrapbox {}
    impl BoxImpl for Wrapbox {}
}

glib::wrapper! {
    pub struct Wrapbox(ObjectSubclass<imp::Wrapbox>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

/// Append to an `Adw.WrapBox` (same as `adw_wrap_box_append`).
pub fn wrap_box_append(wrap_box: &impl IsA<gtk::Widget>, child: &impl IsA<gtk::Widget>) {
    child.as_ref().insert_before(wrap_box.as_ref(), None::<&gtk::Widget>);
}

/// Children of any widget, in order.
pub fn children(w: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    std::iter::successors(w.as_ref().first_child(), |c| c.next_sibling()).collect()
}

impl Wrapbox {
    /// The inner `Adw.WrapBox` (set `justify`, `child-spacing`… with `set_property`).
    pub fn list(&self) -> gtk::Widget {
        self.imp().list_el.get()
    }

    pub fn children(&self) -> Vec<gtk::Widget> {
        children(&self.list())
    }

    pub fn remove_all(&self) {
        for c in self.children() {
            c.unparent();
        }
    }

    pub fn append(&self, w: &impl IsA<gtk::Widget>) {
        wrap_box_append(&self.list(), w);
    }

    /// Replace the children; hides the whole container when empty.
    pub fn set_widgets<W: IsA<gtk::Widget>>(&self, widgets: impl IntoIterator<Item = W>) {
        self.remove_all();
        for w in widgets {
            self.append(&w);
        }
        self.set_visible(self.list().first_child().is_some());
    }
}
