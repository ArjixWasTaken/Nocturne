//! `NocturneApplication`: owns the integration, player and windows.
use crate::{
    backend::Connection,
    config::{APP_ID, settings},
    integration::{self, Integration},
    player::Player,
    runtime::spawn_local,
    window::Window,
};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{gdk, gio, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::Application)]
    pub struct Application {
        #[property(get, set, nullable)]
        pub integration: RefCell<Option<Integration>>,
        pub player: RefCell<Option<Player>>,
        pub window: RefCell<Option<Window>>,
        /// Dynamic accent/background CSS generated from the cover art.
        pub css_provider: std::cell::OnceCell<gtk::CssProvider>,
        /// Files passed on the command line before login finished.
        pub pending_files: RefCell<Vec<gio::File>>,
        pub popout: RefCell<Option<crate::widgets::playing::PopoutWindow>>,
        pub inhibit_cookie: std::cell::Cell<Option<u32>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "NocturneApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn startup(&self) {
            self.parent_startup();
            let obj = self.obj();
            crate::widgets::ensure_types();
            if let Some(display) = gdk::Display::default() {
                gtk::style_context_add_provider_for_display(&display, self.css_provider.get_or_init(gtk::CssProvider::new), gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
                gtk::IconTheme::for_display(&display).add_resource_path("/dev/arjix/Nocturne/icons");
            }
            crate::actions::register(&obj);
        }

        fn activate(&self) {
            let obj = self.obj();
            let first = self.window.borrow().is_none();
            let window = obj.main_window();
            window.present();
            if first {
                obj.load_default_integration();
            }
        }

        fn open(&self, files: &[gio::File], _hint: &str) {
            self.pending_files.borrow_mut().extend(files.iter().cloned());
            let obj = self.obj();
            if integration::current().is_some() {
                obj.play_pending_files();
            }
            self.activate();
        }

        fn shutdown(&self) {
            if let Some(w) = self.window.borrow().as_ref() {
                let _ = settings().set_int("default-width", w.default_width());
                let _ = settings().set_int("default-height", w.default_height());
            }
            self.parent_shutdown();
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends adw::Application, gtk::Application, gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for Application {
    fn default() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("flags", gio::ApplicationFlags::HANDLES_OPEN)
            .property("resource-base-path", "/dev/arjix/Nocturne")
            .build()
    }
}

impl Application {
    pub fn get() -> Self {
        gio::Application::default().and_downcast().expect("no NocturneApplication")
    }

    pub fn main_window(&self) -> Window {
        self.imp().window.borrow_mut().get_or_insert_with(|| Window::new(self)).clone()
    }

    pub fn player(&self) -> Option<Player> {
        self.imp().player.borrow().clone()
    }

    pub fn popout_window(&self) -> Option<crate::widgets::playing::PopoutWindow> {
        self.imp().popout.borrow().clone()
    }

    pub fn set_popout_window(&self, w: Option<crate::widgets::playing::PopoutWindow>) {
        self.imp().popout.replace(w);
    }

    /// Inhibit (or allow) system suspend while music is playing.
    pub fn inhibit_suspend(&self, on: bool) {
        let imp = self.imp();
        match (on, imp.inhibit_cookie.get()) {
            (true, None) => {
                let c = self.inhibit(self.active_window().as_ref(), gtk::ApplicationInhibitFlags::SUSPEND, Some("Music is Playing"));
                imp.inhibit_cookie.set(Some(c));
            }
            (false, Some(c)) => {
                self.uninhibit(c);
                imp.inhibit_cookie.set(None);
            }
            _ => {}
        }
    }

    pub fn css_provider(&self) -> gtk::CssProvider {
        self.imp().css_provider.get_or_init(gtk::CssProvider::new).clone()
    }

    /// Log back into the integration stored in settings, or show the welcome page.
    fn load_default_integration(&self) {
        let s = settings();
        let kind = s.string("selected-instance-type");
        if kind.is_empty() {
            self.main_window().show_welcome();
            return;
        }
        let app = self.clone();
        spawn_local(async move {
            let conn = Connection {
                url: s.string("integration-ip").into(),
                user: s.string("integration-user").into(),
                password: crate::secret::lookup("password").await.ok().flatten().unwrap_or_default(),
                trust_server: s.boolean("integration-trust-server"),
                library_dir: s.string("integration-library-dir").into(),
                library_ids: s.strv("library-ids").iter().map(|g| g.to_string()).collect(),
            };
            if let Err(e) = app.login(&kind, conn, false).await {
                tracing::warn!("auto login failed: {e:#}");
                app.main_window().show_welcome();
                app.main_window().toast_error(&gettext_login_failed(), &format!("{e:#}"));
            }
        });
    }

    /// Connect, ping and switch the UI to the library. With `remember`,
    /// persist settings + password for the next start.
    pub async fn login(&self, kind: &str, conn: Connection, remember: bool) -> anyhow::Result<()> {
        let integ = Integration::connect(kind, conn.clone()).await?;
        integ.call(|b| async move { b.ping().await }).await?;

        if remember {
            let s = settings();
            let _ = s.set_string("selected-instance-type", kind);
            let _ = s.set_string("integration-ip", &conn.url);
            let _ = s.set_string("integration-user", &conn.user);
            let _ = s.set_string("integration-library-dir", &conn.library_dir);
            let _ = s.set_boolean("integration-trust-server", conn.trust_server);
            if !conn.password.is_empty() && let Err(e) = crate::secret::store("password", &conn.password).await {
                tracing::warn!("could not store password: {e:#}");
            }
        }

        if let Some(old) = self.player() {
            old.stop();
        }
        integration::set_current(Some(integ.clone()));
        self.set_integration(Some(integ.clone()));

        let player = Player::new(integ.current_state());
        let app = self.clone();
        crate::mpris::start(player.clone(), move || app.main_window().present());
        self.imp().player.replace(Some(player.clone()));

        let window = self.main_window();
        window.setup();

        if !self.imp().pending_files.borrow().is_empty() {
            self.play_pending_files();
        } else if settings().boolean("restore-session") {
            let q = integ.call(|b| async move { b.play_queue().await }).await.unwrap_or_default();
            for s in &q.songs {
                integ.ingest_song(s);
            }
            if !q.ids.is_empty() {
                player.pause_next_change();
                crate::queue::replace(&q.ids, q.current.as_deref(), "");
            }
        }
        Ok(())
    }

    /// Forget the session and go back to the welcome page.
    pub async fn logout(&self) {
        if let Some(p) = self.imp().player.take() {
            p.stop();
        }
        integration::set_current(None);
        self.set_integration(None::<Integration>);
        let s = settings();
        let _ = s.set_string("selected-instance-type", "");
        let _ = crate::secret::delete("password").await;
        self.main_window().show_welcome();
    }

    /// Save the play queue (called before quitting).
    pub async fn save_session(&self) {
        let Some(integ) = integration::current() else { return };
        crate::queue::save_playlist_resume();
        let state = integ.current_state();
        let (ids, cur, pos) = (state.queue_ids(), state.songId(), (state.positionSeconds() * 1000.0) as i64);
        let ids: Vec<String> = ids.into_iter().filter(|i| !integ.song(i).isExternalFile()).collect();
        let _ = integ.call(move |b| async move { b.save_play_queue(&ids, &cur, pos).await }).await;
    }

    fn play_pending_files(&self) {
        let files: Vec<gio::File> = self.imp().pending_files.take();
        let Some(integ) = integration::current() else { return };
        let ids: Vec<String> = files
            .iter()
            .filter_map(|f| f.path())
            .filter_map(|p| crate::backend::local::song_from_file(&p))
            .map(|mut d| {
                d.id = format!("EXTERNAL_SONG:{}", d.path);
                let song = integ.ingest_song(&d);
                song.set_isExternalFile(true);
                song.set_verified(true);
                song.id()
            })
            .collect();
        if !ids.is_empty() {
            crate::queue::replace(&ids, None, "");
        }
    }
}

fn gettext_login_failed() -> String {
    "Login Failed".into()
}
