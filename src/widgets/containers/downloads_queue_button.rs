//! `NocturneDownloadsQueueButton` (containers/downloads_queue_button.blp).
//! The template is an `Adw.Bin` wrapping the `Gtk.MenuButton` (`menu_button`).
use super::DownloadRow;
use crate::{integration, models::SongDownload};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/containers/downloads_queue_button.ui")]
    pub struct DownloadsQueueButton {
        #[template_child]
        pub menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub download_list_el: TemplateChild<gtk::ListBox>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DownloadsQueueButton {
        const NAME: &'static str = "NocturneDownloadsQueueButton";
        type Type = super::DownloadsQueueButton;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl DownloadsQueueButton {
        #[template_callback]
        fn clear_done_downloads(&self) {
            let mut row = self.download_list_el.first_child();
            while let Some(r) = row {
                row = r.next_sibling();
                if let Some(r) = r.downcast_ref::<DownloadRow>().filter(|r| r.is_done()) {
                    r.remove_from_queue();
                }
            }
        }
        #[template_callback]
        fn button_activated(&self) {
            self.menu_button.remove_css_class("accent");
        }
    }

    impl ObjectImpl for DownloadsQueueButton {}
    impl WidgetImpl for DownloadsQueueButton {}
    impl BinImpl for DownloadsQueueButton {}
}

glib::wrapper! {
    pub struct DownloadsQueueButton(ObjectSubclass<imp::DownloadsQueueButton>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl DownloadsQueueButton {
    /// Bind to the current integration's download queue (call after login).
    pub fn setup(&self) {
        let model: gio::ListStore = integration::get().current_state().downloadQueueModel();
        self.imp().download_list_el.bind_model(Some(&model), |o| DownloadRow::new(o.downcast_ref::<SongDownload>().unwrap()).upcast());
        let this = self.downgrade();
        let changed = move |m: &gio::ListStore| {
            let Some(this) = this.upgrade() else { return };
            this.set_visible(m.n_items() > 0);
            if m.n_items() > 0 {
                this.imp().menu_button.add_css_class("accent");
            }
        };
        changed(&model);
        model.connect_items_changed(move |m, _, _, _| changed(m));
    }
}
