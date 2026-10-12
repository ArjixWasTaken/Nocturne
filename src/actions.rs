//! `app.*` actions (actions.py and the action table of window.py).
use crate::{
    application::Application,
    config::{data_dir, downloads_dir, settings},
    integration::{self, Integration},
    models::SongDownload,
    queue,
    runtime::spawn_local,
    widgets::{
        album::AlbumPage,
        artist::ArtistPage,
        containers::PageDialog,
        lyrics::LyricsDialog,
        pages::{self, HomePage},
        playing::PopoutWindow,
        playlist::{PlaylistPage, dialog as playlist_dialog},
        song::SongDetailsDialog,
    },
    window::Window,
};
use adw::prelude::*;
use gtk::{gdk, gio, glib};

// ---------- registration ----------

fn add(app: &Application, name: &str, ty: Option<&str>, accels: &[&str], f: impl Fn(Option<&glib::Variant>) + 'static) {
    let action = gio::SimpleAction::new(name, ty.map(|t| glib::VariantTy::new(t).expect("variant type")));
    action.connect_activate(move |_, v| f(v));
    app.add_action(&action);
    if !accels.is_empty() {
        app.set_accels_for_action(&format!("app.{name}"), accels);
    }
}

fn none(app: &Application, name: &str, accels: &[&str], f: impl Fn() + 'static) {
    add(app, name, None, accels, move |_| f());
}

fn s(app: &Application, name: &str, f: impl Fn(String) + 'static) {
    add(app, name, Some("s"), &[], move |v| f(v.and_then(|v| v.get::<String>()).unwrap_or_default()));
}

fn strv(app: &Application, name: &str, f: impl Fn(Vec<String>) + 'static) {
    add(app, name, Some("as"), &[], move |v| f(v.and_then(|v| v.get::<Vec<String>>()).unwrap_or_default()));
}

fn dict(app: &Application, name: &str, f: impl Fn(glib::VariantDict) + 'static) {
    add(app, name, Some("a{sv}"), &[], move |v| f(glib::VariantDict::new(v)));
}

/// Like [`s`], but only runs (in an async task) while logged in.
fn s_async<F, Fut>(app: &Application, name: &str, f: F)
where
    F: Fn(Integration, String) -> Fut + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let f = std::rc::Rc::new(f);
    s(app, name, move |id| {
        if let Some(i) = integration::current() {
            spawn_local(f(i, id));
        }
    });
}

fn d_str(d: &glib::VariantDict, key: &str) -> String {
    d.lookup::<String>(key).ok().flatten().unwrap_or_default()
}

fn d_strv(d: &glib::VariantDict, key: &str) -> Vec<String> {
    d.lookup::<Vec<String>>(key).ok().flatten().unwrap_or_default()
}

