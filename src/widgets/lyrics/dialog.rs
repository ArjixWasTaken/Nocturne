//! `NocturneLyricsDialog` (lyrics/dialog.blp): LRC editor for the current song.
use super::LyricEditRow;
use crate::backend::{Lyrics, LyricsKind};
use crate::{application::Application, config, integration, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/lyrics/dialog.ui")]
    pub struct LyricsDialog {
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub lrc_list_el: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub progress_el: TemplateChild<gtk::Scale>,
        #[template_child]
        pub state_stack_el: TemplateChild<gtk::Stack>,
        #[template_child]
        pub position_spin: TemplateChild<gtk::SpinButton>,
        pub id: RefCell<String>,
        pub seeking: Cell<bool>,
        pub focused: RefCell<Option<LyricEditRow>>,
        pub playback_mode_backup: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LyricsDialog {
        const NAME: &'static str = "NocturneLyricsDialog";
        type Type = super::LyricsDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    fn seek(secs: f64) {
        if let Some(p) = Application::get().player() {
            p.seek(secs);
        }
    }

    #[gtk::template_callbacks]
    impl LyricsDialog {
        #[template_callback]
        fn seek_start(&self) {
            self.seeking.set(true);
        }
        #[template_callback]
        fn seek_end(&self) {
            self.seeking.set(false);
        }
        #[template_callback]
        fn progress_bar_changed(&self, adj: &gtk::Adjustment) {
            if self.seeking.get() {
                seek(adj.value());
            }
        }
        #[template_callback]
        fn position_spin_changed(&self, spin: &gtk::SpinButton) -> bool {
            if spin.is_sensitive() && spin.root().is_some() {
                seek(spin.value());
            }
            false
        }
        #[template_callback]
        fn state_stack_changed(&self, _pspec: glib::ParamSpec, stack: &gtk::Stack) {
            self.position_spin.set_sensitive(stack.visible_child_name().as_deref() == Some("play"));
        }
        #[template_callback]
        fn add_line(&self) {
            let ps = integration::get().current_state().positionSeconds();
            self.lrc_list_el.append(&LyricEditRow::new((ps * 1000.0) as i64, "", false));
            self.lrc_list_el.invalidate_sort();
            self.obj().update_visibility();
        }
        #[template_callback]
        fn cancel_clicked(&self) {
            self.obj().close();
        }
        #[template_callback]
        fn save_clicked(&self) {
            let obj = self.obj();
            let content = obj
                .rows()
                .iter()
                .map(|r| if r.is_invalid() { r.text().to_string() } else { format!("[{}] {}", super::super::edit_row::format_ms(r.ms()), r.text()) })
                .collect::<Vec<_>>()
                .join("\n");
            let id = self.id.borrow().clone();
            spawn_local(async move {
                integration::get().save_lyrics(&id, &Lyrics { kind: LyricsKind::Synced, content }).await;
            });
            obj.close();
        }
        #[template_callback]
        fn set_next_timestamp(&self) {
            let rows = self.obj().rows();
            let focused = self.focused.borrow().clone();
            let next = focused.and_then(|f| rows.iter().position(|r| *r == f)).map(|i| i + 1).unwrap_or(0);
            if let Some(r) = rows.get(next) {
                r.set_current_timestamp();
            }
        }
    }

    impl ObjectImpl for LyricsDialog {}
    impl WidgetImpl for LyricsDialog {}
    impl AdwDialogImpl for LyricsDialog {
        fn closed(&self) {
            let _ = config::settings().set_string("playback-mode", &self.playback_mode_backup.borrow());
            self.parent_closed();
        }
    }
}

glib::wrapper! {
    pub struct LyricsDialog(ObjectSubclass<imp::LyricsDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl LyricsDialog {
    /// Lyrics editor for the current song.
    pub fn new() -> Self {
        let obj: Self = glib::Object::new();
        let imp = obj.imp();
        let integ = integration::get();
        let state = integ.current_state();
        let id = state.songId();
        imp.id.replace(id.clone());
        let song = integ.song(&id);
        song.bind_property("title", &obj, "title").sync_create().build();
        for adj in [imp.progress_el.adjustment(), imp.position_spin.adjustment()] {
            song.bind_property("duration", &adj, "upper").transform_to(|_, d: i32| Some(d as f64)).sync_create().build();
        }
        state.bind_property("positionSeconds", &imp.progress_el.adjustment(), "value").sync_create().build();
        state.bind_property("buttonState", &*imp.state_stack_el, "visible-child-name").sync_create().build();
        let w = obj.downgrade();
        state.connect_notify_local(Some("positionSeconds"), move |s, _| {
            if let Some(o) = w.upgrade() {
                o.position_changed(s.positionSeconds());
            }
        });
        imp.lrc_list_el.set_sort_func(|a, b| {
            let ms = |r: &gtk::ListBoxRow| r.downcast_ref::<LyricEditRow>().map(|r| r.ms()).unwrap_or(0);
            ms(a).cmp(&ms(b)).into()
        });

        let settings = config::settings();
        imp.playback_mode_backup.replace(settings.string("playback-mode").into());
        let _ = settings.set_string("playback-mode", "repeat-one");

        imp.main_stack.set_visible_child_name("loading");
        let w = obj.downgrade();
        spawn_local(async move {
            let lyrics = integration::get().lyrics(&id, true).await;
            let Some(o) = w.upgrade() else { return };
            let list = &o.imp().lrc_list_el;
            match lyrics {
                Some(Lyrics { kind: LyricsKind::Synced, content }) => {
                    for (secs, line) in crate::lyrics::parse_lrc(&content) {
                        list.append(&LyricEditRow::new((secs * 1000.0).round() as i64, line.trim(), false));
                    }
                }
                Some(Lyrics { content, .. }) => {
                    for (i, line) in content.lines().enumerate() {
                        list.append(&LyricEditRow::new(i as i64 * 10_000_000, line, true));
                    }
                }
                None => {}
            }
            list.invalidate_sort();
            o.update_visibility();
        });
        obj
    }

    fn rows(&self) -> Vec<LyricEditRow> {
        std::iter::successors(self.imp().lrc_list_el.first_child(), |c| c.next_sibling()).filter_map(|w| w.downcast().ok()).collect()
    }

    pub fn update_visibility(&self) {
        let has = self.imp().lrc_list_el.first_child().is_some();
        self.imp().main_stack.set_visible_child_name(if has { "content" } else { "empty" });
    }

    /// Port of `format_position`: follow playback in the spin button and highlight the current line.
    fn position_changed(&self, secs: f64) {
        let imp = self.imp();
        if !imp.position_spin.has_focus() {
            imp.position_spin.set_value(secs);
        }
        let rows = self.rows();
        let Some(first) = rows.first() else { return };
        let ms = (secs * 1000.0) as i64 + 100;
        let best = rows.iter().take_while(|r| r.ms() <= ms).last().unwrap_or(first).clone();
        if imp.focused.borrow().as_ref() != Some(&best) {
            if let Some(f) = imp.focused.borrow().as_ref() {
                f.ts_button().remove_css_class("suggested-action");
            }
            best.ts_button().add_css_class("suggested-action");
            imp.focused.replace(Some(best));
        }
    }
}

impl Default for LyricsDialog {
    fn default() -> Self {
        Self::new()
    }
}
