//! Context menus (port of `containers/context.py` + the `CONTEXT_*` tables) and
//! small helpers shared by the item widgets.
use crate::{config, integration};
use adw::prelude::*;
use gtk::{gdk, glib};
use std::rc::Rc;

/// One context entry. Buttons activate `action` (an `app.*` name) with the model id as target.
#[derive(Clone)]
pub struct Item {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub action: &'static str,
    pub error: bool,
    pub sensitive: bool,
    pub callback: Option<Rc<dyn Fn()>>,
    /// `Some(rating)` turns the entry into the 5-star rating row.
    pub rating: Option<i32>,
}

const fn it(key: &'static str, name: &'static str, icon: &'static str, action: &'static str) -> (&'static str, &'static str, &'static str, &'static str) {
    (key, name, icon, action)
}

fn build(table: &[(&'static str, &'static str, &'static str, &'static str)]) -> Vec<Item> {
    table
        .iter()
        .map(|&(key, name, icon, action)| Item {
            key,
            name,
            icon,
            action,
            error: matches!(key, "delete" | "delete-radio" | "remove" | "delete-download") || (key == "remove-lyrics"),
            sensitive: true,
            callback: None,
            rating: None,
        })
        .collect()
}

pub trait ItemsExt {
    fn without(self, keys: &[&str]) -> Self;
    fn get_mut(&mut self, key: &str) -> Option<&mut Item>;
}

impl ItemsExt for Vec<Item> {
    fn without(mut self, keys: &[&str]) -> Self {
        self.retain(|i| !keys.contains(&i.key));
        self
    }
    fn get_mut(&mut self, key: &str) -> Option<&mut Item> {
        self.iter_mut().find(|i| i.key == key)
    }
}

/// Whether the current backend can download songs.
pub fn downloads_allowed() -> bool {
    integration::current().is_some_and(|i| i.kind() != crate::backend::local::KIND)
}

pub fn album_items() -> Vec<Item> {
    let v = build(&[
        it("play", "Play", "media-playback-start-symbolic", "app.play_album"),
        it("shuffle", "Shuffle", "playlist-shuffle-symbolic", "app.play_album_shuffle"),
        it("play-next", "Play Next", "list-high-priority-symbolic", "app.play_album_next"),
        it("play-later", "Play Later", "list-low-priority-symbolic", "app.play_album_later"),
        it("add-to-playlist", "Add To Playlist", "playlist-symbolic", "app.prompt_add_album_to_playlist"),
        it("download", "Download", "folder-download-symbolic", "app.download_album"),
        it("show-artist", "Show Artist", "music-artist-symbolic", "app.show_artist_from_album"),
    ]);
    if downloads_allowed() { v } else { v.without(&["download"]) }
}

pub fn artist_items() -> Vec<Item> {
    build(&[
        it("shuffle", "Shuffle", "playlist-shuffle-symbolic", "app.play_shuffle_artist"),
        it("radio", "Radio", "sound-symbolic", "app.play_radio_artist"),
    ])
}

pub fn playlist_items() -> Vec<Item> {
    let v = build(&[
        it("play", "Play", "media-playback-start-symbolic", "app.play_playlist"),
        it("resume", "Resume", "playback-options-symbolic", "app.resume_playlist"),
        it("shuffle", "Shuffle", "media-playlist-shuffle-symbolic", "app.play_playlist_shuffle"),
        it("play-next", "Play Next", "list-high-priority-symbolic", "app.play_playlist_next"),
        it("play-later", "Play Later", "list-low-priority-symbolic", "app.play_playlist_later"),
        it("download", "Download", "folder-download-symbolic", "app.download_playlist"),
        it("edit", "Edit", "document-edit-symbolic", "app.update_playlist"),
        it("delete", "Delete", "user-trash-symbolic", "app.delete_playlist"),
    ]);
    if downloads_allowed() { v } else { v.without(&["download"]) }
}

