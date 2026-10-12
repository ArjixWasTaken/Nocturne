//! `NocturnePlaybackModeButton` (playing/playback_mode_button.blp).
use crate::config::settings;
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};

pub const PLAYBACK_MODES: [(&str, &str, &str); 3] = [
    ("consecutive", "media-playlist-consecutive-symbolic", "Consecutive"),
    ("repeat-all", "media-playlist-repeat-symbolic", "Repeat All"),
    ("repeat-one", "media-playlist-repeat-song-symbolic", "Repeat One"),
];

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/playing/playback_mode_button.ui")]
    pub struct PlaybackModeButton {
        #[template_child]
        pub button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub container: TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaybackModeButton {
        const NAME: &'static str = "NocturnePlaybackModeButton";
        type Type = super::PlaybackModeButton;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlaybackModeButton {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            for (name, icon, label) in PLAYBACK_MODES {
                let b = gtk::Button::builder().css_classes(["flat"]).icon_name(icon).tooltip_text(label).build();
                b.connect_clicked(glib::clone!(#[weak] obj, move |_| obj.mode_changed(name)));
                self.container.append(&b);
            }
            settings().connect_changed(Some("playback-mode"), glib::clone!(#[weak] obj, move |s, k| obj.mode_changed(&s.string(k))));
            obj.mode_changed(&settings().string("playback-mode"));
        }
    }
    impl WidgetImpl for PlaybackModeButton {}
    impl BinImpl for PlaybackModeButton {}
}

glib::wrapper! {
    pub struct PlaybackModeButton(ObjectSubclass<imp::PlaybackModeButton>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PlaybackModeButton {
    fn mode_changed(&self, name: &str) {
        let button = &self.imp().button;
        if let Some((_, icon, label)) = PLAYBACK_MODES.iter().find(|m| m.0 == name) {
            button.set_icon_name(icon);
            button.set_tooltip_text(Some(label));
        }
        button.popdown();
        let s = settings();
        if s.string("playback-mode") != name {
            let _ = s.set_string("playback-mode", name);
        }
    }
}
