//! `NocturneSongRow` (song/row.blp).
use super::SongQueue;
use crate::widgets::containers::{context::{self, Formatters, ItemsExt}, wrapbox::wrap_box_append};
use crate::{config, integration, models::Song, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gdk, glib};
use std::{cell::{Cell, RefCell}, rc::Rc};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/jeffser/Nocturne/song/row.ui")]
    #[properties(wrapper_type = super::SongRow)]
    pub struct SongRow {
        #[template_child]
        pub icon_el: TemplateChild<gtk::Image>,
        #[template_child]
        pub title_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub duration_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub artist_container_el: TemplateChild<adw::Bin>,
        #[template_child]
        pub external_file_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub suffixes_stack_el: TemplateChild<gtk::Stack>,
        #[template_child]
        pub star_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub check_el: TemplateChild<gtk::CheckButton>,
        #[template_child]
        pub menu_button_el: TemplateChild<gtk::MenuButton>,
        #[property(get, set)]
        pub model: RefCell<Option<Song>>,
        pub draggable: Cell<bool>,
        pub removable: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongRow {
        const NAME: &'static str = "NocturneSongRow";
        type Type = super::SongRow;
        type ParentType = adw::ActionRow;

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
    impl SongRow {
        #[template_callback(function)]
        fn format_starred_visibility(is_external: bool, radio: Option<String>) -> bool {
            !is_external && radio.unwrap_or_default().is_empty()
        }
        #[template_callback]
        fn on_drop(&self, _target: &gtk::DropTarget, value: glib::Value, _x: f64, y: f64) -> bool {
            let obj = self.obj();
            let Ok(row) = value.get::<super::SongRow>() else { return false };
            if row == *obj || !self.draggable.get() {
                return false;
            }
            let src = row.index();
            let mut dst = obj.index() + i32::from(y > obj.height() as f64 / 2.0);
            if src < dst {
                dst -= 1;
            }
            let queue = integration::get().current_state().queueModel();
            queue.remove(src as u32);
            queue.insert(dst.max(0) as u32, &gtk::StringObject::new(&obj.id()));
            true
        }
        #[template_callback]
        fn on_drag_begin(&self, source: &gtk::DragSource, _drag: &gdk::Drag) {
            if self.draggable.get() {
                source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&*self.obj()))), 0, 0);
            }
        }
        #[template_callback]
        fn on_drag_prepare(&self, _source: &gtk::DragSource, _x: f64, _y: f64) -> Option<gdk::ContentProvider> {
            self.draggable.get().then(|| gdk::ContentProvider::for_value(&self.obj().to_value()))
        }
        #[template_callback]
        fn check_toggled(&self, check: &gtk::CheckButton) {
            if !check.is_active()
                && let Some(q) = self.obj().queue()
                && q.selected_rows().is_empty()
            {
                q.set_selected_mode(false, None);
            }
        }
        #[template_callback]
        fn on_context_button_active(&self, button: &gtk::MenuButton) {
            if let Some(p) = button.popover() {
                p.set_child(Some(&context::container(&self.obj().context_items(), &self.obj().id())));
            }
        }
        #[template_callback]
        fn show_popover(&self, #[rest] args: &[glib::Value]) {
            let (_, x, y) = context::gesture_point(args);
            let obj = self.obj();
            context::popup_items(&*obj, &obj.context_items(), &obj.id(), x, y);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SongRow {}
    impl WidgetImpl for SongRow {}
    impl ListBoxRowImpl for SongRow {}
    impl PreferencesRowImpl for SongRow {}
    impl ActionRowImpl for SongRow {}
}

