//! `NocturnePreferencesDialog` (preferences.blp): mostly plain settings bindings.
use crate::{application::Application, config::{data_dir, settings}, integration, runtime::{run, spawn_local}, window::SIDEBAR_MENU};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, gio, glib};
use std::cell::RefCell;

const BITRATES: [(&str, i32); 5] = [("Low (64 kbps)", 64), ("Medium (128 kbps)", 128), ("High (192 kbps)", 192), ("Ultra (320 kbps)", 320), ("Original File", 0)];

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/preferences.ui")]
    pub struct PreferencesDialog {
        #[template_child] pub restore_el: TemplateChild<gtk::Widget>,
        #[template_child] pub hide_on_close_el: TemplateChild<gtk::Widget>,
        #[template_child] pub simulate_wbwl_el: TemplateChild<gtk::Widget>,
        #[template_child] pub auto_download_lyrics_el: TemplateChild<gtk::Widget>,
        #[template_child] pub use_gain_el: TemplateChild<gtk::Widget>,
        #[template_child] pub default_page_el: TemplateChild<adw::ComboRow>,
        #[template_child] pub bitrate_el: TemplateChild<adw::ComboRow>,
        #[template_child] pub gnome_search_group_el: TemplateChild<gtk::Widget>,
        #[template_child] pub session_group_el: TemplateChild<gtk::Widget>,
        #[template_child] pub discord_rpc_el: TemplateChild<gtk::Widget>,
        #[template_child] pub discord_coverart_share_el: TemplateChild<gtk::Widget>,
        #[template_child] pub listenbrainz_stack_el: TemplateChild<gtk::Stack>,
        #[template_child] pub instance_el: TemplateChild<adw::ActionRow>,
        #[template_child] pub instance_avatar_el: TemplateChild<adw::Avatar>,
        #[template_child] pub instance_icon_el: TemplateChild<gtk::Image>,
        #[template_child] pub library_combo_el: TemplateChild<adw::ComboRow>,
        #[template_child] pub library_expander_el: TemplateChild<adw::ExpanderRow>,
        #[template_child] pub context_button_el: TemplateChild<gtk::Widget>,
        #[template_child] pub context_label_el: TemplateChild<gtk::Widget>,
        #[template_child] pub footer_big_mode_el: TemplateChild<gtk::Widget>,
        #[template_child] pub translucent_player_el: TemplateChild<gtk::Widget>,
        #[template_child] pub use_sidebar_player_el: TemplateChild<gtk::Widget>,
        #[template_child] pub hide_singles_el: TemplateChild<gtk::Widget>,
        #[template_child] pub button_size_el: TemplateChild<gtk::Widget>,
        #[template_child] pub global_dynamic_bg_el: TemplateChild<gtk::Widget>,
        #[template_child] pub player_dynamic_bg_el: TemplateChild<gtk::Widget>,
        #[template_child] pub popout_dynamic_bg_el: TemplateChild<gtk::Widget>,
        #[template_child] pub dynamic_accent_el: TemplateChild<gtk::Widget>,
        #[template_child] pub home_mode_el: TemplateChild<gtk::Widget>,
        #[template_child] pub hp_frequent_albums_el: TemplateChild<gtk::Widget>,
        #[template_child] pub hp_songs_el: TemplateChild<gtk::Widget>,
        #[template_child] pub hp_albums_el: TemplateChild<gtk::Widget>,
        #[template_child] pub hp_artists_el: TemplateChild<gtk::Widget>,
        #[template_child] pub hp_playlists_el: TemplateChild<gtk::Widget>,
        #[template_child] pub sidebar_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child] pub visualizer_el: TemplateChild<gtk::Widget>,
        #[template_child] pub visualizer_bar_n_el: TemplateChild<gtk::Widget>,
        #[template_child] pub visualizer_type_el: TemplateChild<gtk::Widget>,
        #[template_child] pub visualizer_fill_el: TemplateChild<gtk::Widget>,
        #[template_child] pub visualizer_auto_color_el: TemplateChild<gtk::Widget>,
        #[template_child] pub visualizer_invert_auto_color_el: TemplateChild<gtk::Widget>,
        #[template_child] pub visualizer_manual_color_el: TemplateChild<gtk::ColorDialogButton>,
        pub default_pages: RefCell<Vec<&'static str>>,
        pub libraries: RefCell<Vec<crate::models::Ref>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesDialog {
        const NAME: &'static str = "NocturnePreferencesDialog";
        type Type = super::PreferencesDialog;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PreferencesDialog {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup();
        }
    }
    impl WidgetImpl for PreferencesDialog {}
    impl AdwDialogImpl for PreferencesDialog {}
    impl PreferencesDialogImpl for PreferencesDialog {}
}