/// Full song table (`CONTEXT_SONG`); callers remove what doesn't apply.
pub fn song_items(id: &str) -> Vec<Item> {
    let mut v = build(&[
        it("select", "Select", "object-select-symbolic", ""),
        it("play-next", "Play Next", "list-high-priority-symbolic", "app.play_song_next"),
        it("play-later", "Play Later", "list-low-priority-symbolic", "app.play_song_later"),
        it("edit-radio", "Edit", "document-edit-symbolic", "app.update_radio"),
        it("add-to-playlist", "Add To Playlist", "playlist-symbolic", "app.prompt_add_song_to_playlist"),
        it("download", "Download", "folder-download-symbolic", "app.download_song"),
        it("show-album", "Show Album", "music-queue-symbolic", "app.show_album_from_song"),
        it("show-artist", "Show Artist", "music-artist-symbolic", "app.show_artist_from_song"),
        it("delete-radio", "Delete", "user-trash-symbolic", "app.delete_radio"),
        it("remove", "Remove", "user-trash-symbolic", ""),
        it("details", "Show Details", "info-outline-symbolic", "app.show_song_details"),
    ]);
    let integ = integration::get();
    let playing = integ.current_state().songId() == id;
    for k in ["play-next", "play-later"] {
        if let Some(i) = v.get_mut(k) {
            i.sensitive = !playing;
        }
    }
    v.push(Item { key: "rating", rating: Some(integ.song(id).userRating()), ..v[0].clone() });
    if downloads_allowed() { v } else { v.without(&["download"]) }
}

/// Simple song menu used by buttons / small rows.
pub fn simple_song_items(id: &str) -> Vec<Item> {
    song_items(id).without(&["edit-radio", "delete-radio", "remove", "select"])
}

pub fn lyrics_items() -> Vec<Item> {
    build(&[
        it("edit", "Edit Lyrics", "document-edit-symbolic", "app.edit_lyrics"),
        it("remove-lyrics", "Remove Lyrics", "user-trash-symbolic", ""),
    ])
}

fn rating_target(id: &str, rating: i32) -> glib::Variant {
    let d = glib::VariantDict::new(None);
    d.insert("model_id", id);
    d.insert("rating", rating);
    d.end()
}

/// Port of `get_context_buttons_list`. `on_click` runs after a button is clicked.
pub fn buttons(items: &[Item], model_id: &str, connect_style: bool, on_click: Option<Rc<dyn Fn(&gtk::Button)>>) -> Vec<gtk::Widget> {
    let mut out: Vec<gtk::Widget> = vec![];
    for item in items {
        let hook = |btn: &gtk::Button, cb: Option<Rc<dyn Fn()>>| {
            let on_click = on_click.clone();
            btn.connect_clicked(move |b| {
                if let Some(f) = &on_click {
                    f(b);
                }
                if let Some(cb) = &cb {
                    cb();
                }
            });
        };
        if let Some(value) = item.rating {
            out.push(gtk::Separator::builder().margin_top(5).margin_bottom(5).build().upcast());
            let row = gtk::Box::builder().css_classes(["linked"]).build();
            for i in 1..=5 {
                let btn = gtk::Button::builder()
                    .tooltip_text(if i == 1 { "1 Star".to_string() } else { format!("{i} Stars") })
                    .icon_name(if value >= i { "starred-symbolic" } else { "non-starred-symbolic" })
                    .css_classes(["flat"])
                    .action_name("app.set_rating")
                    .action_target(&rating_target(model_id, i))
                    .build();
                hook(&btn, None);
                row.append(&btn);
            }
            out.push(row.upcast());
            continue;
        }
        let content = adw::ButtonContent::builder()
            .label(item.name)
            .icon_name(item.icon)
            .halign(if connect_style { gtk::Align::Fill } else { gtk::Align::Start })
            .build();
        let btn = gtk::Button::builder().child(&content).tooltip_text(item.name).name(item.key).sensitive(item.sensitive).build();
        if item.error {
            btn.add_css_class("error");
        }
        if item.sensitive {
            hook(&btn, item.callback.clone());
            if !item.action.is_empty() {
                btn.set_action_name(Some(item.action));
                if !model_id.is_empty() {
                    btn.set_action_target_value(Some(&model_id.to_variant()));
                }
            }
        }
        if connect_style {
            let settings = config::settings();
            let apply = {
                let (btn, content) = (btn.downgrade(), content.downgrade());
                move |s: &gtk::gio::Settings| {
                    let (Some(btn), Some(content)) = (btn.upgrade(), content.upgrade()) else { return };
                    let visible = s.boolean("show-context-button-label");
                    content.set_label(&if visible { btn.tooltip_text().unwrap_or_default().to_string() } else { String::new() });
                    for c in ["circular", "flat"] {
                        if visible { btn.remove_css_class(c) } else { btn.add_css_class(c) }
                    }
                }
            };
            apply(&settings);
            settings.connect_changed(Some("show-context-button-label"), move |s, _| apply(s));
            // keep the settings object alive with the button
            unsafe { btn.set_data("settings", settings) };
        }
        out.push(btn.upcast());
    }
    out
}

