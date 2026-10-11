//! `NocturneStarredPage` (pages/starred.blp): favorite songs or artists, depending on the tag.
use super::{LibraryPage, Kind, bind_view_mode, on_short_content, queue_parts, wrap_append, wrap_clear};
use crate::{integration, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/pages/starred.ui")]
    pub struct StarredPage {
        #[template_child]
        pub list_el: TemplateChild<crate::widgets::song::SongQueue>,
        #[template_child]
        pub wrapbox_el: TemplateChild<gtk::Widget>,
        #[template_child]
        pub search_entry_el: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub scrolledwindow: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub toggle_group_el: TemplateChild<gtk::Widget>,
        #[template_child]
        pub failed_status_el: TemplateChild<adw::StatusPage>,
        pub item_ids: RefCell<Vec<String>>,
        pub items: RefCell<HashMap<String, (gtk::Widget, gtk::Widget)>>,
        pub offset: Cell<usize>,
        pub lazy: Cell<bool>,
        pub bound: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StarredPage {
        const NAME: &'static str = "NocturneStarredPage";
        type Type = super::StarredPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for StarredPage {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.connect_tag_notify(|o| o.on_tag_assigned());
            obj.on_tag_assigned();
            let weak = obj.downgrade();
            on_short_content(&self.scrolledwindow, move || {
                if let Some(o) = weak.upgrade() {
                    o.search();
                }
            });
        }
    }
    impl WidgetImpl for StarredPage {}
    impl NavigationPageImpl for StarredPage {
        fn showing(&self) {
            self.parent_showing();
            if self.lazy.replace(false) {
                self.obj().reload();
            }
        }
    }
}

glib::wrapper! {
    pub struct StarredPage(ObjectSubclass<imp::StarredPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl LibraryPage for StarredPage {
    fn load(&self) {
        self.reload();
    }

    fn reload(&self) {
        let imp = self.imp();
        imp.main_stack.set_visible_child_name("loading");
        self.reset();
        let Some(integ) = integration::current() else { return };
        let obj = self.clone();
        spawn_local(async move {
            let (artists, _, songs) = integ.starred().await;
            obj.imp().item_ids.replace(if obj.kind() == Kind::Artist { artists } else { songs });
            obj.search();
        });
    }

    fn reset(&self) {
        let imp = self.imp();
        imp.offset.set(0);
        queue_parts(&imp.list_el).1.remove_all();
        wrap_clear(&imp.wrapbox_el);
        imp.items.borrow_mut().clear();
        imp.item_ids.borrow_mut().clear();
    }

    fn set_lazy_reload(&self) {
        self.imp().lazy.set(true);
    }
}

#[gtk::template_callbacks]
impl StarredPage {
    fn kind(&self) -> Kind {
        if self.tag().is_some_and(|t| t.starts_with("artists")) { Kind::Artist } else { Kind::Song }
    }

    fn on_tag_assigned(&self) {
        let imp = self.imp();
        let Some(tag) = self.tag() else { return };
        if imp.bound.replace(true) {
            return;
        }
        let prefix = tag.split('-').next().unwrap_or("songs");
        bind_view_mode(&imp.toggle_group_el, &format!("default-view-mode-{prefix}"));
        if self.kind() == Kind::Artist {
            imp.search_entry_el.update_property(&[gtk::accessible::Property::Label("Search favorite artists")]);
            imp.failed_status_el.set_title("No Artists Found");
        }
    }

    fn search(&self) {
        let imp = self.imp();
        let Some(integ) = integration::current() else { return };
        let kind = self.kind();
        let query = imp.search_entry_el.text().to_lowercase();
        let ids = imp.item_ids.borrow().clone();
        let mut shown: Vec<String> = ids
            .into_iter()
            .filter(|id| {
                query.is_empty() || {
                    let text = match kind {
                        Kind::Artist => integ.artist(id).name(),
                        _ => {
                            let s = integ.song(id);
                            format!("{}{}", s.title(), s.artist())
                        }
                    };
                    text.to_lowercase().contains(&query)
                }
            })
            .collect();
        shown.truncate(imp.offset.get() + 30);
        let (stack, list) = queue_parts(&imp.list_el);
        let mut items = imp.items.borrow_mut();
        for (id, (r, b)) in items.iter() {
            let v = shown.contains(id);
            r.set_visible(v);
            b.set_visible(v);
        }
        for id in &shown {
            if items.contains_key(id) {
                continue;
            }
            let (r, b): (gtk::Widget, gtk::Widget) = match kind {
                Kind::Artist => (
                    crate::widgets::artist::ArtistRow::new(id).upcast(),
                    crate::widgets::artist::ArtistButton::new(id).upcast(),
                ),
                _ => (crate::widgets::song::SongRow::new(id).upcast(), crate::widgets::song::SongButton::new(id).upcast()),
            };
            list.append(&r);
            wrap_append(&*imp.wrapbox_el, &b);
            items.insert(id.clone(), (r, b));
        }
        imp.offset.set(imp.offset.get() + 30);
        let name = if shown.is_empty() { "no-content" } else { "content" };
        imp.main_stack.set_visible_child_name(name);
        stack.set_visible_child_name(name);
    }

    #[template_callback]
    fn on_search(&self, _entry: &gtk::SearchEntry) {
        self.imp().offset.set(0);
        self.search();
    }

    #[template_callback]
    fn scroll_edge_reached(&self, _sw: &gtk::ScrolledWindow, pos: gtk::PositionType) {
        if pos == gtk::PositionType::Bottom && self.imp().offset.get() < self.imp().item_ids.borrow().len() {
            self.search();
        }
    }
}
