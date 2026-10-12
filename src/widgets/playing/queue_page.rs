//! `NocturnePlayingQueuePage` (playing/queue_page.blp): the play queue.
use super::on_login;
use crate::{config::settings, widgets::song::SongRow};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/playing/queue_page.ui")]
    pub struct PlayingQueuePage {
        #[template_child]
        pub song_list_el: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub autoplay_row_el: TemplateChild<adw::SwitchRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlayingQueuePage {
        const NAME: &'static str = "NocturnePlayingQueuePage";
        type Type = super::PlayingQueuePage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlayingQueuePage {
        fn constructed(&self) {
            self.parent_constructed();
            settings().bind("auto-play", &*self.autoplay_row_el, "active").build();
            on_login(&*self.obj(), |obj, state| {
                obj.imp().song_list_el.bind_model(Some(&state.queueModel()), |item| {
                    let id = item.downcast_ref::<gtk::StringObject>().map(|s| s.string()).unwrap_or_default();
                    SongRow::new(&id).upcast()
                });
            });
        }
    }
    impl WidgetImpl for PlayingQueuePage {}
    impl BinImpl for PlayingQueuePage {}
}

glib::wrapper! {
    pub struct PlayingQueuePage(ObjectSubclass<imp::PlayingQueuePage>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
