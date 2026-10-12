//! `NocturneDownloadRow` (containers/download_row.blp): one entry of the downloads queue.
use crate::{integration, models::SongDownload};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/containers/download_row.ui")]
    pub struct DownloadRow {
        #[template_child]
        pub title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub done_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub progressbar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub done_button: TemplateChild<gtk::Button>,
        pub model: RefCell<Option<SongDownload>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DownloadRow {
        const NAME: &'static str = "NocturneDownloadRow";
        type Type = super::DownloadRow;
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl DownloadRow {
        #[template_callback]
        fn progressbar_frac_changed(&self, _pspec: glib::ParamSpec, bar: &gtk::ProgressBar) {
            if bar.fraction() >= 1.0 {
                self.done_button.set_visible(true);
                self.done_label.set_visible(true);
                bar.set_visible(false);
            }
        }
        #[template_callback]
        fn remove_from_queue(&self) {
            self.obj().remove_from_queue();
        }
    }

    impl ObjectImpl for DownloadRow {}
    impl WidgetImpl for DownloadRow {}
    impl ListBoxRowImpl for DownloadRow {}
}

glib::wrapper! {
    pub struct DownloadRow(ObjectSubclass<imp::DownloadRow>)
        @extends gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl DownloadRow {
    pub fn new(model: &SongDownload) -> Self {
        let obj: Self = glib::Object::new();
        let imp = obj.imp();
        let song = integration::get().song(&model.id());
        song.bind_property("title", &*imp.title_label, "label").sync_create().build();
        song.bind_property("title", &obj, "tooltip-text").sync_create().build();
        model.bind_property("progress", &*imp.progressbar, "fraction").sync_create().build();
        imp.model.replace(Some(model.clone()));
        let id = model.id();
        crate::runtime::spawn_local(async move {
            integration::get().verify_song(&id, false).await;
        });
        obj
    }

    pub fn is_done(&self) -> bool {
        self.imp().done_button.is_visible()
    }

    pub fn remove_from_queue(&self) {
        let Some(m) = self.imp().model.borrow().clone() else { return };
        let queue = integration::get().current_state().downloadQueueModel();
        if let Some(pos) = queue.iter::<SongDownload>().flatten().position(|d| d.id() == m.id()) {
            queue.remove(pos as u32);
        }
    }
}
