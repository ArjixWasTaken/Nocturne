//! `NocturneSongDetailsDialog` (song/details_dialog.blp).
use crate::{integration, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/song/details_dialog.ui")]
    pub struct SongDetailsDialog {
        #[template_child]
        pub cover_el: TemplateChild<gtk::Picture>,
        #[template_child]
        pub title_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub listbox_el: TemplateChild<gtk::ListBox>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongDetailsDialog {
        const NAME: &'static str = "NocturneSongDetailsDialog";
        type Type = super::SongDetailsDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SongDetailsDialog {}
    impl WidgetImpl for SongDetailsDialog {}
    impl AdwDialogImpl for SongDetailsDialog {}
}

glib::wrapper! {
    pub struct SongDetailsDialog(ObjectSubclass<imp::SongDetailsDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SongDetailsDialog {
    pub fn new(id: &str) -> Self {
        let obj: Self = glib::Object::new();
        let song = integration::get().song(id);
        let imp = obj.imp();
        imp.title_el.set_label(&song.title());
        let p = song.gdkPaintableBig().or(song.gdkPaintable());
        imp.cover_el.set_visible(p.is_some());
        imp.cover_el.set_paintable(p.as_ref());
        let (w, id) = (obj.downgrade(), id.to_string());
        spawn_local(async move {
            let integ = integration::get();
            let i = id.clone();
            let rows = integ.call(move |b| async move { b.song_details(&i).await }).await.unwrap_or_else(|e| {
                tracing::warn!("song details: {e:#}");
                vec![]
            });
            let Some(o) = w.upgrade() else { return };
            let song = integ.song(&id);
            for (k, v) in rows {
                let row = adw::ActionRow::builder().title(&k).subtitle(&v).css_classes(["property"]).use_markup(false).build();
                let link = match k.as_str() {
                    "Album" if !song.albumId().is_empty() => Some(("app.show_album", song.albumId())),
                    "Display Artist" | "Artist" if !song.artistId().is_empty() => Some(("app.show_artist", song.artistId())),
                    "MusicBrainz ID" => Some(("app.visit_url", format!("https://musicbrainz.org/recording/{v}"))),
                    _ => None,
                };
                if let Some((action, target)) = link {
                    row.set_action_name(Some(action));
                    row.set_action_target_value(Some(&target.to_variant()));
                    row.set_activatable(true);
                }
                o.imp().listbox_el.append(&row);
            }
        });
        obj
    }
}
