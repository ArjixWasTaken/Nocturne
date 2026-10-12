//! `NocturnePlayingControlPage` (playing/control_page.blp): big "now playing" controls.
use super::{delayed_seek, follow_song, on_login, watch};
use crate::{config::display_time, integration, models::Song, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PlayingControlPage)]
    #[template(resource = "/dev/arjix/Nocturne/playing/control_page.ui")]
    pub struct PlayingControlPage {
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub clamp: TemplateChild<adw::Clamp>,
        #[template_child]
        pub main_container: TemplateChild<gtk::Box>,
        #[template_child]
        pub cover_art_el: TemplateChild<super::super::PlayingCoverArt>,
        #[template_child]
        pub title_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub radio_homepage_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub artist_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub album_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub progress_el: TemplateChild<gtk::Scale>,
        #[template_child]
        pub positive_progress_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub negative_progress_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub star_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub show_sidebar_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub state_stack_el: TemplateChild<gtk::Stack>,
        #[template_child]
        pub rating_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub rating_container: TemplateChild<gtk::Box>,
        #[property(get, set, nullable, name = "extra-widget")]
        pub extra_widget: RefCell<Option<gtk::Widget>>,
        pub breakpoint_state: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlayingControlPage {
        const NAME: &'static str = "NocturnePlayingControlPage";
        type Type = super::PlayingControlPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PlayingControlPage {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.connect_notify_local(Some("extra-widget"), |o, _| {
                if let Some(w) = o.extra_widget() && w.parent().is_none() {
                    o.imp().main_container.prepend(&w);
                }
            });
            on_login(&*obj, |o, s| o.setup(s));
        }
    }
    impl WidgetImpl for PlayingControlPage {
        fn realize(&self) {
            self.parent_realize();
            self.obj().setup_breakpoint();
        }
    }
    impl NavigationPageImpl for PlayingControlPage {}
}