fn d_int(d: &glib::VariantDict, key: &str) -> i64 {
    let Some(v) = d.lookup_value(key, None) else { return 0 };
    v.get::<i32>()
        .map(i64::from)
        .or_else(|| v.get::<i64>())
        .or_else(|| v.get::<u32>().map(i64::from))
        .or_else(|| v.get::<f64>().map(|f| f as i64))
        .or_else(|| v.get::<String>().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

// ---------- helpers ----------

fn main_window() -> Window {
    Application::get().main_window()
}

fn active_window() -> gtk::Window {
    Application::get().active_window().unwrap_or_else(|| main_window().upcast())
}

fn present(dialog: &impl IsA<adw::Dialog>) {
    dialog.present(Some(&active_window()));
}

/// Push `page` on the main window, or in a page dialog when the pop-out window is active.
pub fn show_page(page: &impl IsA<adw::NavigationPage>) {
    let active = active_window();
    if active.is::<PopoutWindow>() {
        let dialog = active.downcast_ref::<adw::ApplicationWindow>().and_then(|w| w.visible_dialog());
        match dialog.and_downcast::<PageDialog>() {
            Some(d) => d.push(page),
            None => PageDialog::new(page).present(Some(&active)),
        }
    } else {
        main_window().show_page(page);
    }
}

fn alert(heading: &str, body: &str) -> adw::AlertDialog {
    adw::AlertDialog::new(Some(heading), Some(body))
}

fn toast(title: &str) {
    main_window().toast(title);
}

/// Toast with a cover/icon, a title and a subtitle.
fn custom_toast(title: &str, subtitle: &str, icon: Option<&str>, paintable: Option<gdk::Paintable>) {
    let row = adw::ActionRow::builder().title(title).subtitle(subtitle).build();
    if let Some(icon) = icon {
        row.add_prefix(&gtk::Image::from_icon_name(icon));
    } else {
        let img = gtk::Image::builder()
            .css_classes(["card"])
            .height_request(48)
            .width_request(48)
            .overflow(gtk::Overflow::Hidden)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .icon_name("music-note-symbolic")
            .pixel_size(32)
            .build();
        if let Some(p) = paintable {
            img.set_paintable(Some(&p));
            img.set_pixel_size(48);
        }
        row.add_prefix(&img);
    }
    main_window().add_toast(adw::Toast::builder().custom_title(&row).timeout(2).build());
}

/// Toast titled with `prop` ("title"/"name") of model `id`.
fn toast_model(id: &str, prop: &str, subtitle: &str) {
    let m = integration::current().and_then(|i| i.lookup(id));
    let title = m.as_ref().map(|m| m.property::<String>(prop)).unwrap_or_default();
    custom_toast(&title, subtitle, None, m.and_then(|m| m.property::<Option<gdk::Paintable>>("gdkPaintable")));
}

fn songs_label(n: usize) -> String {
    if n == 1 { "1 Song".into() } else { format!("{n} Songs") }
}

fn toast_songs(ids: &[String], subtitle: &str, icon: &str) {
    match ids {
        [one] => toast_model(one, "title", subtitle),
        _ => custom_toast(&songs_label(ids.len()), subtitle, Some(icon), None),
    }
}

fn play_or_select(id: String) {
    let Some(i) = integration::current() else { return };
    let state = i.current_state();
    if state.queue_ids().contains(&id) {
        state.set_songId(id);
    } else {
        queue::replace(&[id], None, "");
    }
}

async fn album_songs(i: &Integration, id: &str) -> Vec<String> {
    i.verify_album(id, true).await.song().0
}

async fn playlist_songs(i: &Integration, id: &str) -> Vec<String> {
    i.verify_playlist(id, true).await.entry().0
}

fn clear_queue() {
    if let Some(i) = integration::current() {
        let state = i.current_state();
        queue::save_playlist_resume();
        state.set_queue_ids(&[]);
        state.set_queueOrigin("");
        state.set_songId("");
    }
}

/// Re-read the playlists shown in the sidebar.
fn refresh_sidebar() {
    main_window().setup_sidebar_playlists();
}

fn edit_radio(id: Option<String>) {
    let Some(integ) = integration::current() else { return };
    let model = id.as_deref().map(|id| integ.song(id));
    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).build();
    let name_el = adw::EntryRow::builder().title("Name").build();
    let stream_el = adw::EntryRow::builder().title("Stream Url").build();
    if let Some(m) = &model {
        name_el.set_text(&m.title());
        stream_el.set_text(&m.radioStreamUrl());
    }
    list.append(&name_el);
    list.append(&stream_el);
    let editing = id.is_some();
    let dialog = alert(if editing { "Update Radio Station" } else { "Add Radio Station" }, "");
    dialog.set_extra_child(Some(&list));
    dialog.add_responses(&[("cancel", "Cancel"), ("save", "Save")]);
    dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
    let parent = active_window();
    spawn_local(async move {
        if dialog.choose_future(Some(&parent)).await != "save" {
            return;
        }
        let name = Some(name_el.text().trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| "No Name".into());
        let mut stream = stream_el.text().trim().to_string();
        if !stream.starts_with("http") {
            stream = format!("http://{stream}");
        }
        let (n, st) = (name.clone(), stream.clone());
        let ok = match id.clone() {
            Some(id) => integ.call(move |b| async move { b.update_radio(&id, &n, &st).await }).await,
            None => integ.call(move |b| async move { b.create_radio(&n, &st).await }).await,
        };
        match ok {
            Ok(()) => {
                if let Some(m) = &model {
                    m.set_title(name);
                    m.set_radioStreamUrl(stream);
                } else {
                    main_window().reload_visible_page();
                }
                toast(if editing { "Radio updated successfully" } else { "Radio added successfully" });
            }
            Err(e) => {
                tracing::warn!("radio: {e:#}");
                toast(if editing { "Error updating radio" } else { "Error adding radio" });
            }
        }
    });
}

fn edit_playlist(id: Option<String>) {
    let Some(integ) = integration::current() else { return };
    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).build();
    let name_el = adw::EntryRow::builder().title("Name").build();
    if let Some(id) = &id {
        name_el.set_text(&integ.playlist(id).name());
    }
    list.append(&name_el);
    let editing = id.is_some();
    let dialog = alert(if editing { "Update Playlist" } else { "Create Playlist" }, "");
    dialog.set_extra_child(Some(&list));
    dialog.add_responses(&[("cancel", "Cancel"), ("create", if editing { "Update" } else { "Create" })]);
    dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
    let parent = active_window();
    spawn_local(async move {
        if dialog.choose_future(Some(&parent)).await != "create" {
            return;
        }
        let name = name_el.text().trim().to_string();
        if name.is_empty() {
            return;
        }
        let r = match id.clone() {
            Some(pid) => integ.call(move |b| async move { b.update_playlist(&pid, Some(&name), &[], &[]).await.map(|_| pid) }).await,
            None => integ.call(move |b| async move { b.create_playlist(&name, &[]).await }).await,
        };
        match r {
            Ok(pid) => {
                integ.verify_playlist(&pid, true).await;
                main_window().reload_visible_page();
                refresh_sidebar();
                toast(if editing { "Playlist updated successfully" } else { "Playlist created successfully" });
            }
            Err(e) => {
                tracing::warn!("playlist: {e:#}");
                toast(if editing { "Error updating playlist" } else { "Error creating playlist" });
            }
        }
    });
}

