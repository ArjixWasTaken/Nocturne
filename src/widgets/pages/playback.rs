//! `NocturnePlaybackPage` (pages/playback.blp): last month's most played songs.
use super::{queue_parts, wrap_append, wrap_clear};
use crate::widgets::song::{SongButton, SongQueue, SongRow};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/pages/playback.ui")]
    pub struct PlaybackPage {
        #[template_child]
        pub top_wrapbox: TemplateChild<gtk::Widget>,
        #[template_child]
        pub song_queue: TemplateChild<SongQueue>,
        #[template_child]
        pub playlist_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub thanks_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaybackPage {
        const NAME: &'static str = "NocturnePlaybackPage";
        type Type = super::PlaybackPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlaybackPage {}
    impl WidgetImpl for PlaybackPage {}
    impl NavigationPageImpl for PlaybackPage {}
}

glib::wrapper! {
    pub struct PlaybackPage(ObjectSubclass<imp::PlaybackPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl PlaybackPage {
    /// Show `songs` (`(id, plays)`, most played first) for `month` (e.g. "March 2026").
    pub fn setup(&self, songs: &[(String, i64)], month: &str) {
        let imp = self.imp();
        wrap_clear(&imp.top_wrapbox);
        let (stack, list) = queue_parts(&imp.song_queue);
        list.remove_all();
        imp.thanks_label.set_visible(songs.len() >= 50);
        imp.playlist_stack.set_visible_child_name("active");
        let target = glib::VariantDict::new(None);
        target.insert("new_playlist", format!("Nocturne Playback ~ {month}"));
        target.insert("songs", songs.iter().map(|s| s.0.clone()).collect::<Vec<_>>());
        if let Some(btn) = imp.playlist_stack.child_by_name("active").and_downcast::<gtk::Button>() {
            btn.set_action_target_value(Some(&target.end()));
            btn.set_action_name(Some("app.add_songs_to_playlist"));
        }
        for (rank, (id, plays)) in songs.iter().enumerate() {
            if rank < 5 {
                let b = gtk::Box::new(gtk::Orientation::Vertical, 5);
                let rank_label = gtk::Label::new(Some(&format!("#{}", rank + 1)));
                rank_label.add_css_class("title-1");
                b.append(&rank_label);
                b.append(&SongButton::new(id));
                let plays_label = gtk::Label::new(Some(&format!("{plays} Plays")));
                plays_label.add_css_class("dimmed");
                b.append(&plays_label);
                wrap_append(&*imp.top_wrapbox, &b);
            } else {
                let row = SongRow::new(id);
                row.set_tooltip_text(Some(&format!("#{} · {plays} Plays", rank + 1)));
                list.append(&row);
            }
        }
        stack.set_visible_child_name("content");
    }

    #[template_callback]
    fn go_home(&self, _button: &gtk::Button) {
        let window = crate::application::Application::get().main_window();
        window.show_content();
        window.replace_root_page("home");
    }

    #[template_callback]
    fn save_playlist(&self, _button: &gtk::Button) {
        self.imp().playlist_stack.set_visible_child_name("inactive");
    }
}
