//! `NocturnePopoutWindow` (playing/popout_window.blp): detached player window.
use super::{PlayingControlPage, PlayingFooter};
use crate::config::settings;
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/playing/popout_window.ui")]
    pub struct PopoutWindow {
        #[template_child]
        pub playing_page: TemplateChild<PlayingControlPage>,
        #[template_child]
        pub footer: TemplateChild<PlayingFooter>,
        pub fullscreen_btn: RefCell<Option<gtk::Button>>,
        pub idle_cookie: Cell<Option<u32>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PopoutWindow {
        const NAME: &'static str = "NocturnePopoutWindow";
        type Type = super::PopoutWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PopoutWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let page = &self.playing_page;
            let header = page.header_bar();
            if let Some(tv) = header.ancestor(adw::ToolbarView::static_type()).and_downcast::<adw::ToolbarView>() {
                tv.set_extend_content_to_top_edge(false);
            }
            header.set_show_start_title_buttons(true);
            header.set_show_end_title_buttons(true);
            self.footer.set_forceHugeMode(true);

            let btn = gtk::Button::builder().icon_name("view-fullscreen-symbolic").tooltip_text("Toggle Fullscreen").build();
            btn.connect_clicked(glib::clone!(#[weak] obj, move |_| if obj.is_fullscreen() { obj.unfullscreen() } else { obj.fullscreen() }));
            header.pack_start(&btn);
            self.fullscreen_btn.replace(Some(btn));

            let s = settings();
            s.connect_changed(Some("popout-dynamic-bg-mode"), glib::clone!(#[weak] obj, move |_, _| obj.dynamic_bg_mode_changed()));
            obj.dynamic_bg_mode_changed();
            s.connect_changed(Some("use-dynamic-accent"), glib::clone!(#[weak] obj, move |s, k| obj.set_css_class("dynamic-accent", s.boolean(k))));
            obj.set_css_class("dynamic-accent", s.boolean("use-dynamic-accent"));

            let clamp = page.clamp_el();
            page.cover_art().view_stack().connect_visible_child_name_notify(move |vs| {
                clamp.set_maximum_size(if vs.visible_child_name().as_deref() == Some("video") { 1920 } else { 720 });
            });
        }
    }
    impl WidgetImpl for PopoutWindow {}
    impl WindowImpl for PopoutWindow {}
    impl ApplicationWindowImpl for PopoutWindow {}
    impl AdwApplicationWindowImpl for PopoutWindow {}
}

glib::wrapper! {
    pub struct PopoutWindow(ObjectSubclass<imp::PopoutWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PopoutWindow {
    pub fn new(app: &crate::application::Application, fullscreen: bool) -> Self {
        glib::Object::builder().property("application", app).property("fullscreened", fullscreen).build()
    }

    fn set_css_class(&self, class: &str, on: bool) {
        if on { self.add_css_class(class) } else { self.remove_css_class(class) }
    }

    fn dynamic_bg_mode_changed(&self) {
        self.remove_css_class("dynamic-bg-gradient");
        self.remove_css_class("dynamic-bg-blur");
        let v = settings().string("popout-dynamic-bg-mode");
        if !v.is_empty() {
            self.add_css_class(&format!("dynamic-bg-{v}"));
        }
    }

    fn set_idle_inhibit(&self, on: bool) {
        let Some(app) = self.application() else { return };
        let imp = self.imp();
        match (on, imp.idle_cookie.get()) {
            (true, None) => imp.idle_cookie.set(Some(app.inhibit(Some(self), gtk::ApplicationInhibitFlags::IDLE, Some("Fullscreen player")))),
            (false, Some(c)) => {
                app.uninhibit(c);
                imp.idle_cookie.set(None);
            }
            _ => {}
        }
    }

    #[template_callback]
    fn close_request(&self, _: &Self) -> bool {
        self.set_idle_inhibit(false);
        if let Some(app) = self.application().and_downcast::<crate::application::Application>() {
            let main = app.main_window();
            if main.hides_on_close() {
                main.present();
            }
        }
        false
    }

    #[template_callback]
    fn fullscreen_toggled(&self, _: &Self, _: glib::ParamSpec) {
        let fs = self.is_fullscreen();
        if let Some(b) = self.imp().fullscreen_btn.borrow().as_ref() {
            b.set_icon_name(if fs { "view-unfullscreen-symbolic" } else { "view-fullscreen-symbolic" });
        }
        self.set_idle_inhibit(fs);
    }

    #[template_callback]
    fn big_mode_apply(&self, _: &adw::Breakpoint) {
        self.add_css_class("big-mode");
    }

    #[template_callback]
    fn big_mode_unapply(&self, _: &adw::Breakpoint) {
        self.remove_css_class("big-mode");
    }
}