// ---------- downloads ----------

/// Start downloading `id`; false if it is already downloaded or queued.
async fn request_download(integ: &Integration, id: &str) -> bool {
    let song = integ.verify_song(id, false).await;
    let artist = song.artists().0.first().map(|a| a.name.clone()).unwrap_or_else(|| song.artist().split(';').next().unwrap_or("").to_string());
    let file_title = format!("{} - {} - {}", song.title(), song.album(), artist).replace('/', " ");
    let dir = downloads_dir();
    let exists = std::fs::read_dir(&dir).ok().is_some_and(|rd| {
        rd.flatten().any(|e| e.path().file_stem().is_some_and(|s| s.to_string_lossy() == file_title))
    });
    let queue = integ.current_state().downloadQueueModel();
    if exists || queue.iter::<SongDownload>().flatten().any(|d| d.id() == id) {
        return false;
    }
    let ext = std::path::Path::new(&song.path())
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .or_else(|| Some(song.container()).filter(|c| !c.is_empty()))
        .unwrap_or_else(|| "mp3".into());
    let model = SongDownload::new(id);
    queue.insert(0, &model);
    let dest = dir.join(format!("{file_title}.{ext}"));
    let (integ, id) = (integ.clone(), id.to_string());
    spawn_local(async move {
        let _ = std::fs::create_dir_all(&dir);
        match integ.call(move |b| async move { b.download(&id, &dest).await }).await {
            Ok(()) => model.set_progress(1.0),
            Err(e) => {
                tracing::warn!("download: {e:#}");
                if let Some(pos) = queue.find(&model) {
                    queue.remove(pos);
                }
            }
        }
    });
    true
}

async fn download_many(integ: Integration, ids: Vec<String>, toast_id: Option<String>) {
    let mut started = 0;
    for id in &ids {
        started += request_download(&integ, id).await as usize;
    }
    let skipped = ids.len() - started;
    let subtitle = if started == 0 {
        "Already Downloaded".to_string()
    } else if skipped > 0 {
        format!("Download Started ({} Skipped)", songs_label(skipped))
    } else {
        "Download Started".to_string()
    };
    match toast_id {
        Some(id) => toast_model(&id, "name", &subtitle),
        None => custom_toast(&songs_label(ids.len()), &subtitle, Some("folder-download-symbolic"), None),
    }
}