/// Port of `ContextContainer`: vertical list of flat buttons that closes its popover on click.
pub fn container(items: &[Item], model_id: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let close: Rc<dyn Fn(&gtk::Button)> = Rc::new(|btn: &gtk::Button| {
        if let Some(p) = btn.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>() {
            p.popdown();
        }
    });
    for w in buttons(items, model_id, false, Some(close)) {
        w.add_css_class("flat");
        w.set_tooltip_text(None);
        b.append(&w);
    }
    b
}

/// Show `items` in a popover attached to `parent`, pointing at (x, y).
pub fn popup_items(parent: &impl IsA<gtk::Widget>, items: &[Item], model_id: &str, x: f64, y: f64) {
    popup(parent, &container(items, model_id), x, y);
}

pub fn popup(parent: &impl IsA<gtk::Widget>, child: &impl IsA<gtk::Widget>, x: f64, y: f64) {
    let popover = gtk::Popover::builder()
        .child(child)
        .pointing_to(&gdk::Rectangle::new(x as i32, y as i32, 1, 1))
        .has_arrow(false)
        .build();
    popover.set_parent(parent);
    popover.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    popover.popup();
}

/// (gesture, x, y) from the args of `GestureClick::released` / `GestureLongPress::pressed`.
pub fn gesture_point(args: &[glib::Value]) -> (Option<gtk::Widget>, f64, f64) {
    let n = args.len();
    let x = args.get(n.wrapping_sub(2)).and_then(|v| v.get::<f64>().ok()).unwrap_or(0.0);
    let y = args.get(n.wrapping_sub(1)).and_then(|v| v.get::<f64>().ok()).unwrap_or(0.0);
    let w = args.first().and_then(|v| v.get::<gtk::EventController>().ok()).and_then(|g| g.widget());
    (w, x, y)
}

/// Integer out of a closure constant (blueprint may type literals as int, uint or double).
pub fn value_i64(v: &glib::Value) -> i64 {
    v.get::<i32>()
        .map(i64::from)
        .or_else(|_| v.get::<i64>())
        .or_else(|_| v.get::<u32>().map(i64::from))
        .or_else(|_| v.get::<f64>().map(|f| f as i64))
        .unwrap_or(0)
}

/// Truthiness of a closure value (bool, numbers, strings, objects).
pub fn value_bool(v: &glib::Value) -> bool {
    if let Ok(b) = v.get::<bool>() {
        return b;
    }
    if let Ok(s) = v.get::<Option<String>>() {
        return s.is_some_and(|s| !s.is_empty());
    }
    if let Ok(o) = v.get::<Option<glib::Object>>() {
        return o.is_some();
    }
    value_i64(v) != 0
}

/// Accent the star button and keep it in sync with `starred` of `model`.
pub fn track_star(btn: &impl IsA<gtk::Widget>, model: &impl IsA<glib::Object>) {
    let apply = {
        let btn = btn.as_ref().downgrade();
        move |m: &glib::Object| {
            let Some(btn) = btn.upgrade() else { return };
            let s = m.property::<bool>("starred");
            if s { btn.add_css_class("accent") } else { btn.remove_css_class("accent") }
            if s { btn.remove_css_class("dim-label") } else { btn.add_css_class("dim-label") }
        }
    };
    apply(model.upcast_ref());
    model.connect_notify_local(Some("starred"), move |m, _| apply(m.upcast_ref()));
}

/// Follow the `button-size` setting: resize the cover image and swap title classes.
pub fn track_button_size(cover: Option<&gtk::Image>, avatar: Option<&adw::Avatar>, title: &gtk::Label) {
    let settings = config::settings();
    let apply = {
        let (cover, avatar, title) = (cover.map(|c| c.downgrade()), avatar.map(|a| a.downgrade()), title.downgrade());
        let s = settings.clone();
        Rc::new(move || {
            let big = s.string("button-size") == "big";
            let size = if big { 240 } else { 180 };
            if let Some(c) = cover.as_ref().and_then(|c| c.upgrade()) {
                c.set_size_request(size, size);
                c.set_pixel_size(if c.paintable().is_some() { size } else { -1 });
            }
            if let Some(a) = avatar.as_ref().and_then(|a| a.upgrade()) {
                a.set_size(size);
            }
            if let Some(t) = title.upgrade() {
                t.remove_css_class(if big { "title-4" } else { "title-3" });
                t.add_css_class(if big { "title-3" } else { "title-4" });
            }
        })
    };
    apply();
    let a = apply.clone();
    settings.connect_changed(Some("button-size"), move |_, _| a());
    if let Some(c) = cover {
        let a = apply.clone();
        c.connect_paintable_notify(move |_| a());
    }
    unsafe { title.set_data("button-size-settings", settings) };
}