glib::wrapper! {
    pub struct PreferencesDialog(ObjectSubclass<imp::PreferencesDialog>)
        @extends adw::PreferencesDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for PreferencesDialog {
    fn default() -> Self {
        Self::new()
    }
}

fn strings(combo: &adw::ComboRow) -> gtk::StringList {
    combo.model().and_downcast().unwrap_or_else(|| {
        let l = gtk::StringList::new(&[]);
        combo.set_model(Some(&l));
        l
    })
}

fn lazy_reload(w: &impl IsA<gtk::Widget>) {
    let _ = w.activate_action("app.lazy_reload_window", None);
}

#[gtk::template_callbacks]
impl PreferencesDialog {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn setup(&self) {
        let imp = self.imp();
        let s = settings();
        let integ = integration::current();
        let binds: [(&str, &gtk::Widget, &str); 30] = [
            ("restore-session", &imp.restore_el, "active"),
            ("hide-on-close", &imp.hide_on_close_el, "active"),
            ("simulate-word-by-word-lyrics", &imp.simulate_wbwl_el, "active"),
            ("auto-download-lyrics", &imp.auto_download_lyrics_el, "active"),
            ("use-gain", &imp.use_gain_el, "active"),
            ("discord-rpc-enabled", &imp.discord_rpc_el, "active"),
            ("discord-instance-art-share", &imp.discord_coverart_share_el, "active"),
            ("show-context-button", &imp.context_button_el, "active"),
            ("show-context-button-label", &imp.context_label_el, "active"),
            ("use-big-footer", &imp.footer_big_mode_el, "active"),
            ("player-blur-bg", &imp.translucent_player_el, "active"),
            ("use-sidebar-player", &imp.use_sidebar_player_el, "active"),
            ("hide-singles", &imp.hide_singles_el, "active"),
            ("button-size", &imp.button_size_el, "active-name"),
            ("global-dynamic-bg-mode", &imp.global_dynamic_bg_el, "active-name"),
            ("player-dynamic-bg-mode", &imp.player_dynamic_bg_el, "active-name"),
            ("popout-dynamic-bg-mode", &imp.popout_dynamic_bg_el, "active-name"),
            ("use-dynamic-accent", &imp.dynamic_accent_el, "active"),
            ("welcome-mode-home", &imp.home_mode_el, "active-name"),
            ("n-frequent-albums-home", &imp.hp_frequent_albums_el, "value"),
            ("n-songs-home", &imp.hp_songs_el, "value"),
            ("n-albums-home", &imp.hp_albums_el, "value"),
            ("n-artists-home", &imp.hp_artists_el, "value"),
            ("n-playlists-home", &imp.hp_playlists_el, "value"),
            ("show-visualizer", &imp.visualizer_el, "active"),
            ("visualizer-bar-n", &imp.visualizer_bar_n_el, "value"),
            ("visualizer-type", &imp.visualizer_type_el, "active-name"),
            ("visualizer-fill-mode", &imp.visualizer_fill_el, "active-name"),
            ("visualizer-auto-color", &imp.visualizer_auto_color_el, "active"),
            ("visualizer-auto-color-invert", &imp.visualizer_invert_auto_color_el, "active"),
        ];
        for (key, w, prop) in binds {
            s.bind(key, w, prop).build();
        }
        // GNOME search provider and Discord are not ported.
        imp.gnome_search_group_el.set_visible(false);
        imp.discord_rpc_el.set_visible(false);
        imp.discord_coverart_share_el.set_visible(false);

        // Default page
        let selected = s.string("default-page-tag");
        let model = strings(&imp.default_page_el);
        let mut tags = vec![];
        for sec in SIDEBAR_MENU {
            for item in sec.items.iter().filter(|i| !i.tag.is_empty()) {
                let title = if !sec.title.is_empty() && item.tag != "radios" { format!("{} ({})", sec.title, item.title) } else { item.title.into() };
                model.append(&title);
                if item.tag == selected {
                    imp.default_page_el.set_selected(tags.len() as u32);
                }
                tags.push(item.tag);
            }
        }
        imp.default_pages.replace(tags);

        // Bitrate
        let model = strings(&imp.bitrate_el);
        let br = s.int("max-bitrate");
        for (i, (title, kbps)) in BITRATES.iter().enumerate() {
            model.append(title);
            if *kbps == br {
                imp.bitrate_el.set_selected(i as u32);
            }
        }
        imp.bitrate_el.set_visible(integ.as_ref().is_some_and(|i| i.kind() != crate::backend::local::KIND));

        // Session
        imp.session_group_el.set_visible(integ.is_some());
        imp.instance_el.set_visible(false);
        imp.library_combo_el.set_visible(false);
        imp.library_expander_el.set_visible(false);
        let obj = self.clone();
        spawn_local(async move {
            let linked = crate::secret::lookup("listenbrainz").await.ok().flatten().is_some();
            obj.imp().listenbrainz_stack_el.set_visible_child_name(if linked { "unlink" } else { "link" });
        });
        if let Some(integ) = integ {
            let obj = self.clone();
            spawn_local(async move {
                if let Ok(info) = integ.call(|b| async move { b.server_info().await }).await {
                    let imp = obj.imp();
                    imp.instance_el.set_title(&info.username);
                    imp.instance_el.set_subtitle(&info.title);
                    imp.instance_el.set_tooltip_text(Some(&info.link));
                    imp.instance_el.set_action_target_value(Some(&info.link.to_variant()));
                    imp.instance_el.set_activatable(!info.link.is_empty());
                    imp.instance_icon_el.set_visible(!info.link.is_empty());
                    imp.instance_avatar_el.set_text(Some(&info.username));
                    let tex = info.avatar.as_deref().and_then(crate::integration::texture);
                    imp.instance_avatar_el.set_custom_image(tex.as_ref());
                    imp.instance_el.set_visible(true);
                }
                let (multi, libs) = integ.call(|b| async move { b.libraries().await }).await.unwrap_or_default();
                obj.show_libraries(multi, libs);
            });
        }

        // Sidebar
        let disabled_sections = s.strv("sidebar-disabled-sections");
        let disabled_items = s.strv("sidebar-disabled-items");
        for sec in SIDEBAR_MENU.iter().filter(|s| s.id != "root") {
            let expander = adw::ExpanderRow::builder()
                .title(sec.title)
                .subtitle("Show as a section")
                .show_enable_switch(true)
                .enable_expansion(!disabled_sections.iter().any(|d| d == sec.id))
                .build();
            let id = sec.id;
            expander.connect_enable_expansion_notify(move |e| {
                toggle_list("sidebar-disabled-sections", id, e.enables_expansion());
            });
            for item in sec.items {
                let row = adw::SwitchRow::builder().title(item.title).build();
                row.add_prefix(&gtk::Image::from_icon_name(item.icon));
                if item.id == "playlists-group" {
                    row.set_title("Show Playlists in Sidebar");
                    s.bind("show-playlists-in-sidebar", &row, "active").build();
                } else {
                    row.set_active(!disabled_items.iter().any(|d| d == item.id));
                    let id = item.id;
                    row.connect_active_notify(move |r| toggle_list("sidebar-disabled-items", id, r.is_active()));
                }
                expander.add_row(&row);
            }
            imp.sidebar_group.add(&expander);
        }

        // Visualizer colour
        let rgb: Vec<f32> = s.string("visualizer-manual-color").split(',').filter_map(|c| c.trim().parse().ok()).collect();
        let rgb = if rgb.len() >= 3 { rgb } else { vec![0.11, 0.44, 0.85] };
        imp.visualizer_manual_color_el.set_rgba(&gdk::RGBA::new(rgb[0], rgb[1], rgb[2], 1.0));
    }

    fn show_libraries(&self, multi: bool, libs: Vec<crate::models::Ref>) {
        let imp = self.imp();
        let stored: Vec<String> = settings().strv("library-ids").iter().map(|g| g.to_string()).collect();
        if multi && libs.len() > 1 {
            for lib in &libs {
                let row = adw::SwitchRow::builder().title(&lib.name).active(stored.contains(&lib.id)).build();
                let id = lib.id.clone();
                row.connect_active_notify(move |r| {
                    toggle_list("library-ids", &id, r.is_active());
                    lazy_reload(r);
                });
                imp.library_expander_el.add_row(&row);
            }
            imp.library_expander_el.set_visible(true);
        } else if !multi && !libs.is_empty() {
            let names: Vec<&str> = libs.iter().map(|l| l.name.as_str()).collect();
            imp.library_combo_el.set_model(Some(&gtk::StringList::new(&names)));
            let idx = stored.first().and_then(|s| libs.iter().position(|l| &l.id == s)).unwrap_or(0);
            imp.libraries.replace(libs);
            imp.library_combo_el.set_selected(idx as u32);
            imp.library_combo_el.set_visible(true);
        }
    }

    #[template_callback]
    fn library_combo_changed(&self, combo: &adw::ComboRow) {
        if !combo.is_mapped() {
            return;
        }
        let Some(lib) = self.imp().libraries.borrow().get(combo.selected() as usize).cloned() else { return };
        let s = settings();
        if s.strv("library-ids").first().map(|g| g.as_str()) != Some(lib.id.as_str()) {
            let _ = s.set_strv("library-ids", [lib.id.as_str()]);
            lazy_reload(combo);
        }
    }

    #[template_callback]
    fn default_page_changed(&self, combo: &adw::ComboRow) {
        let tag = self.imp().default_pages.borrow().get(combo.selected() as usize).copied().unwrap_or("home");
        let _ = settings().set_string("default-page-tag", tag);
    }

    #[template_callback]
    fn max_bitrate_changed(&self, combo: &adw::ComboRow) {
        let kbps = BITRATES.get(combo.selected() as usize).map_or(0, |b| b.1);
        let _ = settings().set_int("max-bitrate", kbps);
    }

    #[template_callback]
    fn visualizer_manual_color_changed(&self, btn: &gtk::ColorDialogButton) {
        let c = btn.rgba();
        let _ = settings().set_string("visualizer-manual-color", &format!("{:.3},{:.3},{:.3}", c.red(), c.green(), c.blue()));
    }

    #[template_callback]
    fn listenbrainz_link_requested(&self) {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 10);
        container.append(&gtk::LinkButton::with_label("https://listenbrainz.org/settings/", "Settings Page"));
        let token_el = gtk::Entry::builder().placeholder_text("User Token").build();
        container.append(&token_el);
        let dialog = adw::AlertDialog::new(Some("Link ListenBrainz"), Some("Connect your ListenBrainz account with a user token"));
        dialog.set_extra_child(Some(&container));
        dialog.add_responses(&[("cancel", "Cancel"), ("save", "Save")]);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        let obj = self.clone();
        spawn_local(async move {
            if dialog.choose_future(Some(&obj)).await != "save" {
                return;
            }
            let token = token_el.text().to_string();
            if token.is_empty() {
                return;
            }
            match crate::secret::store("listenbrainz", &token).await {
                Ok(()) => obj.imp().listenbrainz_stack_el.set_visible_child_name("unlink"),
                Err(e) => tracing::warn!("listenbrainz token: {e:#}"),
            }
        });
    }

    #[template_callback]
    fn listenbrainz_unlink_requested(&self) {
        let obj = self.clone();
        spawn_local(async move {
            let _ = crate::secret::delete("listenbrainz").await;
            obj.imp().listenbrainz_stack_el.set_visible_child_name("link");
        });
    }

    #[template_callback]
    fn delete_image_cache_requested(&self, button: &gtk::Widget) {
        if let Some(i) = integration::current() {
            let db = i.db();
            spawn_local(async move {
                let r = run(async move { sqlx::query("DELETE FROM images").execute(&db).await }).await;
                if let Err(e) = r {
                    tracing::warn!("delete image cache: {e}");
                }
            });
        }
        button.set_property("end-icon-name", "check-plain-symbolic");
        button.set_sensitive(false);
    }

    #[template_callback]
    fn home_mode_changed(&self, toggle: &gtk::Widget) {
        if self.root().is_none() {
            return;
        }
        let s = settings();
        let mode = toggle.property::<Option<String>>("active-name").unwrap_or_default();
        let pfp = data_dir().join("pfp");
        let _ = std::fs::remove_file(&pfp);
        let _ = s.set_string("welcome-user-home", "");
        match mode.as_str() {
            "system" => {
                let name = glib::real_name().to_string_lossy().into_owned();
                let name = if name.is_empty() || name == "Unknown" { glib::user_name().to_string_lossy().into_owned() } else { name };
                let _ = s.set_string("welcome-user-home", &name);
            }
            "instance" => {
                let Some(integ) = integration::current() else { return };
                let toggle = toggle.clone();
                spawn_local(async move {
                    match integ.call(|b| async move { b.server_info().await }).await {
                        Ok(info) if !info.username.is_empty() => {
                            let _ = settings().set_string("welcome-user-home", &info.username);
                            if let Some(bytes) = info.avatar {
                                let _ = std::fs::write(&pfp, bytes);
                            }
                        }
                        _ => toggle.set_property("active-name", ""),
                    }
                });
            }
            _ => {}
        }
    }
}

/// Add (`on == false`) or remove `id` from the string-list setting `key`, then rebuild the sidebar.
fn toggle_list(key: &str, id: &str, on: bool) {
    let s = settings();
    let mut list: Vec<String> = s.strv(key).iter().map(|g| g.to_string()).collect();
    let present = list.iter().any(|x| x == id);
    let disabled_list = key.starts_with("sidebar");
    let want = if disabled_list { !on } else { on };
    if want && !present {
        list.push(id.to_string());
    } else if !want && present {
        list.retain(|x| x != id);
    }
    let _ = s.set_strv(key, list);
    if disabled_list && let Some(app) = gio::Application::default().and_downcast::<Application>() {
        app.main_window().setup_sidebar();
    }
}
