//! `NocturneWelcomePage` (pages/welcome.blp): pick a login method.
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/pages/welcome.ui")]
    pub struct WelcomePage {
        #[template_child]
        pub listbox_el: TemplateChild<gtk::ListBox>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WelcomePage {
        const NAME: &'static str = "NocturneWelcomePage";
        type Type = super::WelcomePage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for WelcomePage {
        fn constructed(&self) {
            self.parent_constructed();
            // The integrated Navidrome setup (setup.blp) is not ported, so it is not offered here.
            for m in &super::super::login::METHODS {
                let row = adw::ActionRow::builder()
                    .title(m.button_title)
                    .subtitle(m.button_subtitle)
                    .tooltip_text(m.button_title)
                    .activatable(true)
                    .build();
                row.add_prefix(&gtk::Image::builder().icon_name(m.icon).valign(gtk::Align::Center).build());
                row.add_suffix(&gtk::Image::builder().icon_name("go-next-symbolic").valign(gtk::Align::Center).build());
                let kind = m.kind;
                row.connect_activated(move |row| {
                    super::super::LoginDialog::new(kind).present(row.root().as_ref());
                });
                self.listbox_el.append(&row);
            }
        }
    }
    impl WidgetImpl for WelcomePage {}
    impl NavigationPageImpl for WelcomePage {}
}

glib::wrapper! {
    pub struct WelcomePage(ObjectSubclass<imp::WelcomePage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