glib::wrapper! {
    pub struct SongRow(ObjectSubclass<imp::SongRow>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

fn link(label: &str, action: &str, target: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .action_name(action)
        .action_target(&target.to_variant())
        .child(&gtk::Label::builder().ellipsize(gtk::pango::EllipsizeMode::End).label(label).css_classes(["subtitle"]).build())
        .css_classes(["p0", "flat"])
        .tooltip_text(tooltip)
        .build()
}

impl SongRow {
    pub fn new(id: &str) -> Self {
        Self::with_options(id, false, false)
    }

    /// `draggable`: reorderable row of the play queue; `removable`: offers "Remove".
    pub fn with_options(id: &str, draggable: bool, removable: bool) -> Self {
        let integ = integration::get();
        let model = integ.song(id);
        let obj: Self = glib::Object::builder().property("model", &model).build();
        let imp = obj.imp();
        imp.draggable.set(draggable);
        imp.removable.set(removable);
        config::settings().bind("show-context-button", &*imp.menu_button_el, "visible").build();
        context::track_star(&*imp.star_el, &model);

        let w = obj.downgrade();
        model.connect_notify_local(Some("artists"), move |_, _| {
            if let Some(o) = w.upgrade() {
                o.update_artists();
            }
        });
        let w = obj.downgrade();
        model.connect_notify_local(Some("radioStreamUrl"), move |_, _| {
            if let Some(o) = w.upgrade() {
                o.update_artists();
            }
        });
        let w = obj.downgrade();
        model.connect_notify_local(Some("deleted"), move |m, _| {
            if let Some(o) = w.upgrade()
                && m.deleted()
                && let Some(lb) = o.parent().and_downcast::<gtk::ListBox>()
            {
                lb.remove(&o);
            }
        });
        let w = obj.downgrade();
        let state = integ.current_state();
        state.connect_notify_local(Some("songId"), move |s, _| {
            if let Some(o) = w.upgrade() {
                o.current_song_changed(&s.songId());
            }
        });
        obj.update_artists();
        obj.current_song_changed(&state.songId());

        let id = id.to_string();
        spawn_local(async move {
            let i = integration::get();
            i.verify_song(&id, false).await;
            i.load_cover(&id).await;
        });
        obj
    }

    pub fn id(&self) -> String {
        self.model().map(|m| m.id()).unwrap_or_default()
    }
    pub fn is_draggable(&self) -> bool {
        self.imp().draggable.get()
    }
    pub fn is_removable(&self) -> bool {
        self.imp().removable.get()
    }
    pub fn is_checked(&self) -> bool {
        self.imp().check_el.is_active()
    }
    pub fn set_checked(&self, v: bool) {
        self.imp().check_el.set_active(v);
    }

    /// Make the row play `songs` (starting at itself) through `app.play_song_from_list`.
    pub fn set_play_from_list(&self, songs: &[String], origin: &str) {
        self.set_action_name(None);
        self.set_action_target_value(Some(&context::play_from_list_target(&self.id(), songs, origin)));
        self.set_action_name(Some("app.play_song_from_list"));
    }

    /// Switch between the normal suffixes and the selection check box.
    pub fn set_select_mode(&self, select: bool, checked: bool) {
        let imp = self.imp();
        imp.suffixes_stack_el.set_visible_child_name(if select { "select" } else { "normal" });
        imp.check_el.set_active(checked);
        self.set_activatable(!select && self.id() != integration::get().current_state().songId());
    }

    fn queue(&self) -> Option<SongQueue> {
        self.ancestor(SongQueue::static_type()).and_downcast()
    }

    fn current_song_changed(&self, song_id: &str) {
        let imp = self.imp();
        let current = song_id == self.id();
        self.set_activatable(!current);
        if current {
            imp.icon_el.set_icon_name(Some("sound-symbolic"));
            imp.icon_el.set_visible(true);
            self.add_css_class("accent");
        } else {
            self.remove_css_class("accent");
            imp.icon_el.set_icon_name(self.is_draggable().then_some("list-drag-handle-symbolic"));
            imp.icon_el.set_visible(self.is_draggable());
        }
    }

    fn update_artists(&self) {
        let Some(m) = self.model() else { return };
        let bin = &self.imp().artist_container_el;
        let radio = m.radioStreamUrl();
        if !radio.is_empty() {
            if let Ok(u) = url::Url::parse(&radio) {
                let host = u.host_str().unwrap_or_default().to_string();
                let home = format!("{}://{}", u.scheme(), host);
                bin.set_child(Some(&link(&host, "app.visit_url", &home, &home)));
            }
            bin.set_visible(true);
            return;
        }
        let artists = m.artists().0;
        let child: Option<gtk::Widget> = match artists.len() {
            0 => None,
            1 => Some(link(&artists[0].name, "app.show_artist", &artists[0].id, &artists[0].name).upcast()),
            n if n >= 5 => {
                let menu = gtk::gio::Menu::new();
                for a in &artists {
                    let item = gtk::gio::MenuItem::new(Some(&a.name), None);
                    if !m.isExternalFile() {
                        item.set_action_and_target_value(Some("app.show_artist"), Some(&a.id.to_variant()));
                    }
                    menu.append_item(&item);
                }
                let label = gtk::Label::builder().ellipsize(gtk::pango::EllipsizeMode::End).label("Multiple Artists").css_classes(["subtitle"]).build();
                Some(gtk::MenuButton::builder().child(&label).css_classes(["p0", "flat"]).menu_model(&menu).tooltip_text("Multiple Artists").build().upcast())
            }
            _ => {
                let container: gtk::Widget = glib::Type::from_name("AdwWrapBox")
                    .map(|t| glib::Object::builder_with_type(t).property("line-spacing", 2i32).property("child-spacing", 2i32).build().downcast().unwrap())
                    .unwrap_or_else(|| gtk::Box::new(gtk::Orientation::Horizontal, 2).upcast());
                for a in &artists {
                    wrap_box_append(&container, &link(&a.name, "app.show_artist", &a.id, &a.name));
                }
                Some(container)
            }
        };
        bin.set_sensitive(!m.isExternalFile());
        bin.set_visible(child.is_some());
        bin.set_child(child.as_ref());
    }

    fn context_items(&self) -> Vec<context::Item> {
        let id = self.id();
        let m = integration::get().song(&id);
        let radio = !m.radioStreamUrl().is_empty();
        let draggable = self.is_draggable();
        let mut v = context::song_items(&id);
        let w = self.downgrade();
        if let Some(i) = v.get_mut("select") {
            i.callback = Some(Rc::new(move || {
                if let Some(o) = w.upgrade()
                    && let Some(q) = o.queue()
                {
                    q.set_selected_mode(true, Some(&o));
                }
            }));
        }
        if !(radio && !draggable) {
            v = v.without(&["edit-radio", "delete-radio"]);
        }
        if integration::get().kind() == crate::backend::jellyfin::KIND {
            v = v.without(&["edit-radio"]);
        }
        if radio {
            v = v.without(&["download"]);
        }
        if radio || m.isExternalFile() {
            v = v.without(&["add-to-playlist", "show-album", "show-artist"]);
            if !draggable {
                v = v.without(&["select"]);
            }
        }
        if self.is_removable() {
            let w = self.downgrade();
            if let Some(i) = v.get_mut("remove") {
                i.callback = Some(Rc::new(move || {
                    if let Some(o) = w.upgrade() {
                        o.remove_self();
                    }
                }));
            }
        } else {
            v = v.without(&["remove"]);
        }
        v
    }

    /// Port of `remove_selected` for a single row.
    fn remove_self(&self) {
        let Some(q) = self.queue() else { return };
        self.set_checked(true);
        q.remove_rows(&[self.clone()]);
    }
}
