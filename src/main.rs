mod actions;
mod application;
mod backend;
mod config;
mod db;
mod integration;
mod lyrics;
mod models;
mod mpris;
mod player;
mod preferences;
mod queue;
mod runtime;
mod secret;
mod widgets;
mod window;

use adw::prelude::*;
use clap::Parser;
use gtk::{gio, glib};

/// Command line. Playback flags are forwarded to the running instance as `app.*` actions.
#[derive(Parser, Debug)]
#[command(name = "nocturne", version, about = "Nocturne music player")]
struct Cli {
    #[arg(long)]
    player_next: bool,
    #[arg(long)]
    player_previous: bool,
    #[arg(long)]
    player_pause: bool,
    #[arg(long)]
    player_play: bool,
    #[arg(long)]
    player_toggle: bool,
    #[arg(long)]
    player_raise_volume: bool,
    #[arg(long)]
    player_lower_volume: bool,
    #[arg(long, value_name = "ID")]
    song_show: Option<String>,
    #[arg(long, value_name = "ID")]
    song_play: Option<String>,
    #[arg(long, value_name = "ID")]
    play_song_next: Option<String>,
    #[arg(long, value_name = "ID")]
    play_song_later: Option<String>,
    #[arg(long, value_name = "ID")]
    album_show: Option<String>,
    #[arg(long, value_name = "ID")]
    album_play: Option<String>,
    #[arg(long, value_name = "ID")]
    album_play_shuffle: Option<String>,
    #[arg(long, value_name = "ID")]
    artist_show: Option<String>,
    #[arg(long, value_name = "ID")]
    playlist_show: Option<String>,
    #[arg(long, value_name = "ID")]
    playlist_play: Option<String>,
    /// Audio files to play.
    files: Vec<String>,
}

impl Cli {
    fn actions(&self) -> Vec<(&'static str, Option<glib::Variant>)> {
        let flags = [
            (self.player_next, "player_next"),
            (self.player_previous, "player_previous"),
            (self.player_pause, "player_pause"),
            (self.player_play, "player_play"),
            (self.player_toggle, "player_toggle"),
            (self.player_raise_volume, "player_raise_volume"),
            (self.player_lower_volume, "player_lower_volume"),
        ];
        let ids = [
            (&self.song_show, "show_song_details"),
            (&self.song_play, "play_song"),
            (&self.play_song_next, "play_song_next"),
            (&self.play_song_later, "play_song_later"),
            (&self.album_show, "show_album"),
            (&self.album_play, "play_album"),
            (&self.album_play_shuffle, "play_album_shuffle"),
            (&self.artist_show, "show_artist"),
            (&self.playlist_show, "show_playlist"),
            (&self.playlist_play, "play_playlist"),
        ];
        flags
            .into_iter()
            .filter(|(on, _)| *on)
            .map(|(_, a)| (a, None))
            .chain(ids.into_iter().filter_map(|(v, a)| v.as_ref().map(|id| (a, Some(id.to_variant())))))
            .collect()
    }
}

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).init();
    let cli = Cli::parse();

    // Translations for Blueprint strings (and future gettext! calls).
    let _ = gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
    let _ = gettextrs::bindtextdomain("nocturne", option_env!("NOCTURNE_LOCALEDIR").unwrap_or("/usr/share/locale"));
    let _ = gettextrs::bind_textdomain_codeset("nocturne", "UTF-8");
    let _ = gettextrs::textdomain("nocturne");

    gio::resources_register_include!("nocturne.gresource").expect("failed to register resources");
    glib::set_prgname(Some(config::APP_ID));
    glib::set_application_name("Nocturne");
    let _ = gst::init();

    let app = application::Application::default();
    let actions = cli.actions();
    if !actions.is_empty() {
        // Remote control of a running instance.
        if app.register(gio::Cancellable::NONE).is_err() || !app.is_remote() {
            eprintln!("Nocturne has to already be running for the options to work");
            return glib::ExitCode::FAILURE;
        }
        for (name, param) in actions {
            app.activate_action(name, param.as_ref());
        }
        return glib::ExitCode::SUCCESS;
    }

    let files: Vec<String> = std::iter::once("nocturne".to_string()).chain(cli.files).collect();
    app.run_with_args(&files)
}
