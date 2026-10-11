//! `NocturnePlayingLyricsPage` (playing/lyrics_page.blp): plain and synced lyrics.
use super::{on_login, watch};
use crate::{application::Application, backend::{Lyrics, LyricsKind}, config::settings, integration, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};
use std::{cell::{Cell, RefCell}, rc::Rc};

/// One synced line with per-word cues that fade in as the song progresses.
pub struct LyricLine {
    ms: i64,
    label: gtk::Label,
    cues: Vec<(i64, String)>,
    alphas: RefCell<Vec<f64>>,
    last_ms: Cell<i64>,
}

impl LyricLine {
    fn new(content: &str, ms: i64, duration_ms: i64) -> Rc<Self> {
        let mut cues = word_cues(content);
        if cues.is_empty() {
            if content.is_empty() {
                cues.push((ms, if duration_ms > 1000 { "🎵".into() } else { String::new() }));
            } else if duration_ms == 0 {
                cues.push((ms, content.into()));
            } else {
                let words: Vec<&str> = content.split(' ').collect();
                let weights: Vec<i64> = words
                    .iter()
                    .map(|w| w.replace('.', "...").replace(',', "..").chars().filter(|c| !"¡!¿?'\"".contains(*c)).count() as i64)
                    .collect();
                let per_letter = duration_ms / weights.iter().sum::<i64>().max(1);
                let mut ts = ms;
                for (i, w) in words.iter().enumerate() {
                    if i > 0 {
                        ts += weights[i - 1] * per_letter;
                    }
                    cues.push((ts, format!("{w} ")));
                }
            }
        }
        cues.sort_by_key(|c| c.0);
        let label = gtk::Label::builder()
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .justify(gtk::Justification::Center)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        let line = Rc::new(Self { ms, label, alphas: RefCell::new(vec![0.3; cues.len()]), cues, last_ms: Cell::new(0) });
        line.render(true);
        let weak = Rc::downgrade(&line);
        line.label.add_tick_callback(move |_, _| match weak.upgrade() {
            Some(l) => {
                l.render(false);
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
        line
    }

    fn render(&self, force: bool) {
        let mut alphas = self.alphas.borrow_mut();
        let mut changed = false;
        let mut markup = String::new();
        for ((ts, word), a) in self.cues.iter().zip(alphas.iter_mut()) {
            let target = if self.last_ms.get() >= *ts { 1.0 } else { 0.3 };
            if (*a - target).abs() > 0.001 {
                *a += (target - *a) * 0.1;
                changed = true;
            }
            markup.push_str(&format!("<span fgalpha='{}'>{}</span>", (*a * 65535.0) as u32, glib::markup_escape_text(word)));
        }
        if changed || force {
            self.label.set_markup(&markup);
        }
    }
}

/// Parse enhanced-LRC `<mm:ss.xx> word` cues.
fn word_cues(content: &str) -> Vec<(i64, String)> {
    content
        .split('<')
        .skip(1)
        .filter_map(|part| {
            let (ts, text) = part.split_once('>')?;
            let (m, rest) = ts.split_once(':')?;
            let (s, frac) = rest.split_once('.')?;
            let mut f: i64 = frac.parse().ok()?;
            if frac.len() == 2 {
                f *= 10;
            }
            let text = text.trim_start();
            let ms = m.parse::<i64>().ok()? * 60000 + s.parse::<i64>().ok()? * 1000 + f;
            (!text.is_empty()).then(|| (ms, text.to_string()))
        })
        .collect()
}

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/playing/lyrics_page.ui")]
    pub struct PlayingLyricsPage {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub plain_label_el: TemplateChild<gtk::Label>,
        #[template_child]
        pub lrc_list_el: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub scrolledwindow: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub lrc_menu_popover: TemplateChild<gtk::Popover>,
        pub lines: RefCell<Vec<Rc<LyricLine>>>,
        pub code_is_selecting: Cell<bool>,
        pub last_best: Cell<Option<usize>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlayingLyricsPage {
        const NAME: &'static str = "NocturnePlayingLyricsPage";
        type Type = super::PlayingLyricsPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlayingLyricsPage {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let edit = gtk::Button::builder().css_classes(["flat"]).action_name("app.edit_lyrics")
                .child(&adw::ButtonContent::builder().icon_name("document-edit-symbolic").label("Edit Lyrics").build()).build();
            let remove = gtk::Button::builder().css_classes(["flat", "error"])
                .child(&adw::ButtonContent::builder().icon_name("user-trash-symbolic").label("Remove Lyrics").build()).build();
            remove.connect_clicked(glib::clone!(#[weak] obj, move |_| obj.go_to_main()));
            let popover = self.lrc_menu_popover.get();
            for b in [&edit, &remove] {
                b.connect_clicked(glib::clone!(#[weak] popover, move |_| popover.popdown()));
                menu.append(b);
            }
            popover.set_child(Some(&menu));
            on_login(&*obj, |o, state| {
                watch(state, o, "songId", |o, s| o.song_changed(s.songId().into(), false));
                watch(state, o, "positionSeconds", |o, s| o.position_changed(s.positionSeconds()));
            });
        }
    }
    impl WidgetImpl for PlayingLyricsPage {}
    impl BinImpl for PlayingLyricsPage {}
}

glib::wrapper! {
    pub struct PlayingLyricsPage(ObjectSubclass<imp::PlayingLyricsPage>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl PlayingLyricsPage {
    /// The inner stack (`loading`, `plain`, `lrc`, `not-found`, `instrumental`, `radio`, `not-found-locally`).
    pub fn stack(&self) -> gtk::Stack {
        self.imp().stack.get()
    }

    fn current_id() -> String {
        integration::current().map(|i| i.current_state().songId().to_string()).unwrap_or_default()
    }

    fn song_changed(&self, id: String, online: bool) {
        self.imp().stack.set_visible_child_name("loading");
        let obj = self.downgrade();
        spawn_local(async move {
            let Some(integ) = integration::current() else { return };
            if id.is_empty() || Self::current_id() != id {
                return;
            }
            if !integ.song(&id).radioStreamUrl().is_empty() {
                if let Some(o) = obj.upgrade() {
                    o.imp().stack.set_visible_child_name("radio");
                }
                return;
            }
            let lyrics = integ.lyrics(&id, online).await;
            if Self::current_id() != id {
                return;
            }
            if lyrics.is_none() && !online && settings().boolean("auto-download-lyrics") {
                glib::timeout_add_local_once(std::time::Duration::from_secs(3), glib::clone!(#[strong] obj, move || {
                    if let Some(o) = obj.upgrade() {
                        o.song_changed(id, true);
                    }
                }));
                return;
            }
            if let Some(o) = obj.upgrade() {
                o.show_lyrics(&id, lyrics, online);
            }
        });
    }

    fn show_lyrics(&self, id: &str, lyrics: Option<Lyrics>, online: bool) {
        let imp = self.imp();
        let Some(l) = lyrics else {
            imp.stack.set_visible_child_name(if online { "not-found" } else { "not-found-locally" });
            return;
        };
        if l.kind == LyricsKind::Plain {
            imp.plain_label_el.set_label(&l.content);
            imp.stack.set_visible_child_name("plain");
            return;
        }
        let mut content: Vec<(i64, String)> = crate::lyrics::parse_lrc(&l.content).into_iter().map(|(s, t)| ((s * 1000.0) as i64, t.trim().to_string())).collect();
        if content.is_empty() {
            imp.stack.set_visible_child_name("not-found");
            return;
        }
        if !content[0].1.is_empty() {
            content.insert(0, (0, String::new()));
        }
        let simulate = settings().boolean("simulate-word-by-word-lyrics");
        let song_ms = integration::current().map(|i| i.song(id).duration() as i64 * 1000).unwrap_or(0);
        imp.lrc_list_el.remove_all();
        imp.last_best.set(None);
        let lines: Vec<Rc<LyricLine>> = content
            .iter()
            .enumerate()
            .map(|(i, (ms, text))| {
                let duration = if !simulate { 0 } else { content.get(i + 1).map_or(song_ms - ms, |n| n.0 - ms) };
                let line = LyricLine::new(text, *ms, duration);
                imp.lrc_list_el.append(&gtk::ListBoxRow::builder().child(&line.label).build());
                line
            })
            .collect();
        imp.lines.replace(lines);
        imp.stack.set_visible_child_name("lrc");
    }

    fn position_changed(&self, pos: f64) {
        let imp = self.imp();
        if imp.stack.visible_child_name().as_deref() != Some("lrc") {
            return;
        }
        let ms = (pos * 1000.0) as i64 + 100;
        let lines = imp.lines.borrow();
        let Some(best) = lines.iter().rposition(|l| l.ms <= ms) else { return };
        let Some(row) = imp.lrc_list_el.row_at_index(best as i32) else { return };
        if imp.lrc_list_el.selected_row().as_ref() == Some(&row) {
            lines[best].last_ms.set(ms);
        } else {
            drop(lines);
            imp.code_is_selecting.set(true);
            imp.lrc_list_el.select_row(Some(&row));
            imp.code_is_selecting.set(false);
        }
    }

    #[template_callback]
    fn on_lrc_selection(&self, _list: &gtk::ListBox, row: Option<gtk::ListBoxRow>) {
        let imp = self.imp();
        let lines = imp.lines.borrow();
        if let Some(prev) = imp.last_best.get().and_then(|i| lines.get(i)) {
            prev.last_ms.set(-1);
        }
        imp.last_best.set(row.as_ref().map(|r| r.index() as usize));
        let Some(row) = row else { return };
        let row = &row;
        if !imp.code_is_selecting.get()
            && let (Some(line), Some(p)) = (lines.get(row.index() as usize), Application::get().player())
        {
            p.seek(line.ms as f64 / 1000.0);
        }
        let sw = &imp.scrolledwindow;
        let vadj = sw.vadjustment();
        let y = row.compute_bounds(&*imp.lrc_list_el).map_or(0.0, |b| b.y() as f64);
        let target = y - sw.height() as f64 / 2.0 + row.height() as f64 / 2.0;
        vadj.set_value(target.clamp(vadj.lower(), (vadj.upper() - vadj.page_size()).max(vadj.lower())));
    }

    #[template_callback]
    fn lyric_download_requested(&self, _: &gtk::Button) {
        self.song_changed(Self::current_id(), true);
    }

    #[template_callback]
    fn lyric_load_requested(&self, _: &gtk::Button) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("LRC File"));
        filter.add_mime_type("text/x-lrc");
        filter.add_mime_type("application/x-lrc");
        filter.add_suffix("lrc");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder().filters(&filters).build();
        let window = self.root().and_downcast::<gtk::Window>();
        let obj = self.downgrade();
        dialog.open(window.as_ref(), gio::Cancellable::NONE, move |res| {
            let Ok(file) = res else { return };
            let id = Self::current_id();
            spawn_local(async move {
                let Ok((bytes, _)) = file.load_contents_future().await else { return };
                let content = String::from_utf8_lossy(&bytes).to_string();
                if id.is_empty() || content.is_empty() {
                    return;
                }
                integration::get().save_lyrics(&id, &Lyrics { kind: LyricsKind::Synced, content }).await;
                if let Some(o) = obj.upgrade() {
                    o.song_changed(id, false);
                }
            });
        });
    }

    #[template_callback(name = "go_to_main")]
    fn go_to_main_clicked(&self, _: &gtk::Button) {
        self.go_to_main();
    }

    fn go_to_main(&self) {
        self.imp().stack.set_visible_child_name("not-found-locally");
        let id = Self::current_id();
        if let Some(integ) = integration::current() {
            spawn_local(async move { integ.delete_lyrics(&id).await });
        }
    }
}
