//! `opendeepcool`: the settings app (Phase 10). One window per user (GTK's default), which
//! mirrors the cooler and changes its settings through the service.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use gtk::gio;
use gtk::prelude::*;
use odc_app::client::Client;
use odc_app::flags::{self, Flags};
use odc_app::pci_ids;
use odc_app::theme::{self, Theme};
use odc_app::ui::window::Window;

const APP_ID: &str = "io.github.dtmarangoni.OpenDeepCool";

/// The current Omarchy theme's folder: `<state>/current/theme`, where `<state>` is
/// `~/.local/state/omarchy` or the folder `--omarchy-state` gives.
fn theme_dir(flags: &Flags) -> Option<PathBuf> {
    match &flags.omarchy_state {
        Some(state) => Some(state.join("current/theme")),
        None => {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(theme::OMARCHY_THEME_DIR))
        }
    }
}

/// The theme, its problems written to the journal.
fn load_theme(flags: &Flags) -> Theme {
    let dir = theme_dir(flags);
    let read = |name: &str| {
        dir.as_ref()
            .and_then(|d| fs::read_to_string(d.join(name)).ok())
    };
    let colors = read("colors.toml");
    let theme = theme::theme(colors.as_deref(), read("shell.toml").as_deref());
    for problem in &theme.problems {
        eprintln!("opendeepcool: theme: {problem}");
    }
    eprintln!(
        "opendeepcool: theme: {}",
        if colors.is_some() {
            "the Omarchy theme"
        } else {
            "the default theme"
        }
    );
    theme
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flags = match flags::parse(&args) {
        Ok(flags) => flags,
        Err(error) => {
            eprintln!("opendeepcool: {error}\n{}", flags::USAGE);
            return ExitCode::from(2);
        }
    };
    // A test's app never hands over to the user's own (decisions log D47).
    let app_flags = if flags.testing() {
        gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::empty()
    };
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(app_flags)
        .build();
    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        let theme = load_theme(&flags);
        let client = Client::new();
        let window = Window::new(
            app,
            client.sink(),
            PathBuf::from(pci_ids::PATH),
            &theme.tokens,
        );
        client.start(window.clone(), flags.service_address.clone());
        window.widget().present();
    });
    // GTK reads no arguments: the flags are the app's own.
    app.run_with_args::<&str>(&[]).into()
}
