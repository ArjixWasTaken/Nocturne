//! `NocturneWindow` (window.blp): sidebar, page navigation, bottom sheet player and toasts.
use crate::{
    application::Application,
    config::settings,
    integration,
    models::CurrentState,
    runtime::spawn_local,
    widgets::pages::{self, PlaybackPage},
};
use adw::{prelude::*, subclass::prelude::*};
use chrono::Datelike;
use gtk::{CompositeTemplate, gio, glib};
use std::cell::{Cell, RefCell};

pub struct SidebarEntry {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub tag: &'static str,
}

pub struct SidebarSection {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub tag: &'static str,
    pub items: &'static [SidebarEntry],
}

const fn e(id: &'static str, title: &'static str, icon: &'static str, tag: &'static str) -> SidebarEntry {
    SidebarEntry { id, title, icon, tag }
}

/// Sidebar layout (`SIDEBAR_MENU` in the Python constants). Ids are stored in
/// `sidebar-disabled-sections` / `sidebar-disabled-items`.
pub const SIDEBAR_MENU: &[SidebarSection] = &[
    SidebarSection { id: "root", title: "", icon: "", tag: "", items: &[e("home", "Home", "user-home-symbolic", "home")] },
    SidebarSection {
        id: "albums",
        title: "Albums",
        icon: "music-queue-symbolic",
        tag: "albums-all",
        items: &[
            e("albums-all", "All", "music-queue-symbolic", "albums-all"),
            e("albums-random", "Random", "playlist-shuffle-symbolic", "albums-random"),
            e("albums-starred", "Favorites", "heart-filled-symbolic", "albums-starred"),
            e("albums-newest", "Recently Added", "list-add-symbolic", "albums-newest"),
            e("albums-recent", "Recently Played", "media-playback-start-symbolic", "albums-recent"),
            e("albums-frequent", "Most Played", "media-playlist-repeat-symbolic", "albums-frequent"),
        ],
    },
    SidebarSection {
        id: "artists",
        title: "Artists",
        icon: "music-artist-symbolic",
        tag: "artists",
        items: &[
            e("artists-all", "All", "music-artist-symbolic", "artists"),
            e("artists-starred", "Favorites", "heart-filled-symbolic", "artists-starred"),
        ],
    },
    SidebarSection {
        id: "songs",
        title: "Songs",
        icon: "music-note-symbolic",
        tag: "songs-all",
        items: &[
            e("songs-all", "All", "music-note-symbolic", "songs-all"),
            e("songs-starred", "Favorites", "heart-filled-symbolic", "songs-starred"),
            e("radios", "Radios", "sound-wave-symbolic", "radios"),
        ],
    },
    SidebarSection {
        id: "playlists",
        title: "Playlists",
        icon: "playlist-symbolic",
        tag: "playlists",
        items: &[
            e("playlists-all", "All", "playlist-symbolic", "playlists"),
            // Special: the first playlists of the library.
            e("playlists-group", "Playlists", "playlist-symbolic", ""),
        ],
    },
];

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/window.ui")]
    pub struct Window {
        #[template_child]
        pub loading_label_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub breakpoint_el: TemplateChild<adw::Breakpoint>,
        #[template_child]
        pub main_navigationview: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub main_bottom_sheet: TemplateChild<gtk::Widget>,
        #[template_child]
        pub main_split_view: TemplateChild<adw::NavigationSplitView>,
        #[template_child]
        pub player_sidebar_splitview: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub sidebar_bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        /// The `Adw.Sidebar` (libadwaita 1.9) currently in `sidebar_bin`.
        pub sidebar: RefCell<Option<gtk::Widget>>,
        /// `(page tag, playlist id)` of every sidebar item, in activation-index order.
        pub sidebar_entries: RefCell<Vec<(String, Option<String>)>>,
        pub sidebar_playlists: RefCell<Vec<String>>,
        pub narrow: Cell<bool>,
        pub state_handler: RefCell<Option<(CurrentState, glib::SignalHandlerId)>>,
        pub loading_handler: RefCell<Option<(integration::Integration, glib::SignalHandlerId)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "NocturneWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let s = settings();
            obj.set_default_size(s.int("default-width"), s.int("default-height"));
            s.bind("hide-on-close", &*obj, "hide-on-close").build();

            for (key, class) in [("use-dynamic-accent", "dynamic-accent"), ("player-blur-bg", "player-translucent")] {
                let w = obj.downgrade();
                let apply = move |s: &gio::Settings, key: &str| {
                    if let Some(w) = w.upgrade() {
                        if s.boolean(key) { w.add_css_class(class) } else { w.remove_css_class(class) }
                    }
                };
                apply(&s, key);
                s.connect_changed(Some(key), apply);
            }
            for (key, prefix) in [("global-dynamic-bg-mode", "global-"), ("player-dynamic-bg-mode", "")] {
                let w = obj.downgrade();
                let apply = move |s: &gio::Settings, key: &str| {
                    let Some(w) = w.upgrade() else { return };
                    w.remove_css_class(&format!("{prefix}dynamic-bg-gradient"));
                    w.remove_css_class(&format!("{prefix}dynamic-bg-blur"));
                    let v = s.string(key);
                    if !v.is_empty() {
                        w.add_css_class(&format!("{prefix}dynamic-bg-{v}"));
                    }
                };
                apply(&s, key);
                s.connect_changed(Some(key), apply);
            }
            let w = obj.downgrade();
            s.connect_changed(Some("use-sidebar-player"), move |_, _| {
                if let Some(w) = w.upgrade() {
                    w.big_breakpoint_toggled();
                }
            });
            let w = obj.downgrade();
            s.connect_changed(Some("show-playlists-in-sidebar"), move |_, _| {
                if let Some(w) = w.upgrade() {
                    w.setup_sidebar();
                }
            });

            for apply in [true, false] {
                let w = obj.downgrade();
                let f = move |_: &adw::Breakpoint| {
                    if let Some(w) = w.upgrade() {
                        w.imp().narrow.set(apply);
                        w.update_sidebar_mode();
                    }
                };
                if apply { self.breakpoint_el.connect_apply(f) } else { self.breakpoint_el.connect_unapply(f) };
            }
        }
    }
    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

