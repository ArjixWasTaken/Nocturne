//! `NocturneCarousel` (containers/carousel.blp): horizontally paged row of widgets.
use super::context::Formatters;
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/jeffser/Nocturne/containers/carousel.ui")]
    #[properties(wrapper_type = super::Carousel)]
    pub struct Carousel {
        #[template_child]
        pub header_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub list_el: TemplateChild<adw::Carousel>,
        #[template_child]
        pub pan_container_el: TemplateChild<gtk::Box>,
        #[template_child]
        pub pan_start_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub pan_end_el: TemplateChild<gtk::Button>,
        #[property(get, set)]
        pub header_label: RefCell<String>,
        #[property(get, set)]
        pub header_icon_name: RefCell<String>,
        #[property(get, set)]
        pub header_page_tag: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Carousel {
        const NAME: &'static str = "NocturneCarousel";
        type Type = super::Carousel;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
            Formatters::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl Carousel {
        #[template_callback]
        fn on_scroll(&self, controller: &gtk::EventControllerScroll, dx: f64, dy: f64) -> bool {
            let list = &*self.list_el;
            let pos = list.position();
            if pos == pos.trunc() {
                let shift = controller.current_event_state().contains(gdk::ModifierType::SHIFT_MASK);
                if shift || dx != 0.0 {
                    let dir = if dy != 0.0 { dy } else { dx };
                    let next = (pos + dir).clamp(0.0, list.n_pages().saturating_sub(1) as f64) as u32;
                    if next < list.n_pages() {
                        list.scroll_to(&list.nth_page(next), true);
                    }
                }
            }
            false
        }
        #[template_callback]
        fn pan_start(&self) {
            self.obj().pan(false);
        }
        #[template_callback]
        fn pan_end(&self) {
            self.obj().pan(true);
        }
        #[template_callback]
        fn page_changed(&self, _carousel: &adw::Carousel, index: u32) {
            self.pan_start_el.set_sensitive(index != 0);
            self.pan_end_el.set_sensitive(index + 1 != self.list_el.n_pages());
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for Carousel {}
    impl WidgetImpl for Carousel {}
    impl BoxImpl for Carousel {}
}

glib::wrapper! {
    pub struct Carousel(ObjectSubclass<imp::Carousel>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Carousel {
    pub fn list(&self) -> adw::Carousel {
        self.imp().list_el.get()
    }

    pub fn remove_all(&self) {
        let list = self.list();
        while list.n_pages() > 0 {
            list.remove(&list.nth_page(0));
        }
    }

    /// Replace the pages (hides the carousel when empty) and scroll to the middle.
    pub fn set_widgets<W: IsA<gtk::Widget>>(&self, widgets: impl IntoIterator<Item = W>) {
        self.remove_all();
        let list = self.list();
        for w in widgets {
            list.append(&w);
        }
        let n = list.n_pages();
        self.set_visible(n > 0);
        self.imp().pan_container_el.set_visible(n >= 2);
        glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            if list.n_pages() > 0 {
                list.scroll_to(&list.nth_page((list.n_pages() - 1) / 2), true);
            }
        });
    }

    fn pan(&self, to_end: bool) {
        let list = self.list();
        if list.n_pages() == 0 {
            return;
        }
        let first = list.nth_page(0);
        let visible = (list.width() / first.width().max(1)).max(1) as f64;
        let next = list.position() + if to_end { visible } else { -visible };
        let next = next.clamp(0.0, (list.n_pages() - 1) as f64) as u32;
        list.scroll_to(&list.nth_page(next), true);
    }
}
