//! `NocturneLoginDialog` (pages/login.blp): credentials for one login method.
use crate::{application::Application, backend::Connection, config::settings, runtime::spawn_local};
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, gio, glib};
use std::cell::Cell;

/// A login method shown on the welcome page.
pub struct Method {
    pub kind: &'static str,
    pub icon: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub button_title: &'static str,
    pub button_subtitle: &'static str,
    pub entries: &'static [&'static str],
    pub login_label: &'static str,
    pub default_url: &'static str,
}

pub const METHODS: [Method; 3] = [
    Method {
        kind: crate::backend::subsonic::KIND,
        icon: "opensubsonic-symbolic",
        title: "External Server",
        description: "Connect to an OpenSubsonic server like Navidrome.",
        button_title: "External Server",
        button_subtitle: "Use an existing OpenSubsonic instance",
        entries: &["url", "user", "password", "trust-server"],
        login_label: "Login",
        default_url: "http://127.0.0.1:4533",
    },
    Method {
        kind: crate::backend::jellyfin::KIND,
        icon: "jellyfin-symbolic",
        title: "Jellyfin",
        description: "Connect to a Jellyfin server.",
        button_title: "Jellyfin",
        button_subtitle: "Use an existing Jellyfin instance",
        entries: &["url", "user", "password", "trust-server"],
        login_label: "Login",
        default_url: "http://127.0.0.1:8096",
    },
    Method {
        kind: crate::backend::local::KIND,
        icon: "folder-open-symbolic",
        title: "Local Files",
        description: "Let Nocturne load your local files directly, for big libraries it is recommended to use a dedicated server.",
        button_title: "Local Files",
        button_subtitle: "Limited functionality",
        entries: &["library-dir"],
        login_label: "Continue",
        default_url: "",
    },
];

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/pages/login.ui")]
    pub struct LoginDialog {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub link_button_el: TemplateChild<gtk::LinkButton>,
        #[template_child]
        pub server_status_el: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub extra_menu_el: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub url_el: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub url_options_el: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub trust_server_el: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub directory_el: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub user_el: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub password_el: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub login_button_el: TemplateChild<gtk::Button>,
        #[template_child]
        pub quick_connect_button_el: TemplateChild<gtk::Button>,
        pub method: Cell<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LoginDialog {
        const NAME: &'static str = "NocturneLoginDialog";
        type Type = super::LoginDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LoginDialog {}
    impl WidgetImpl for LoginDialog {}
    impl AdwDialogImpl for LoginDialog {}
}

glib::wrapper! {
    pub struct LoginDialog(ObjectSubclass<imp::LoginDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl LoginDialog {
    /// Dialog for the login method with backend `kind` (see [`METHODS`]).
    pub fn new(kind: &str) -> Self {
        let obj: Self = glib::Object::new();
        let imp = obj.imp();
        let idx = METHODS.iter().position(|m| m.kind == kind).unwrap_or(0);
        imp.method.set(idx);
        let m = &METHODS[idx];
        let s = settings();
        let same_kind = s.string("selected-instance-type") == m.kind;
        let has = |e: &str| m.entries.contains(&e);

        imp.status_page.set_icon_name(Some(m.icon));
        imp.status_page.set_title(m.title);
        imp.status_page.set_description(Some(m.description));
        if let Some(p) = imp.link_button_el.parent() {
            p.set_visible(false);
        }
        imp.server_status_el.set_visible(false);
        imp.extra_menu_el.set_visible(false);

        let url = s.string("integration-ip");
        imp.url_el.set_visible(has("url"));
        imp.url_el.set_text(if same_kind && !url.is_empty() { &url } else { m.default_url });
        imp.url_options_el.set_visible(has("trust-server"));
        imp.trust_server_el.set_visible(has("trust-server"));
        imp.trust_server_el.set_active(same_kind && s.boolean("integration-trust-server"));
        imp.user_el.set_visible(has("user"));
        imp.user_el.set_text(if same_kind { s.string("integration-user") } else { "".into() }.as_str());
        imp.password_el.set_visible(has("password"));
        imp.directory_el.set_visible(has("library-dir"));
        imp.directory_el.set_subtitle(&s.string("integration-library-dir"));
        imp.login_button_el.set_label(m.login_label);
        imp.quick_connect_button_el.set_visible(m.kind == crate::backend::jellyfin::KIND);
        obj.update_login_sensitivity();
        obj
    }

    fn method(&self) -> &'static Method {
        &METHODS[self.imp().method.get()]
    }

    fn connection(&self) -> Connection {
        let imp = self.imp();
        let mut url = imp.url_el.text().trim().to_string();
        if !url.is_empty() && !url.starts_with("http") {
            url = format!("http://{url}");
        }
        Connection {
            url,
            user: imp.user_el.text().trim().to_string(),
            password: imp.password_el.text().trim().to_string(),
            trust_server: imp.trust_server_el.is_active(),
            library_dir: imp.directory_el.subtitle().map(|s| s.to_string()).unwrap_or_default(),
            library_ids: settings().strv("library-ids").iter().map(|g| g.to_string()).collect(),
        }
    }

    fn login(&self, conn: Connection) {
        let imp = self.imp();
        imp.login_button_el.set_sensitive(false);
        let obj = self.clone();
        let kind = self.method().kind;
        spawn_local(async move {
            match Application::get().login(kind, conn, true).await {
                Ok(()) => {
                    obj.close();
                }
                Err(e) => {
                    tracing::warn!("login failed: {e:#}");
                    let toast = adw::Toast::builder()
                        .title("Login Failed")
                        .button_label("Show Details")
                        .action_name("app.show_error")
                        .action_target(&format!("{e:#}").to_variant())
                        .build();
                    obj.imp().toast_overlay.add_toast(toast);
                    obj.update_login_sensitivity();
                }
            }
        });
    }

    #[template_callback]
    fn update_login_sensitivity(&self) {
        let imp = self.imp();
        let has = |e: &str| self.method().entries.contains(&e);
        let ok = (!has("url") || !imp.url_el.text().is_empty())
            && (!has("user") || !imp.user_el.text().is_empty())
            && (!has("password") || !imp.password_el.text().is_empty())
            && (!has("library-dir") || imp.directory_el.subtitle().is_some_and(|s| !s.is_empty()));
        imp.login_button_el.set_sensitive(ok);
    }

    #[template_callback]
    fn library_changed(&self) {}

    #[template_callback]
    fn server_restart_requested(&self) {}

    #[template_callback]
    fn open_local_directory(&self, row: &adw::ActionRow) {
        let current = row.subtitle().unwrap_or_default();
        let dialog = gtk::FileDialog::builder().title("Music Library").build();
        if !current.is_empty() {
            dialog.set_initial_folder(Some(&gio::File::for_path(current.as_str())));
        }
        let row = row.clone();
        let root = self.root().and_downcast::<gtk::Window>();
        dialog.select_folder(root.as_ref(), gio::Cancellable::NONE, move |res| {
            if let Ok(folder) = res
                && let Some(path) = folder.path()
            {
                row.set_subtitle(&path.to_string_lossy());
            }
        });
    }

    #[template_callback]
    fn login_button_clicked(&self) {
        if self.imp().login_button_el.is_sensitive() {
            self.login(self.connection());
        }
    }

    #[template_callback]
    fn quick_connect_button_clicked(&self) {
        use crate::{backend::jellyfin, runtime::run};
        let conn = self.connection();
        let obj = self.clone();
        spawn_local(async move {
            let (url, trust) = (conn.url.clone(), conn.trust_server);
            let (secret, code) = match run(async move { jellyfin::quick_connect_initiate(&url, trust).await }).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("quick connect: {e:#}");
                    (String::new(), String::new())
                }
            };
            let dialog = adw::AlertDialog::new(Some("Quick Connect"), Some(if code.is_empty() { "Error getting code" } else { &code }));
            dialog.set_extra_child(Some(&gtk::LinkButton::with_label(&format!("{}/web/#/quickconnect", conn.url), "Quick Connect Page")));
            dialog.add_response("cancel", "Cancel");
            dialog.set_close_response("cancel");
            dialog.present(Some(&obj));
            if secret.is_empty() {
                return;
            }
            for _ in 0..5 {
                glib::timeout_future_seconds(5).await;
                if dialog.root().is_none() {
                    return;
                }
                let (url, s) = (conn.url.clone(), secret.clone());
                if run(async move { jellyfin::quick_connect_check(&url, trust, &s).await }).await.unwrap_or(false) {
                    dialog.close();
                    obj.login(Connection { password: secret, ..conn });
                    return;
                }
            }
            dialog.close();
        });
    }
}
