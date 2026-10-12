//! `NocturnePlayingFooter` (playing/footer.blp): mini player at the bottom of the window.
use super::{delayed_seek, follow_song, on_login, watch};
use crate::{config::settings, integration};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::Cell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PlayingFooter)]
    #[template(resource = "/dev/arjix/Nocturne/playing/footer.ui")]
    pub struct PlayingFooter {
        #[template_child]
        pub cover_el: TemplateChild<gtk::Image>,
        #[template_child]
        pub title_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub artist_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub progress_el: TemplateChild<gtk::Scale>,
        #[template_child]
        pub ro_progress_el: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub state_stack_el: TemplateChild<gtk::Stack>,
        #[template_child]
        pub detail_container: TemplateChild<gtk::Box>,
        #[property(get, set = Self::set_force_huge, name = "forceHugeMode")]
        pub force_huge_mode: Cell<bool>,
    }

    impl PlayingFooter {
        fn set_force_huge(&self, v: bool) {
            self.force_huge_mode.set(v);
            self.obj().big_mode_toggled();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlayingFooter {
        const NAME: &'static str = "NocturnePlayingFooter";
        type Type = super::PlayingFooter;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PlayingFooter {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            settings().connect_changed(Some("use-big-footer"), glib::clone!(#[weak] obj, move |_, _| obj.big_mode_toggled()));
            obj.big_mode_toggled();
            on_login(&*obj, |obj, state| obj.setup(state));
        }
    }
    impl WidgetImpl for PlayingFooter {}
    impl BinImpl for PlayingFooter {}
}

glib::wrapper! {
    pub struct PlayingFooter(ObjectSubclass<imp::PlayingFooter>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl PlayingFooter {
    fn setup(&self, state: &crate::models::CurrentState) {
        watch(state, self, "songId", |o, _| o.update_progress_visibility());
        watch(state, self, "buttonState", |o, s| o.imp().state_stack_el.set_visible_child_name(&s.buttonState()));
        watch(state, self, "displaySongTitle", |o, s| o.imp().title_el.set_label(&s.displaySongTitle()));
        watch(state, self, "displaySongArtist", |o, s| {
            let a = s.displaySongArtist();
            o.imp().artist_el.set_label(&a);
            o.imp().artist_el.set_visible(!a.is_empty());
        });
        watch(state, self, "positionSeconds", |o, s| {
            if s.seeking() {
                return;
            }
            let imp = o.imp();
            let pos = s.positionSeconds();
            let duration = imp.progress_el.adjustment().upper();
            imp.ro_progress_el.set_fraction(if duration == 0.0 { 0.0 } else { (pos / duration).clamp(0.0, 1.0) });
            imp.progress_el.set_value(pos);
        });
        follow_song(state, self, |o, song| {
            let imp = o.imp();
            imp.progress_el.adjustment().set_upper(song.duration().max(1) as f64);
            match song.gdkPaintable() {
                Some(p) => {
                    imp.cover_el.set_paintable(Some(&p));
                    imp.cover_el.set_pixel_size(imp.cover_el.size_request().0);
                }
                None => {
                    imp.cover_el.set_icon_name(Some("music-note-symbolic"));
                    imp.cover_el.set_pixel_size(-1);
                }
            }
        });
        if !self.forceHugeMode() {
            super::palette::watch(state);
        }
    }

    fn big_mode_toggled(&self) {
        let imp = self.imp();
        let huge = self.forceHugeMode();
        let big = huge || settings().boolean("use-big-footer");
        let size = if huge { 100 } else if big { 70 } else { 48 };
        imp.cover_el.set_size_request(size, size);
        imp.cover_el.set_pixel_size(size);
        imp.title_el.set_wrap(huge);
        imp.title_el.set_lines(if huge { 2 } else { 1 });
        imp.detail_container.set_spacing(if huge { 10 } else { 5 });
        imp.ro_progress_el.set_visible(!big);
        self.update_progress_visibility();
    }

    fn update_progress_visibility(&self) {
        let big = self.forceHugeMode() || settings().boolean("use-big-footer");
        let radio = integration::current().is_some_and(|i| {
            let id = i.current_state().songId();
            !id.is_empty() && !i.song(&id).radioStreamUrl().is_empty()
        });
        self.imp().progress_el.set_visible(!radio && big);
    }

    #[template_callback]
    fn progress_bar_changed(&self, _scroll: gtk::ScrollType, value: f64, scale: &gtk::Scale) -> bool {
        delayed_seek(scale, value);
        false
    }
}
