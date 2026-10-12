//! `NocturnePlaylistDialog` (playlist/dialog.blp): pick (or create) a playlist to add songs to.
use super::PlaylistSelectorRow;
use crate::{integration, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/playlist/dialog.ui")]
    pub struct PlaylistDialog {
        #[template_child]
        pub search_entry_el: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub preferences_group_el: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub list_el: TemplateChild<gtk::ListBox>,
        /// `Adw.ButtonRow` (1.6)
        #[template_child]
        pub add_button_el: TemplateChild<adw::PreferencesRow>,
        pub songs: RefCell<Vec<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistDialog {
        const NAME: &'static str = "NocturnePlaylistDialog";
        type Type = super::PlaylistDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl PlaylistDialog {
        #[template_callback]
        pub fn search_changed(&self) {
            let query = self.search_entry_el.text().to_string();
            let q = query.to_lowercase();
            let rows: Vec<gtk::Widget> = std::iter::successors(self.list_el.first_child(), |c| c.next_sibling()).skip(1).collect();
            let mut exact = false;
            for r in &rows {
                let name = r.widget_name().to_lowercase();
                r.set_visible(name.contains(&q));
                exact |= name == q;
            }
            self.add_button_el.set_visible((!exact && !query.is_empty()) || rows.is_empty());
            let d = glib::VariantDict::new(None);
            d.insert("new_playlist", &query);
            d.insert("songs", self.songs.borrow().clone());
            self.add_button_el.set_action_target_value(Some(&d.end()));
            self.add_button_el.set_action_name(Some("app.add_songs_to_playlist"));
            self.add_button_el.set_title(if query.is_empty() { "New Playlist" } else { &query });
        }
    }

    impl ObjectImpl for PlaylistDialog {}
    impl WidgetImpl for PlaylistDialog {}
    impl AdwDialogImpl for PlaylistDialog {}
}

glib::wrapper! {
    pub struct PlaylistDialog(ObjectSubclass<imp::PlaylistDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PlaylistDialog {
    /// Selector for adding song `id` (`None`: no songs, i.e. just create a playlist).
    pub fn new(id: Option<&str>) -> Self {
        Self::with_songs(id.map(|i| vec![i.to_string()]).unwrap_or_default())
    }

    /// Port of `PlaylistDialog(song_list)`.
    pub fn with_songs(songs: Vec<String>) -> Self {
        let obj: Self = glib::Object::new();
        let imp = obj.imp();
        let integ = integration::get();
        match songs.len() {
            0 => {}
            1 => {
                let s = integ.song(&songs[0]);
                s.bind_property("title", &*imp.preferences_group_el, "description").sync_create().build();
                let id = songs[0].clone();
                spawn_local(async move {
                    integration::get().verify_song(&id, true).await;
                });
            }
            n => imp.preferences_group_el.set_description(Some(&format!("{n} Songs"))),
        }
        imp.songs.replace(songs);
        imp.search_changed();
        let w = obj.downgrade();
        spawn_local(async move {
            let ids = integration::get().playlists().await;
            let Some(o) = w.upgrade() else { return };
            let imp = o.imp();
            for id in ids {
                let row = PlaylistSelectorRow::new(&id);
                let d = glib::VariantDict::new(None);
                d.insert("playlist", &id);
                d.insert("songs", imp.songs.borrow().clone());
                row.set_action_target_value(Some(&d.end()));
                imp.list_el.append(&row);
            }
            imp.search_changed();
        });
        obj
    }
}

/// Ask which playlist to add `songs` to.
pub fn prompt_add_songs(parent: &impl IsA<gtk::Widget>, songs: Vec<String>) {
    PlaylistDialog::with_songs(songs).present(Some(parent));
}