/// Last month's `(song, plays)` (verified songs only) and its display name ("March 2026").
pub async fn last_month_playback() -> (Vec<(String, i64)>, String, String) {
    let first = chrono::Local::now().date_naive().with_day(1).expect("day 1");
    let prev = first.pred_opt().expect("previous day");
    let (key, label) = (prev.format("%m-%Y").to_string(), prev.format("%B %Y").to_string());
    let Some(integ) = integration::current() else { return (vec![], key, label) };
    let db = integ.db();
    let k = key.clone();
    let plays = crate::runtime::run(async move { crate::db::monthly_plays(&db, &k, 50).await }).await;
    let mut top = vec![];
    for (id, n) in plays {
        if integ.verify_song(&id, false).await.verified() {
            top.push((id, n));
        }
    }
    (top, key, label)
}

#[gtk::template_callbacks]
impl Window {
    pub fn new(app: &Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    /// Show the welcome (login method) page.
    pub fn show_welcome(&self) {
        self.imp().main_stack.set_visible_child_name("welcome");
    }

    /// Show the library (the "content" child of the main stack).
    pub fn show_content(&self) {
        self.imp().main_stack.set_visible_child_name("content");
    }

    /// Show the loading spinner with an optional message.
    pub fn show_loading(&self, message: &str) {
        let imp = self.imp();
        imp.loading_label_el.set_label(message);
        imp.main_stack.set_visible_child_name("loading");
    }

    pub fn main_stack(&self) -> gtk::Stack {
        self.imp().main_stack.get()
    }

    pub fn navigation_view(&self) -> adw::NavigationView {
        self.imp().main_navigationview.get()
    }

    pub fn playback_page(&self) -> PlaybackPage {
        self.imp().main_stack.child_by_name("playback").and_downcast().expect("playback page")
    }

    /// Called after a successful login: build sidebar, show content, open default page.
    pub fn setup(&self) {
        let imp = self.imp();
        let Some(integ) = integration::current() else { return };
        imp.sidebar_playlists.borrow_mut().clear();
        self.setup_sidebar();
        self.show_content();
        if let Some(d) = self.visible_dialog() {
            d.close();
        }
        let tag = settings().string("default-page-tag");
        let tag = if tag.is_empty() { "home".to_string() } else { tag.to_string() };
        self.select_sidebar_tag(&tag);
        self.replace_root_page(&tag);

        let state = integ.current_state();
        let w = self.downgrade();
        let id = state.connect_notify_local(Some("songId"), move |s, _| {
            if let Some(w) = w.upgrade() {
                w.song_changed(&s.songId());
            }
        });
        if let Some((old, id)) = imp.state_handler.replace(Some((state.clone(), id))) {
            old.disconnect(id);
        }
        let w = self.downgrade();
        let id = integ.connect_notify_local(Some("loadingMessage"), move |i, _| {
            if let Some(w) = w.upgrade() {
                w.update_loading_message(&i.loadingMessage());
            }
        });
        if let Some((old, id)) = imp.loading_handler.replace(Some((integ.clone(), id))) {
            old.disconnect(id);
        }
        self.song_changed(&state.songId());

        self.setup_sidebar_playlists();
        let w = self.downgrade();
        spawn_local(async move {
            if let Some(w) = w.upgrade() {
                w.notify_playback().await;
            }
        });
    }

    /// Fetch the first playlists again and rebuild the sidebar.
    pub fn setup_sidebar_playlists(&self) {
        let Some(integ) = integration::current() else { return };
        let w = self.downgrade();
        spawn_local(async move {
            let mut ids = integ.playlists().await;
            ids.truncate(4);
            let Some(w) = w.upgrade() else { return };
            w.imp().sidebar_playlists.replace(ids);
            w.setup_sidebar();
        });
    }

    /// Plain toast.
    pub fn toast(&self, title: &str) {
        self.imp().toast_overlay.add_toast(adw::Toast::builder().title(title).timeout(2).build());
    }

    /// Add an arbitrary toast.
    pub fn add_toast(&self, toast: adw::Toast) {
        self.imp().toast_overlay.add_toast(toast);
    }

    /// Toast with a "Show Details" button that opens `details`.
    pub fn toast_error(&self, title: &str, details: &str) {
        let toast = adw::Toast::builder()
            .title(title)
            .button_label("Show Details")
            .action_name("app.show_error")
            .action_target(&details.to_variant())
            .build();
        self.add_toast(toast);
    }

    /// Push `page` onto the main navigation view.
    pub fn show_page(&self, page: &impl IsA<adw::NavigationPage>) {
        let imp = self.imp();
        imp.main_bottom_sheet.set_property("open", false);
        imp.main_split_view.set_show_content(true);
        imp.main_navigationview.push(page);
        if let Some(d) = self.visible_dialog() {
            d.close();
        }
    }

    /// Root pages declared in window.blp.
    pub fn root_pages(&self) -> Vec<adw::NavigationPage> {
        let mut v = vec![];
        let mut c = self.imp().main_navigationview.first_child();
        while let Some(w) = c {
            c = w.next_sibling();
            if let Ok(p) = w.downcast::<adw::NavigationPage>() {
                v.push(p);
            }
        }
        v
    }

    /// Replace the navigation stack with the root page tagged `tag`.
    pub fn replace_root_page(&self, tag: &str) {
        let imp = self.imp();
        if let Some(page) = imp.main_navigationview.find_page(tag) {
            imp.main_bottom_sheet.set_property("open", false);
            imp.main_split_view.set_show_content(true);
            pages::load(&page);
            imp.main_navigationview.replace(&[page]);
        }
    }

    /// Highlight the sidebar item that opens `tag`.
    pub fn select_sidebar_tag(&self, tag: &str) {
        let imp = self.imp();
        let idx = imp.sidebar_entries.borrow().iter().position(|(t, p)| t == tag && p.is_none());
        if let (Some(i), Some(sb)) = (idx, imp.sidebar.borrow().as_ref()) {
            sb.set_property("selected", i as u32);
        }
    }

    /// Reload the visible page.
    pub fn reload_visible_page(&self) {
        if let Some(p) = self.imp().main_navigationview.visible_page() {
            pages::reload(&p);
        }
    }

    /// (Re)build the sidebar from [`SIDEBAR_MENU`] and the settings.
    pub fn setup_sidebar(&self) {
        let imp = self.imp();
        let s = settings();
        let disabled_sections: Vec<String> = s.strv("sidebar-disabled-sections").iter().map(|g| g.to_string()).collect();
        let disabled_items: Vec<String> = s.strv("sidebar-disabled-items").iter().map(|g| g.to_string()).collect();
        let show_playlists = s.boolean("show-playlists-in-sidebar");
        let integ = integration::current();

        // (title, items: (title, icon, tag, playlist))
        type Item = (String, String, String, Option<String>);
        let mut sections: Vec<(String, Vec<Item>)> = vec![(String::new(), vec![])];
        for sec in SIDEBAR_MENU {
            if sec.id == "root" {
                sections[0].1.extend(sec.items.iter().map(|i| (i.title.into(), i.icon.into(), i.tag.into(), None)));
            } else if disabled_sections.iter().any(|d| d == sec.id) {
                sections[0].1.push((sec.title.into(), sec.icon.into(), sec.tag.into(), None));
            } else {
                let mut items = vec![];
                for i in sec.items.iter().filter(|i| !disabled_items.iter().any(|d| d == i.id)) {
                    if i.id == "playlists-group" {
                        if show_playlists {
                            for p in imp.sidebar_playlists.borrow().iter() {
                                items.push((String::new(), String::new(), "playlists".into(), Some(p.clone())));
                            }
                        }
                    } else {
                        items.push((i.title.into(), i.icon.into(), i.tag.into(), None));
                    }
                }
                if !items.is_empty() {
                    sections.push((sec.title.into(), items));
                }
            }
        }

        let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?><interface><object class="AdwSidebar" id="sidebar">"#);
        let mut n = 0;
        for (si, (_, items)) in sections.iter().enumerate() {
            xml.push_str(&format!(r#"<child><object class="AdwSidebarSection" id="s{si}">"#));
            for _ in items {
                xml.push_str(&format!(r#"<child><object class="AdwSidebarItem" id="i{n}"/></child>"#));
                n += 1;
            }
            xml.push_str("</object></child>");
        }
        xml.push_str("</object></interface>");
        let builder = gtk::Builder::from_string(&xml);

        let mut entries = vec![];
        let mut n = 0;
        for (si, (title, items)) in sections.into_iter().enumerate() {
            if !title.is_empty()
                && let Some(sec) = builder.object::<glib::Object>(&format!("s{si}"))
            {
                sec.set_property("title", &title);
            }
            for (title, icon, tag, playlist) in items {
                let Some(item) = builder.object::<glib::Object>(&format!("i{n}")) else { continue };
                n += 1;
                if let (Some(pid), Some(integ)) = (&playlist, &integ) {
                    let p = integ.playlist(pid);
                    p.bind_property("name", &item, "title").sync_create().build();
                    p.bind_property("name", &item, "tooltip").sync_create().build();
                    p.bind_property("songCount", &item, "subtitle")
                        .transform_to(|_, n: i32| Some(if n == 1 { "1 Song".to_string() } else { format!("{n} Songs") }))
                        .sync_create()
                        .build();
                } else {
                    item.set_property("title", &title);
                    item.set_property("tooltip", &title);
                    item.set_property("icon-name", &icon);
                }
                entries.push((tag, playlist));
            }
        }
        imp.sidebar_entries.replace(entries);

        let Some(sidebar) = builder.object::<gtk::Widget>("sidebar") else { return };
        let old_selected = imp.sidebar.borrow().as_ref().map(|o| o.property::<u32>("selected"));
        let w = self.downgrade();
        sidebar.connect_closure(
            "activated",
            false,
            glib::closure_local!(move |_: glib::Object, index: u32| {
                if let Some(w) = w.upgrade() {
                    w.on_sidebar_activated(index);
                }
            }),
        );
        imp.sidebar_bin.set_child(Some(&sidebar));
        imp.sidebar.replace(Some(sidebar.clone()));
        if let Some(sel) = old_selected
            && (sel as usize) < imp.sidebar_entries.borrow().len()
        {
            sidebar.set_property("selected", sel);
        }
        self.update_sidebar_mode();
    }

    fn update_sidebar_mode(&self) {
        let imp = self.imp();
        if let Some(sb) = imp.sidebar.borrow().as_ref() {
            pages::set_enum_property(sb, "mode", if imp.narrow.get() { "page" } else { "sidebar" });
        }
    }

    fn on_sidebar_activated(&self, index: u32) {
        let entry = self.imp().sidebar_entries.borrow().get(index as usize).cloned();
        match entry {
            Some((_, Some(playlist))) => {
                self.replace_root_page("playlists");
                self.show_page(&crate::widgets::playlist::PlaylistPage::new(&playlist));
            }
            Some((tag, None)) => self.replace_root_page(&tag),
            None => {}
        }
    }

    fn update_loading_message(&self, message: &str) {
        let label = &self.imp().loading_label_el;
        label.set_visible(!message.is_empty());
        label.set_label(message);
        if message.is_empty() {
            self.reload_visible_page();
        }
    }

    fn song_changed(&self, song_id: &str) {
        let app = Application::get();
        let playing = !song_id.is_empty();
        app.inhibit_suspend(playing);
        if !playing && let Some(p) = app.popout_window() {
            p.close();
        }
        self.big_breakpoint_toggled();
    }

    /// Offer last month's Nocturne Playback once per month.
    async fn notify_playback(&self) {
        let (top, key, label) = last_month_playback().await;
        if settings().string("last-playback-checked") == key {
            return;
        }
        if top.len() > 5 {
            let dialog = adw::AlertDialog::new(Some("Nocturne Playback"), Some(&format!("Your Playback queue for {label} is available!")));
            dialog.add_responses(&[("show", "Show"), ("later", "Later")]);
            dialog.set_response_appearance("show", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("show"));
            if dialog.choose_future(Some(self)).await == "show" {
                let _ = gtk::prelude::WidgetExt::activate_action(self, "app.launch_playback", None);
            }
        }
        let _ = settings().set_string("last-playback-checked", &key);
    }

    #[template_callback]
    fn close_request(&self) -> bool {
        if self.hides_on_close() {
            return false;
        }
        let app = Application::get();
        if integration::current().is_some() {
            spawn_local(async move {
                app.save_session().await;
                app.quit();
            });
            return true;
        }
        app.quit();
        false
    }

    #[template_callback]
    fn on_drop(&self, value: &glib::Value) -> bool {
        match value.get::<gio::File>() {
            Ok(file) => {
                Application::get().open(&[file], "");
                true
            }
            Err(_) => false,
        }
    }

    #[template_callback]
    fn big_breakpoint_toggled(&self) {
        let imp = self.imp();
        let playing = integration::current().is_some_and(|i| !i.current_state().songId().is_empty());
        let sheet = &imp.main_bottom_sheet;
        if self.width() > 0 && self.width() <= 840 {
            imp.player_sidebar_splitview.set_show_sidebar(false);
            sheet.set_property("reveal-bottom-bar", playing);
            sheet.set_property("can-open", playing);
        } else {
            let side = settings().boolean("use-sidebar-player");
            imp.player_sidebar_splitview.set_show_sidebar(side && playing);
            sheet.set_property("reveal-bottom-bar", !side && playing);
            sheet.set_property("can-open", !side && playing);
            if side {
                sheet.set_property("open", false);
            }
        }
        if !playing {
            sheet.set_property("open", false);
        }
        if sheet.property::<bool>("reveal-bottom-bar") {
            self.add_css_class("showing-footer");
        } else {
            self.remove_css_class("showing-footer");
        }
    }
}