/// Port of `update_background`: tint `widget` with the average colour of `paintable`.
pub fn tint_background(widget: &impl IsA<gtk::Widget>, paintable: Option<&gdk::Paintable>) {
    let Some(tex) = paintable.and_then(|p| p.downcast_ref::<gdk::Texture>()) else { return };
    let mut dl = gdk::TextureDownloader::new(tex);
    dl.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = dl.download_bytes();
    let (w, h) = (tex.width() as usize, tex.height() as usize);
    let (mut sum, mut n) = ([0u64; 3], 0u64);
    for y in (0..h).step_by(4) {
        for x in (0..w).step_by(4) {
            let p = &bytes[y * stride + x * 4..y * stride + x * 4 + 4];
            if p[3] > 125 {
                sum.iter_mut().zip(p).for_each(|(s, c)| *s += *c as u64);
                n += 1;
            }
        }
    }
    if n == 0 {
        return;
    }
    let css = format!(
        "clamp {{ transition: background .2s; background: linear-gradient(180deg, color-mix(in srgb, rgb({},{},{}) 50%, transparent), transparent 30%); background-size: 100% 1000px; background-repeat: no-repeat; }}",
        sum[0] / n,
        sum[1] / n,
        sum[2] / n
    );
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css);
    #[allow(deprecated)]
    widget.as_ref().style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

/// Callbacks shared by most item templates (all pure closures).
pub struct Formatters;

#[gtk::template_callbacks(functions)]
impl Formatters {
    #[template_callback]
    fn format_action_target(value: Option<String>, _variant: Option<String>) -> glib::Variant {
        value.unwrap_or_default().to_variant()
    }
    #[template_callback]
    fn format_to_bool(#[rest] v: &[glib::Value]) -> bool {
        v.first().is_some_and(value_bool)
    }
    #[template_callback]
    fn format_starred_tooltip_text(starred: bool) -> String {
        if starred { "Favorite" } else { "Not Favorite" }.into()
    }
    #[template_callback]
    fn format_starred_icon_name(starred: bool) -> String {
        if starred { "heart-filled-symbolic" } else { "heart-outline-thick-symbolic" }.into()
    }
    #[template_callback]
    fn format_rating_icon_name(#[rest] v: &[glib::Value]) -> String {
        let (rating, index) = (v.first().map(value_i64).unwrap_or(0), v.get(1).map(value_i64).unwrap_or(0));
        if rating >= index { "starred-symbolic" } else { "non-starred-symbolic" }.into()
    }
    #[template_callback]
    fn format_album_count_label(count: i32) -> String {
        if count == 1 { "1 Album".into() } else { format!("{count} Albums") }
    }
    #[template_callback]
    fn format_duration(duration: i32) -> String {
        if duration == -1 { "Radio".into() } else { config::display_time(duration as i64) }
    }
    #[template_callback]
    fn format_condensed_text(value: Option<String>) -> String {
        value.unwrap_or_default().replace('\n', " ")
    }
    #[template_callback]
    fn handle_visible_bind(value: Option<String>) -> bool {
        value.is_some_and(|v| !v.is_empty())
    }
    #[template_callback]
    fn handle_action_name_bind(value: Option<String>) -> Option<String> {
        value.filter(|v| !v.is_empty()).map(|_| "app.replace_root_page".into())
    }
    #[template_callback]
    fn handle_action_target_bind(value: Option<String>) -> glib::Variant {
        value.unwrap_or_default().to_variant()
    }
}

/// Activate `app.set_rating` for `id` from a rating button whose widget name is the star count.
pub fn change_rating(widget: &impl IsA<gtk::Widget>, id: &str, button: &gtk::Button) {
    let rating: i32 = button.widget_name().parse().unwrap_or(0);
    let _ = widget.as_ref().activate_action("app.set_rating", Some(&rating_target(id, rating)));
}

/// Target of `app.play_song_from_list`: `{songId, songs, originId}`.
pub fn play_from_list_target(song: &str, songs: &[String], origin: &str) -> glib::Variant {
    let d = glib::VariantDict::new(None);
    d.insert("songId", song);
    d.insert("songs", songs.to_vec());
    d.insert("originId", origin);
    d.end()
}