glib::wrapper! {
    pub struct PlayingControlPage(ObjectSubclass<imp::PlayingControlPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

fn label_of(b: &gtk::Button) -> Option<gtk::Label> {
    b.child().and_downcast()
}

#[gtk::template_callbacks]
impl PlayingControlPage {
    pub fn header_bar(&self) -> adw::HeaderBar {
        self.imp().header_bar.get()
    }
    pub fn clamp_el(&self) -> adw::Clamp {
        self.imp().clamp.get()
    }
    pub fn cover_art(&self) -> super::PlayingCoverArt {
        self.imp().cover_art_el.get()
    }

    fn setup(&self, state: &crate::models::CurrentState) {
        watch(state, self, "buttonState", |o, s| o.imp().state_stack_el.set_visible_child_name(&s.buttonState()));
        watch(state, self, "displaySongTitle", |o, s| {
            let t = s.displaySongTitle();
            o.imp().title_el.set_label(&t);
            o.imp().title_el.set_tooltip_text(Some(&t));
        });
        watch(state, self, "displaySongArtist", |o, s| {
            let a = s.displaySongArtist();
            let el = &o.imp().artist_el;
            if let Some(l) = label_of(el) {
                l.set_label(&a);
            }
            el.set_tooltip_text(Some(&a));
            el.set_visible(!a.is_empty());
        });
        watch(state, self, "positionSeconds", |o, s| {
            let Some(integ) = integration::current() else { return };
            let id = s.songId();
            if id.is_empty() {
                return;
            }
            let imp = o.imp();
            let pos = s.positionSeconds();
            let duration = integ.song(&id).duration() as f64;
            imp.positive_progress_el.set_label(&display_time(pos as i64));
            imp.negative_progress_el.set_label(&format!("-{}", display_time((duration - pos).max(0.0) as i64)));
            if !s.seeking() {
                imp.progress_el.set_value(pos);
            }
        });
        follow_song(state, self, |o, song| o.update_song(song));
    }

    fn update_song(&self, song: &Song) {
        let imp = self.imp();
        imp.star_el.set_action_target_value(Some(&song.id().to_variant()));

        let radio = song.radioStreamUrl();
        let is_radio = !radio.is_empty();
        let external = song.isExternalFile();
        imp.radio_homepage_el.set_visible(is_radio);
        imp.artist_el.set_can_target(!is_radio);
        imp.artist_el.set_focusable(!is_radio);
        for w in [imp.positive_progress_el.upcast_ref::<gtk::Widget>(), imp.negative_progress_el.upcast_ref(), imp.progress_el.upcast_ref()] {
            w.set_visible(!is_radio);
        }
        imp.rating_button.set_visible(!is_radio && !external);
        imp.star_el.set_visible(!is_radio && !external);
        let parsed = url::Url::parse(&radio).ok();
        let host = parsed.as_ref().and_then(|u| u.host_str()).unwrap_or_default().to_string();
        let homepage = parsed.as_ref().map(|u| format!("{}://{}", u.scheme(), host)).unwrap_or_default();
        imp.radio_homepage_el.set_tooltip_text(Some(&homepage));
        imp.radio_homepage_el.set_action_target_value(Some(&homepage.to_variant()));
        if let Some(c) = imp.radio_homepage_el.child().and_downcast::<adw::ButtonContent>() {
            c.set_label(&host);
        }

        let artist_id = song.artistId();
        if !artist_id.is_empty() {
            imp.artist_el.set_action_target_value(Some(&artist_id.to_variant()));
            imp.artist_el.set_action_name(Some("app.show_artist"));
        }

        let album = song.album();
        if let Some(l) = label_of(&imp.album_el) {
            l.set_label(&album);
        }
        imp.album_el.set_tooltip_text(Some(&album));
        imp.album_el.set_visible(!album.is_empty());
        let album_id = song.albumId();
        if album_id.is_empty() {
            imp.album_el.set_action_name(None);
            imp.album_el.set_sensitive(false);
        } else {
            imp.album_el.set_action_target_value(Some(&album_id.to_variant()));
            imp.album_el.set_action_name(Some("app.show_album"));
            imp.album_el.set_sensitive(true);
        }
        if let Some(wrap) = imp.album_el.parent() {
            wrap.set_sensitive(!external);
        }

        imp.progress_el.adjustment().set_upper(song.duration().max(1) as f64);

        let rating = song.userRating();
        let mut child = imp.rating_container.first_child();
        let mut i = 1;
        while let Some(b) = child.and_downcast::<gtk::Button>() {
            b.set_icon_name(if rating >= i { "starred-symbolic" } else { "non-starred-symbolic" });
            child = b.next_sibling();
            i += 1;
        }
        imp.rating_button.set_icon_name(if rating > 0 { "starred-symbolic" } else { "non-starred-symbolic" });

        let star = &imp.star_el;
        if song.starred() {
            star.add_css_class("accent");
            star.remove_css_class("dim-label");
            star.set_icon_name("heart-filled-symbolic");
            star.set_tooltip_text(Some("Favorite"));
        } else {
            star.remove_css_class("accent");
            star.add_css_class("dim-label");
            star.set_icon_name("heart-outline-thick-symbolic");
            star.set_tooltip_text(Some("Not Favorite"));
        }
    }

    fn setup_breakpoint(&self) {
        let Some(win) = self.root().and_downcast::<adw::ApplicationWindow>() else { return };
        let update = glib::clone!(#[weak(rename_to = o)] self, move |w: &adw::ApplicationWindow| o.breakpoint_toggled(w.width() > 0 && w.width() <= 620));
        update(&win);
        win.connect_notify_local(Some("current-breakpoint"), move |w, _| update(w));
    }

    fn breakpoint_toggled(&self, active: bool) {
        self.imp().breakpoint_state.set(active);
        if let Some(nav) = self.parent().and_downcast::<adw::NavigationView>() {
            if nav.find_property("vhomogeneous").is_some() {
                nav.set_property("vhomogeneous", true);
            }
        }
    }

    #[template_callback]
    fn progress_bar_changed(&self, _scroll: gtk::ScrollType, value: f64, scale: &gtk::Scale) -> bool {
        delayed_seek(scale, value);
        false
    }

    #[template_callback]
    fn show_content_clicked(&self, _: &gtk::Button) {
        if let Some(view) = self.ancestor(adw::NavigationSplitView::static_type()).and_downcast::<adw::NavigationSplitView>() {
            if self.imp().breakpoint_state.get() {
                view.set_show_content(true);
            } else {
                view.set_collapsed(!view.is_collapsed());
            }
        }
    }

    #[template_callback]
    fn change_rating(&self, button: &gtk::Button) {
        let Some(integ) = integration::current() else { return };
        let id = integ.current_state().songId();
        let rating: i32 = button.widget_name().parse().unwrap_or(0);
        if id.is_empty() {
            return;
        }
        spawn_local(async move {
            integ.set_rating(&id, rating).await;
        });
    }
}