fn delete_download(id: &str) -> bool {
    let Some(i) = integration::current() else { return false };
    let song = i.song(id);
    song.set_deleted(true);
    std::fs::remove_file(song.path()).is_ok()
}

// ---------- table ----------

pub fn register(app: &Application) {
    // Application
    none(app, "quit", &["<control>q"], || {
        let app = Application::get();
        spawn_local(async move {
            app.save_session().await;
            app.quit();
        });
    });
    none(app, "preferences", &["<control>comma"], || present(&crate::preferences::PreferencesDialog::new()));
    none(app, "about", &[], || {
        let about = adw::AboutDialog::builder()
            .application_icon(crate::config::APP_ID)
            .application_name("Nocturne")
            .developer_name("ArjixWasTaken")
            .issue_url("https://github.com/ArjixWasTaken/Nocturne/issues")
            .license_type(gtk::License::Gpl30)
            .support_url("https://github.com/ArjixWasTaken/Nocturne/discussions")
            .version(crate::config::VERSION)
            .website("https://arjix.dev")
            .developers(["ArjixWasTaken https://arjix.dev", "Jeffry Samuel (original Python version) https://jeffser.com"])
            .designers(["Jeffry Samuel https://jeffser.com"])
            .build();
        present(&about);
    });
    none(app, "shortcuts", &["<control>question"], || {
        let b = gtk::Builder::from_resource("/dev/arjix/Nocturne/shortcuts-dialog.ui");
        if let Some(d) = b.object::<adw::Dialog>("shortcuts_dialog") {
            present(&d);
        }
    });

    // Misc
    none(app, "reload", &["<ctrl>R"], || {
        let active = active_window();
        if active.is::<PopoutWindow>() {
            let page = active
                .downcast_ref::<adw::ApplicationWindow>()
                .and_then(|w| w.visible_dialog())
                .and_downcast::<PageDialog>()
                .and_then(|d| d.navigation_view().visible_page());
            if let Some(p) = page {
                reload_page(&p);
            }
        } else if let Some(p) = main_window().navigation_view().visible_page() {
            reload_page(&p);
        }
    });
    s(app, "show_error", |msg| {
        let d = alert("Error", if msg.is_empty() { "No details provided" } else { &msg });
        d.add_response("close", "Close");
        present(&d);
    });
    none(app, "search", &["<ctrl>F"], || {
        let active = active_window();
        let w = main_window();
        if active == *w.upcast_ref::<gtk::Window>() {
            w.select_sidebar_tag("home");
            w.replace_root_page("home");
            if let Some(h) = w.navigation_view().find_page("home").and_downcast::<HomePage>() {
                h.toggle_search();
            }
            return;
        }
        let dialog = active.downcast_ref::<adw::ApplicationWindow>().and_then(|w| w.visible_dialog()).and_downcast::<PageDialog>();
        let home = dialog.and_then(|d| d.navigation_view().find_page("home")).and_downcast::<HomePage>();
        let home = home.unwrap_or_else(|| {
            let h = HomePage::new();
            show_page(&h);
            pages::LibraryPage::load(&h);
            h
        });
        home.toggle_search();
    });
    none(app, "launch_playback", &[], || {
        if integration::current().is_none() {
            return;
        }
        let w = main_window();
        w.show_loading("");
        spawn_local(async move {
            let (top, _, label) = crate::window::last_month_playback().await;
            if top.len() > 5 {
                w.main_stack().set_visible_child_name("playback");
                w.playback_page().setup(&top, &label);
                queue::replace(&top.iter().map(|t| t.0.clone()).collect::<Vec<_>>(), None, "");
            } else {
                w.show_content();
                w.toast(&format!("Not enough data found for Nocturne Playback ({label})"));
            }
        });
    });
    add(app, "generate_auto_play_queue", Some("b"), &[], |v| {
        if integration::current().is_some() {
            queue::generate_auto_play(v.and_then(|v| v.get::<bool>()).unwrap_or(false));
        }
    });
    s(app, "set_equalizer_preset", |name| crate::widgets::playing::equalizer_page::apply_preset(&name));
    s(app, "replace_root_page", |tag| {
        let w = main_window();
        w.select_sidebar_tag(&tag);
        w.replace_root_page(&tag);
    });
    s(app, "visit_url", |url| {
        let uri = match url.strip_prefix("file://") {
            Some(path) => gio::File::for_path(path).uri().to_string(),
            None => url,
        };
        gtk::UriLauncher::new(&uri).launch(Some(&active_window()), gio::Cancellable::NONE, |r| {
            if let Err(e) = r {
                tracing::warn!("open uri: {e}");
            }
        });
    });
    s_async(app, "toggle_star", |i, id| async move {
        if let Some(m) = i.lookup(&id) {
            let starred = m.property::<bool>("starred");
            i.set_starred(&id, !starred).await;
        }
    });
    none(app, "logout", &[], || {
        let app = Application::get();
        let w = app.main_window();
        clear_queue();
        let s = settings();
        let _ = s.set_string("integration-user", "");
        let _ = s.set_strv("library-ids", Vec::<String>::new());
        if s.string("welcome-mode-home") == "instance" {
            let _ = std::fs::remove_file(data_dir().join("pfp"));
            let _ = s.set_string("welcome-user-home", "");
            let _ = s.set_string("welcome-mode-home", "");
        }
        if let Some(d) = w.visible_dialog() {
            d.close();
        }
        for p in w.root_pages() {
            pages::reset(&p);
        }
        w.replace_root_page("home");
        spawn_local(async move { app.logout().await });
    });
    none(app, "lazy_reload_window", &[], || {
        let w = main_window();
        clear_queue();
        for p in w.root_pages() {
            pages::set_lazy_reload(&p);
        }
        let tag = settings().string("default-page-tag");
        let tag = if tag.is_empty() { "home".into() } else { tag.to_string() };
        w.select_sidebar_tag(&tag);
        w.replace_root_page(&tag);
        w.reload_visible_page();
    });
    none(app, "show_external_file_warning", &[], || {
        let d = alert(
            "External File",
            "This track was loaded from an external file, this means it will have less features compared to a track inside the library",
        );
        d.add_response("close", "Close");
        present(&d);
    });
    none(app, "open_popout_window", &["<ctrl>P"], || open_popout(false));
    none(app, "toggle_fullscreen", &["F11"], || {
        let Some(i) = integration::current() else { return };
        if i.current_state().queueModel().n_items() == 0 {
            return;
        }
        match Application::get().popout_window() {
            Some(p) => {
                p.present();
                if p.is_fullscreen() { p.unfullscreen() } else { p.fullscreen() }
            }
            None => open_popout(true),
        }
    });
    dict(app, "set_rating", |d| {
        let Some(i) = integration::current() else { return };
        let id = d_str(&d, "model_id");
        let mut rating = d_int(&d, "rating") as i32;
        let Some(m) = i.lookup(&id) else { return };
        if rating == 0 {
            return;
        }
        if rating == m.property::<i32>("userRating") {
            rating = 0;
        }
        spawn_local(async move {
            i.set_rating(&id, rating).await;
        });
    });

    // Player
    let player = |f: fn(&crate::player::Player)| move || {
        if let Some(p) = Application::get().player() {
            f(&p);
        }
    };
    none(app, "player_toggle", &["<ctrl>K"], player(|p| p.toggle()));
    none(app, "player_play", &[], player(|p| p.play()));
    none(app, "player_pause", &[], player(|p| p.pause()));
    none(app, "player_next", &["<ctrl>Right"], player(|p| p.next()));
    none(app, "player_previous", &["<ctrl>Left"], player(|p| p.previous()));
    let volume = |delta: f64| move || {
        let s = settings();
        let _ = s.set_double("volume", (s.double("volume") + delta).clamp(0.0, 1.0));
    };
    none(app, "player_raise_volume", &["<ctrl>Up"], volume(0.1));
    none(app, "player_lower_volume", &["<ctrl>Down"], volume(-0.1));

    // Radio
    s(app, "play_radio", play_or_select);
    none(app, "add_radio", &[], || edit_radio(None));
    s(app, "update_radio", |id| edit_radio(Some(id).filter(|i| !i.is_empty())));
    s_async(app, "delete_radio", |i, id| async move {
        let d = alert("Delete Radio Station", &format!("Are you sure you want to delete '{}'?", i.song(&id).title()));
        d.add_responses(&[("cancel", "Cancel"), ("delete", "Delete")]);
        d.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        if d.choose_future(Some(&active_window())).await != "delete" {
            return;
        }
        match i.call(move |b| async move { b.delete_radio(&id).await }).await {
            Ok(()) => {
                toast("Radio deleted successfully");
                main_window().reload_visible_page();
            }
            Err(e) => {
                tracing::warn!("delete radio: {e:#}");
                toast("Error deleting radio");
            }
        }
    });

    // Song
    s(app, "play_song", play_or_select);
    dict(app, "play_song_from_list", |d| {
        let id = d_str(&d, "songId");
        if id.is_empty() || integration::current().is_none() {
            return;
        }
        let mut songs = d_strv(&d, "songs");
        if songs.is_empty() {
            songs = vec![id.clone()];
        }
        queue::replace(&songs, Some(&id), &d_str(&d, "originId"));
    });
    s(app, "play_song_next", |id| {
        if integration::current().is_some() {
            queue::play_next(std::slice::from_ref(&id));
            toast_model(&id, "title", "Playing Next");
        }
    });
    s(app, "play_song_later", |id| {
        if integration::current().is_some() {
            queue::play_later(std::slice::from_ref(&id));
            toast_model(&id, "title", "Playing Later");
        }
    });
    strv(app, "play_songs", |ids| {
        if integration::current().is_some() {
            queue::replace(&ids, None, "");
        }
    });
    strv(app, "play_songs_next", |ids| {
        if integration::current().is_some() && !ids.is_empty() {
            queue::play_next(&ids);
            toast_songs(&ids, "Playing Next", "list-high-priority-symbolic");
        }
    });
    strv(app, "play_songs_later", |ids| {
        if integration::current().is_some() && !ids.is_empty() {
            queue::play_later(&ids);
            toast_songs(&ids, "Playing Later", "list-low-priority-symbolic");
        }
    });
    none(app, "edit_lyrics", &[], || present(&LyricsDialog::new()));
    none(app, "play_random_queue", &[], || {
        let Some(i) = integration::current() else { return };
        spawn_local(async move {
            let ids = i.random_songs(50).await;
            queue::replace(&ids, None, "");
        });
    });
    s(app, "show_song_details", |id| present(&SongDetailsDialog::new(&id)));

    // Album
    s(app, "show_album", |id| show_page(&AlbumPage::new(&id)));
    s_async(app, "show_album_from_song", |i, id| async move {
        let album = i.verify_song(&id, false).await.albumId();
        if !album.is_empty() {
            show_page(&AlbumPage::new(&album));
        }
    });
    s_async(app, "play_album", |i, id| async move {
        let songs = album_songs(&i, &id).await;
        queue::replace(&songs, None, &id);
    });
    s_async(app, "play_album_next", |i, id| async move {
        let songs = album_songs(&i, &id).await;
        queue::play_next(&songs);
        toast_model(&id, "name", "Playing Next");
    });
    s_async(app, "play_album_later", |i, id| async move {
        let songs = album_songs(&i, &id).await;
        queue::play_later(&songs);
        toast_model(&id, "name", "Playing Later");
    });
    s_async(app, "play_album_shuffle", |i, id| async move {
        let songs = queue::shuffled(album_songs(&i, &id).await);
        queue::replace(&songs, None, &id);
    });

    // Playlist
    s(app, "show_playlist", |id| show_page(&PlaylistPage::new(&id)));
    s_async(app, "resume_playlist", |i, id| async move {
        let songs = playlist_songs(&i, &id).await;
        let resume = i.playlist_resume(&id).await;
        queue::replace(&songs, resume.as_ref().map(|r| r.0.as_str()), &id);
        if let Some((_, ts)) = resume
            && ts > 0.0
        {
            glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                if let Some(p) = Application::get().player() {
                    p.seek(ts);
                }
            });
        }
    });
    s_async(app, "play_playlist", |i, id| async move {
        let songs = playlist_songs(&i, &id).await;
        queue::replace(&songs, None, &id);
    });
    s_async(app, "play_playlist_next", |i, id| async move {
        let songs = playlist_songs(&i, &id).await;
        queue::play_next(&songs);
        toast_model(&id, "name", "Playing Next");
    });
    s_async(app, "play_playlist_later", |i, id| async move {
        let songs = playlist_songs(&i, &id).await;
        queue::play_later(&songs);
        toast_model(&id, "name", "Playing Later");
    });
    s_async(app, "play_playlist_shuffle", |i, id| async move {
        let songs = queue::shuffled(playlist_songs(&i, &id).await);
        queue::replace(&songs, None, &id);
    });
    s(app, "update_playlist", |id| edit_playlist(Some(id).filter(|i| !i.is_empty())));
    none(app, "create_playlist", &[], || edit_playlist(None));
    dict(app, "remove_songs_from_playlist", |d| {
        let Some(i) = integration::current() else { return };
        let id = d_str(&d, "playlist");
        let indexes: Vec<u32> = d_strv(&d, "indexes").iter().filter_map(|s| s.parse().ok()).collect();
        if id.is_empty() || indexes.is_empty() {
            return;
        }
        spawn_local(async move {
            let (pid, idx) = (id.clone(), indexes.clone());
            match i.call(move |b| async move { b.update_playlist(&pid, None, &[], &idx).await }).await {
                Ok(()) => {
                    let n = indexes.len();
                    toast_model(&id, "name", &format!("{} Removed", songs_label(n)));
                    i.verify_playlist(&id, true).await;
                }
                Err(e) => tracing::warn!("remove from playlist: {e:#}"),
            }
        });
    });
    strv(app, "prompt_add_songs_to_playlist", |ids| playlist_dialog::prompt_add_songs(&active_window(), ids));
    s(app, "prompt_add_song_to_playlist", |id| playlist_dialog::prompt_add_songs(&active_window(), vec![id]));
    s_async(app, "prompt_add_album_to_playlist", |i, id| async move {
        let songs = album_songs(&i, &id).await;
        playlist_dialog::prompt_add_songs(&active_window(), songs);
    });
    dict(app, "add_songs_to_playlist", |d| {
        let Some(i) = integration::current() else { return };
        if let Some(dialog) = active_window().downcast_ref::<adw::ApplicationWindow>().and_then(|w| w.visible_dialog()) {
            dialog.close();
        }
        let (new_name, target, songs) = (d_str(&d, "new_playlist"), d_str(&d, "playlist"), d_strv(&d, "songs"));
        spawn_local(async move {
            if !new_name.is_empty() {
                let s2 = songs.clone();
                match i.call(move |b| async move { b.create_playlist(&new_name, &s2).await }).await {
                    Ok(id) => {
                        i.verify_playlist(&id, true).await;
                        toast_model(&id, "name", &format!("{} Added", songs_label(songs.len())));
                        refresh_sidebar();
                    }
                    Err(e) => {
                        tracing::warn!("create playlist: {e:#}");
                        toast("Error creating playlist");
                    }
                }
            } else if !target.is_empty() {
                let existing = playlist_songs(&i, &target).await;
                let new: Vec<String> = songs.iter().filter(|s| !existing.contains(s)).cloned().collect();
                let (pid, add) = (target.clone(), new.clone());
                if let Err(e) = i.call(move |b| async move { b.update_playlist(&pid, None, &add, &[]).await }).await {
                    tracing::warn!("add to playlist: {e:#}");
                    toast("Error updating playlist");
                    return;
                }
                i.verify_playlist(&target, true).await;
                let mut msg = vec![];
                if !new.is_empty() {
                    msg.push(format!("{} Added", songs_label(new.len())));
                }
                let skipped = songs.len() - new.len();
                if skipped > 0 {
                    msg.push(format!("{} Skipped", songs_label(skipped)));
                }
                toast_model(&target, "name", &msg.join(" | "));
            }
        });
    });
    s_async(app, "delete_playlist", |i, id| async move {
        let name = i.playlist(&id).name();
        let d = alert("Delete Playlist", &format!("Are you sure you want to delete '{name}'?"));
        d.add_responses(&[("cancel", "Cancel"), ("delete", "Delete")]);
        d.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        if d.choose_future(Some(&active_window())).await != "delete" {
            return;
        }
        let pid = id.clone();
        match i.call(move |b| async move { b.delete_playlist(&pid).await }).await {
            Ok(()) => {
                toast_model(&id, "name", "Playlist Deleted");
                let w = main_window();
                if w.navigation_view().visible_page().and_downcast::<PlaylistPage>().is_some() {
                    w.navigation_view().pop();
                }
                w.reload_visible_page();
                refresh_sidebar();
            }
            Err(e) => {
                tracing::warn!("delete playlist: {e:#}");
                toast("Error deleting playlist");
            }
        }
    });

    // Artist
    s(app, "show_artist", |id| show_page(&ArtistPage::new(&id)));
    s_async(app, "show_artist_from_song", |i, id| async move {
        let artist = i.verify_song(&id, false).await.artistId();
        if !artist.is_empty() {
            show_page(&ArtistPage::new(&artist));
        }
    });
    s_async(app, "show_artist_from_album", |i, id| async move {
        let album = i.verify_album(&id, false).await;
        let artist = Some(album.artistId()).filter(|a| !a.is_empty()).or_else(|| album.artists().0.first().map(|a| a.id.clone()));
        if let Some(a) = artist.filter(|a| !a.is_empty()) {
            show_page(&ArtistPage::new(&a));
        }
    });
    s_async(app, "play_shuffle_artist", |i, id| async move {
        let albums = i.verify_artist(&id, true).await.album().0;
        let mut songs = vec![];
        for a in albums {
            songs.extend(album_songs(&i, &a).await);
        }
        let mut songs = queue::shuffled(songs);
        songs.truncate(20);
        if !songs.is_empty() {
            queue::replace(&songs, None, "");
        }
    });
    s_async(app, "play_radio_artist", |i, id| async move {
        let songs = i.similar_songs(&id, 50).await;
        if songs.is_empty() {
            toast("No songs found");
        } else {
            queue::replace(&songs, None, "");
        }
    });

    // Downloads
    s_async(app, "download_song", |i, id| async move {
        let started = request_download(&i, &id).await;
        toast_model(&id, "title", if started { "Download Started" } else { "Already Downloaded" });
    });
    strv(app, "download_songs", |ids| {
        let Some(i) = integration::current() else { return };
        match ids.as_slice() {
            [] => {}
            [one] => {
                let one = one.clone();
                spawn_local(async move {
                    let started = request_download(&i, &one).await;
                    toast_model(&one, "title", if started { "Download Started" } else { "Already Downloaded" });
                });
            }
            _ => spawn_local(download_many(i, ids, None)),
        }
    });
    s_async(app, "download_album", |i, id| async move {
        let songs = album_songs(&i, &id).await;
        download_many(i, songs, Some(id)).await;
    });
    s_async(app, "download_playlist", |i, id| async move {
        let songs = playlist_songs(&i, &id).await;
        download_many(i, songs, Some(id)).await;
    });
    s(app, "delete_download", |id| {
        if delete_download(&id) {
            toast_model(&id, "title", "Deleted");
        }
    });
    strv(app, "delete_downloads", |ids| {
        let n = ids.iter().filter(|id| delete_download(id)).count();
        custom_toast(&songs_label(n), "Deleted", Some("user-trash-symbolic"), None);
    });
}

fn reload_page(page: &adw::NavigationPage) {
    if let Some(p) = page.downcast_ref::<AlbumPage>() {
        p.reload();
    } else if let Some(p) = page.downcast_ref::<ArtistPage>() {
        p.reload();
    } else {
        pages::reload(page);
    }
}

fn open_popout(fullscreen: bool) {
    let app = Application::get();
    let w = app.popout_window().unwrap_or_else(|| {
        let w = PopoutWindow::new(&app, fullscreen);
        app.set_popout_window(Some(w.clone()));
        w.connect_destroy(|_| Application::get().set_popout_window(None));
        w
    });
    w.present();
}
