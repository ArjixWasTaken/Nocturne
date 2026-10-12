use gtk::{gio, glib};
use std::path::PathBuf;

pub const APP_ID: &str = "dev.arjix.Nocturne";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

thread_local! {
    static SETTINGS: gio::Settings = gio::Settings::new(APP_ID);
}

/// Shared GSettings handle (main thread only).
pub fn settings() -> gio::Settings {
    SETTINGS.with(Clone::clone)
}

pub fn data_dir() -> PathBuf {
    glib::user_data_dir()
}
pub fn cache_dir() -> PathBuf {
    glib::user_cache_dir()
}
pub fn integrations_dir() -> PathBuf {
    data_dir().join("integrations")
}
pub fn downloads_dir() -> PathBuf {
    data_dir().join("downloads")
}

/// `mm:ss` / `h:mm:ss`.
pub fn display_time(seconds: i64) -> String {
    let (h, m, s) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}
