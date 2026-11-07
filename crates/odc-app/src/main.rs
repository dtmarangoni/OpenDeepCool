//! `opendeepcool`: the settings app (Phase 10). One window per user (GTK's default), which
//! mirrors the cooler and changes its settings through the service.

use std::path::PathBuf;
use std::process::ExitCode;

use gtk::gio;
use gtk::prelude::*;
use odc_app::client::Client;
use odc_app::flags::{self, Flags};
use odc_app::pci_ids;
use odc_app::ui::theme::LiveTheme;
use odc_app::ui::window::Window;

const APP_ID: &str = "io.github.dtmarangoni.OpenDeepCool";

/// The Omarchy state folder: `--omarchy-state`, or `~/.local/state/omarchy`.
fn state_dir(flags: &Flags) -> Option<PathBuf> {
    flags.omarchy_state.clone().or_else(|| {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state/omarchy"))
    })
}

fn log(line: &str) {
    eprintln!("opendeepcool: {line}");
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
        let state = state_dir(&flags);
        let theme = LiveTheme::first(state.as_deref(), &log);
        let client = Client::new();
        if let Some(path) = &flags.frame_log
            && let Err(e) = client.log_frames(path)
        {
            log(&format!("the frame log {}: {e}", path.display()));
        }
        let window = Window::new(
            app,
            client.sink(),
            PathBuf::from(pci_ids::PATH),
            &theme.tokens,
        );
        if let Some(live) = LiveTheme::follow(&window, state, theme.tokens, log) {
            window.keep(live);
        }
        client.start(window.clone(), flags.service_address.clone());
        window.widget().present();
    });
    // GTK reads no arguments: the flags are the app's own.
    app.run_with_args::<&str>(&[]).into()
}
