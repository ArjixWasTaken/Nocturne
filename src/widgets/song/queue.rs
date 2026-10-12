//! `NocturneSongQueue` (song/queue.blp): list of `SongRow`s with a selection toolbar.
use super::SongRow;
use crate::widgets::containers::context::{self, Formatters};
use crate::{integration, models};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/dev/arjix/Nocturne/song/queue.ui")]
    #[properties(wrapper_type = super::SongQueue)]
    pub struct SongQueue {
        #[template_child]
        pub header_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub toolbar_revealer_el: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub list_el: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub remove_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub play_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub play_next_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub play_later_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub download_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub delete_download_el: TemplateChild<gtk::Button>,
        #[property(get, set)]
        pub header_label: RefCell<String>,
        #[property(get, set)]
        pub header_icon_name: RefCell<String>,
        #[property(get, set)]
        pub header_page_tag: RefCell<String>,
        /// Set when the list shows a playlist (removal edits the playlist).
        #[property(get, set)]
        pub playlist_id: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongQueue {
        const NAME: &'static str = "NocturneSongQueue";
        type Type = super::SongQueue;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
            Formatters::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl SongQueue {
        fn act(&self, action: &str) {
            let ids: Vec<String> = self.obj().selected_rows().iter().map(|r| r.id()).collect();
            let _ = self.obj().activate_action(action, Some(&ids.to_variant()));
            self.obj().set_selected_mode(false, None);
        }
        #[template_callback]
        fn close_selector(&self) {
            self.obj().set_selected_mode(false, None);
        }
        #[template_callback]
        fn delete_download_selected(&self) {
            self.act("app.delete_downloads");
        }
        #[template_callback]
        fn download_selected(&self) {
            self.act("app.download_songs");
        }
        #[template_callback]
        fn play_selected(&self) {
            self.act("app.play_songs");
        }
        #[template_callback]
        fn play_next_selected(&self) {
            self.act("app.play_songs_next");
        }
        #[template_callback]
        fn play_later_selected(&self) {
            self.act("app.play_songs_later");
        }
        #[template_callback]
        fn add_to_playlist_selected(&self) {
            self.act("app.prompt_add_songs_to_playlist");
        }
        #[template_callback]
        fn select_all(&self) {
            self.obj().rows().iter().for_each(|r| r.set_checked(true));
        }
        #[template_callback]
        fn remove_selected(&self) {
            let obj = self.obj();
            obj.remove_rows(&obj.selected_rows());
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SongQueue {}
    impl WidgetImpl for SongQueue {}
    impl BoxImpl for SongQueue {}
}

glib::wrapper! {
    pub struct SongQueue(ObjectSubclass<imp::SongQueue>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl SongQueue {
    pub fn list(&self) -> gtk::ListBox {
        self.imp().list_el.get()
    }

    /// `"loading"`, `"content"` or `"no-content"`.
    pub fn set_state(&self, name: &str) {
        self.imp().main_stack.set_visible_child_name(name);
    }

    /// All `SongRow`s (skips disc indicators and other rows).
    pub fn rows(&self) -> Vec<SongRow> {
        std::iter::successors(self.list().first_child(), |c| c.next_sibling()).filter_map(|w| w.downcast().ok()).collect()
    }

    pub fn all_ids(&self) -> Vec<String> {
        self.rows().iter().map(|r| r.id()).collect()
    }

    pub fn selected_rows(&self) -> Vec<SongRow> {
        self.rows().into_iter().filter(|r| r.is_checked()).collect()
    }

    pub fn clear(&self) {
        self.list().remove_all();
    }

    /// Replace the content with rows for `ids`; with `origin`, rows play the whole list.
    pub fn set_songs(&self, ids: &[String], origin: Option<&str>) {
        self.clear();
        self.append_songs(ids, ids, origin);
        self.set_state(if ids.is_empty() { "no-content" } else { "content" });
    }

    /// Append rows for `ids`; `all` is the list played by `app.play_song_from_list`.
    pub fn append_songs(&self, ids: &[String], all: &[String], origin: Option<&str>) {
        let removable = !self.playlist_id().is_empty();
        for id in ids {
            let row = SongRow::with_options(id, false, removable);
            if let Some(o) = origin {
                row.set_play_from_list(all, o);
            }
            self.list().append(&row);
        }
    }

    /// Mirror a `ListStore` of `Gtk.StringObject` ids (the play queue): rows are draggable and removable.
    pub fn bind_queue(&self, store: &gio::ListStore) {
        self.list().bind_model(Some(store), |o| {
            let id = o.downcast_ref::<gtk::StringObject>().map(|s| s.string().to_string()).unwrap_or_default();
            SongRow::with_options(&id, true, true).upcast()
        });
        self.set_state("content");
    }

    pub fn set_selected_mode(&self, select: bool, selected: Option<&SongRow>) {
        for r in self.rows() {
            r.set_select_mode(select, Some(&r) == selected);
        }
        let imp = self.imp();
        if select && let Some(s) = selected {
            imp.remove_el.set_visible(s.is_removable());
            for b in [&imp.play_el, &imp.play_next_el, &imp.play_later_el] {
                b.set_visible(!s.is_draggable());
            }
            imp.delete_download_el.set_visible(false);
            imp.download_el.set_visible(context::downloads_allowed());
        }
        imp.toolbar_revealer_el.set_reveal_child(select);
    }

    /// Remove rows: from the playlist (when `playlist-id` is set) or from the play queue.
    pub fn remove_rows(&self, rows: &[SongRow]) {
        let playlist = self.playlist_id();
        if !playlist.is_empty() {
            let indexes: Vec<String> = rows.iter().map(|r| r.index().to_string()).collect();
            let d = glib::VariantDict::new(None);
            d.insert("playlist", &playlist);
            d.insert("indexes", indexes);
            let _ = self.activate_action("app.remove_songs_from_playlist", Some(&d.end()));
            for r in rows {
                self.list().remove(r);
            }
            self.set_state(if self.rows().is_empty() { "no-content" } else { "content" });
        } else {
            let state = integration::get().current_state();
            let queue = state.queueModel();
            let all = models::string_list(&queue);
            let selected: Vec<String> = rows.iter().map(|r| r.id()).collect();
            if selected.contains(&state.songId()) {
                let next = all.iter().find(|s| !selected.contains(s)).cloned().unwrap_or_default();
                state.set_songId(next);
            }
            for (i, id) in all.iter().enumerate().rev() {
                if selected.contains(id) {
                    queue.remove(i as u32);
                }
            }
        }
        self.set_selected_mode(false, None);
    }
}
