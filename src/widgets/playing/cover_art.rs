//! `NocturnePlayingCoverArt` (playing/cover_art.blp): cover/video with swipe-to-skip.
use super::{on_login, Spectrum};
use crate::{application::Application, integration};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib};
use std::cell::{Cell, OnceCell, RefCell};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PlayingCoverArt)]
    #[template(resource = "/dev/arjix/Nocturne/playing/cover_art.ui")]
    pub struct PlayingCoverArt {
        #[template_child]
        pub spectrum_el: TemplateChild<Spectrum>,
        #[template_child]
        pub view_stack_el: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub cover_el: TemplateChild<gtk::Picture>,
        #[template_child]
        pub video_el: TemplateChild<gtk::Picture>,
        #[template_child]
        pub view_switcher_el: TemplateChild<adw::ViewSwitcher>,
        #[template_child]
        pub previous_icon_el: TemplateChild<gtk::Image>,
        #[template_child]
        pub next_icon_el: TemplateChild<gtk::Image>,
        #[property(get, set, name = "swipeProgress")]
        pub swipe_progress: Cell<f64>,
        pub animation: OnceCell<adw::TimedAnimation>,
        pub tracker: OnceCell<adw::SwipeTracker>,
        pub song_handler: RefCell<Option<(crate::models::Song, glib::SignalHandlerId)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlayingCoverArt {
        const NAME: &'static str = "NocturnePlayingCoverArt";
        type Type = super::PlayingCoverArt;
        type ParentType = gtk::Box;
        type Interfaces = (adw::Swipeable,);

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PlayingCoverArt {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let target = adw::CallbackAnimationTarget::new(glib::clone!(#[weak] obj, move |v| obj.set_swipeProgress(v)));
            let _ = self.animation.set(adw::TimedAnimation::new(&*obj, 0.0, 0.0, 200, target));
            let tracker = adw::SwipeTracker::new(&*obj);
            tracker.set_enabled(true);
            tracker.set_allow_mouse_drag(true);
            tracker.connect_update_swipe(glib::clone!(#[weak] obj, move |_, p| {
                obj.set_swipeProgress(p);
                obj.queue_allocate();
            }));
            tracker.connect_end_swipe(glib::clone!(#[weak] obj, move |_, _, _| obj.on_swipe_end()));
            let _ = self.tracker.set(tracker);
            obj.connect_notify_local(Some("swipeProgress"), |o, _| o.swipe_progress_changed());
            obj.update_cover(None);
            on_login(&*obj, |o, s| o.setup(s));
        }
    }
    impl WidgetImpl for PlayingCoverArt {}
    impl BoxImpl for PlayingCoverArt {}

    impl SwipeableImpl for PlayingCoverArt {
        fn distance(&self) -> f64 {
            self.obj().width() as f64 / 1.5
        }
        fn progress(&self) -> f64 {
            self.swipe_progress.get()
        }
        fn cancel_progress(&self) -> f64 {
            0.0
        }
        fn snap_points(&self) -> Vec<f64> {
            vec![-1.0, -0.9, 0.9, 1.0]
        }
        fn swipe_area(&self, _: adw::NavigationDirection, _: bool) -> gdk::Rectangle {
            let o = self.obj();
            gdk::Rectangle::new(0, 0, o.width(), o.height())
        }
    }
}

glib::wrapper! {
    pub struct PlayingCoverArt(ObjectSubclass<imp::PlayingCoverArt>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable, adw::Swipeable;
}

#[gtk::template_callbacks]
impl PlayingCoverArt {
    pub fn view_stack(&self) -> adw::ViewStack {
        self.imp().view_stack_el.get()
    }

    fn setup(&self, state: &crate::models::CurrentState) {
        let imp = self.imp();
        if let Some(p) = Application::get().player()
            && let Some(sink) = p.playbin().property::<Option<gst::Element>>("video-sink")
            && sink.has_property("paintable")
        {
            imp.video_el.set_paintable(sink.property::<Option<gdk::Paintable>>("paintable").as_ref());
        }
        super::watch(state, self, "songId", |o, s| {
            let imp = o.imp();
            if let Some((old, id)) = imp.song_handler.take() {
                old.disconnect(id);
            }
            let Some(integ) = integration::current() else { return };
            let song = integ.song(&s.songId());
            let id = song.connect_notify_local(Some("gdkPaintableBig"), glib::clone!(#[weak] o, move |song, _| o.update_cover(song.gdkPaintableBig())));
            o.update_cover(song.gdkPaintableBig());
            imp.song_handler.replace(Some((song, id)));
        });
    }

    fn update_cover(&self, paintable: Option<gdk::Paintable>) {
        let cover = &self.imp().cover_el;
        match paintable {
            Some(p) => {
                cover.remove_css_class("p50");
                cover.set_paintable(Some(&p));
            }
            None => {
                let icon = gtk::IconTheme::for_display(&self.display()).lookup_icon("music-note-symbolic", &[], 64, 1, gtk::TextDirection::None, gtk::IconLookupFlags::empty());
                cover.add_css_class("p50");
                cover.set_paintable(Some(&icon));
            }
        }
    }

    fn on_swipe_end(&self) {
        let progress = self.swipeProgress();
        if progress != 0.0 && let Some(a) = self.imp().animation.get() {
            a.set_value_from(progress);
            a.play();
        }
        let action = if progress > 0.9 { "player_next" } else if progress < -0.9 { "player_previous" } else { return };
        glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            Application::get().activate_action(action, None);
        });
    }

    fn swipe_progress_changed(&self) {
        let imp = self.imp();
        let p = self.swipeProgress();
        let style = |icon: &gtk::Image, active: bool| {
            icon.set_css_classes(if active { &["accent"] } else { &["dimmed"] });
        };
        let (next, prev) = (&imp.next_icon_el, &imp.previous_icon_el);
        let n = p.max(0.0);
        next.set_margin_start((n * 10.0) as i32);
        next.set_pixel_size(((n * 32.0) as i32).max(1));
        next.set_visible(p > 0.0);
        style(next, p > 0.9);
        let v = (-p).max(0.0);
        prev.set_margin_end((v * 10.0) as i32);
        prev.set_pixel_size(((v * 32.0) as i32).max(1));
        prev.set_visible(p < 0.0);
        style(prev, p < -0.9);
    }

    #[template_callback]
    fn format_video_available(&self, paintable: Option<gdk::Paintable>, video_id: Option<String>, song_id: Option<String>) -> bool {
        let v = video_id.unwrap_or_default();
        paintable.is_some() && !v.is_empty() && Some(v) == song_id
    }

    #[template_callback]
    fn format_view_stack_visible_child_name(&self, paintable: Option<gdk::Paintable>, video_id: Option<String>, song_id: Option<String>) -> String {
        if self.format_video_available(paintable, video_id, song_id) { "video" } else { "audio" }.into()
    }
}
