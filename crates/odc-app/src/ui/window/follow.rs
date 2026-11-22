//! What the window follows (Phase 11a Tasks 1, 11 and 14): GTK's reading of the system's dark or
//! light preference, the two settings for reduced motion, the toplevel's state on the compositor,
//! and the theme the window shows from them and from the service's two settings. A part of
//! [`Window`](super::Window), split from `window.rs`.

use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gdk, glib};

use super::{Inner, Window};
use crate::choice::{self, Preference, Source};
use crate::theme::Tokens;
use odc_ipc::WindowOpacity;

/// GTK's `gtk-interface-color-scheme` as the app's [`Preference`].
fn preference_of(settings: &gtk::Settings) -> Preference {
    match settings.gtk_interface_color_scheme() {
        gtk::InterfaceColorScheme::Unsupported => Preference::Unsupported,
        gtk::InterfaceColorScheme::Dark => Preference::Dark,
        gtk::InterfaceColorScheme::Light => Preference::Light,
        _ => Preference::Default,
    }
}

/// GTK's settings that say animations are off or reduced.
const ANIMATION_SETTINGS: [&str; 2] = ["gtk-enable-animations", "gtk-interface-reduced-motion"];

/// Whether the system asks for reduced motion: animations switched off, or, from GTK 4.22, the
/// preference the XDG desktop portal gives (`reduce`).
pub fn reduced_motion(settings: &gtk::Settings) -> bool {
    let animations: bool = settings.property("gtk-enable-animations");
    let reduce = settings
        .find_property("gtk-interface-reduced-motion")
        .is_some_and(|_| {
            let value = settings.property_value("gtk-interface-reduced-motion");
            glib::EnumValue::from_value(&value).is_some_and(|(_, value)| value.nick() == "reduce")
        });
    !animations || reduce
}

/// Whether a toplevel's state says no one sees it: minimised, or suspended (GTK 4.12).
pub fn away(state: gdk::ToplevelState) -> bool {
    state.intersects(gdk::ToplevelState::MINIMIZED | gdk::ToplevelState::SUSPENDED)
}

/// A handler on GTK's settings, which outlive the window: disconnected when this is dropped.
struct Unfollow(gtk::Settings, Option<glib::SignalHandlerId>);

impl Drop for Unfollow {
    fn drop(&mut self) {
        if let Some(id) = self.1.take() {
            self.0.disconnect(id);
        }
    }
}

/// Reads `window`'s toplevel state now and at each change, and tells the display page. Each
/// realisation has a surface of its own, which the old handler left with the old surface.
fn watch_toplevel(weak: &Weak<Inner>, window: &gtk::ApplicationWindow) {
    let Some(toplevel) = window
        .surface()
        .and_then(|surface| surface.downcast::<gdk::Toplevel>().ok())
    else {
        return;
    };
    let show = {
        let weak = weak.clone();
        move |toplevel: &gdk::Toplevel| {
            if let Some(inner) = weak.upgrade() {
                inner.display.set_away(away(toplevel.state()));
            }
        }
    };
    show(&toplevel);
    toplevel.connect_state_notify(show);
}

impl Window {
    /// Follows GTK's reading of the system's dark or light preference, which GDK fills from the
    /// XDG desktop portal and keeps live (Phase 11a Task 1): read now, and again at each change.
    pub(super) fn follow_preference(&self) {
        let Some(settings) = gtk::Settings::default() else {
            return;
        };
        self.preference(preference_of(&settings));
        let weak = Rc::downgrade(&self.0);
        let id = settings.connect_gtk_interface_color_scheme_notify(move |settings| {
            if let Some(inner) = weak.upgrade() {
                Window(inner).preference(preference_of(settings));
            }
        });
        // Dropped with the window: the settings outlive it.
        self.keep(Unfollow(settings, Some(id)));
    }

    /// Follows GTK's two settings for reduced motion, read now and at each change: the tiles
    /// stand at rest while either asks for it (design: Live tiles, Reduced motion). The app
    /// never writes either.
    pub(super) fn follow_motion(&self) {
        let Some(settings) = gtk::Settings::default() else {
            return;
        };
        self.0.display.set_reduced(reduced_motion(&settings));
        for name in ANIMATION_SETTINGS {
            // The second exists from GTK 4.22; without it, only the first matters.
            if settings.find_property(name).is_none() {
                continue;
            }
            let weak = Rc::downgrade(&self.0);
            let id = settings.connect_notify_local(Some(name), move |settings, _| {
                if let Some(inner) = weak.upgrade() {
                    inner.display.set_reduced(reduced_motion(settings));
                }
            });
            self.keep(Unfollow(settings.clone(), Some(id)));
        }
    }

    /// Follows the window's own state on the compositor: minimised or suspended, the tiles don't
    /// move (design: Live tiles, When the tick runs). The surface exists once the window is
    /// realised, and again after each realisation.
    pub(super) fn follow_toplevel(&self) {
        let weak = Rc::downgrade(&self.0);
        self.0.window.connect_realize({
            let weak = weak.clone();
            move |window| watch_toplevel(&weak, window)
        });
        // A window already realised has had its realisation.
        if self.0.window.is_realized() {
            watch_toplevel(&weak, &self.0.window);
        }
    }

    /// The system's preference, as GTK reports it.
    pub(super) fn preference(&self, preference: Preference) {
        let changed = {
            let mut inputs = self.0.inputs.borrow_mut();
            let changed = inputs.ui.system.preference != preference;
            inputs.ui.system.preference = preference;
            changed
        };
        if changed {
            self.apply_theme();
            self.render();
        }
    }

    /// The Omarchy theme as found now: `None` when there is none. The window shows it when the
    /// system's theme is followed (Phase 11a Task 11: [`choice`]).
    pub fn omarchy_theme(&self, tokens: Option<Tokens>) {
        self.0.omarchy.set(tokens);
        self.0.inputs.borrow_mut().ui.system.omarchy = tokens.map(|t| t.dark);
        // Its colours may have changed with its mode as it was: resolved again whatever the
        // inputs say.
        self.0.resolved_from.set(None);
        self.apply_theme();
        self.render();
    }

    /// What the theme is made from now: the system's facts and the service's two settings.
    pub(super) fn theme_inputs(&self) -> choice::Inputs {
        let (use_system, dark, opacity, corners) = self.0.theme_settings.get();
        choice::Inputs::new(self.0.inputs.borrow().ui.system, use_system, dark)
            .with_window(opacity, corners)
    }

    /// Resolves the theme again when what it is made from changed, and shows it.
    pub(super) fn apply_theme(&self) {
        let inputs = self.theme_inputs();
        if self.0.resolved_from.get() == Some(inputs) {
            return;
        }
        self.0.resolved_from.set(Some(inputs));
        let tokens = match inputs.source() {
            Source::Omarchy => self
                .0
                .omarchy
                .get()
                .unwrap_or_else(|| Tokens::default_theme(true)),
            // OpenDeepCool's own theme from the system's preference: opaque and rounded, the
            // saved window settings aren't applied (Window opacity and corners, section 5).
            Source::Own { dark } if inputs.follows_system() => Tokens::default_theme(dark),
            Source::Own { dark } => {
                Tokens::default_theme(dark).with_window(inputs.opacity, inputs.corners)
            }
        };
        self.set_theme(&tokens);
    }

    /// The theme: the CSS reloaded in the same provider, the preview redrawn once.
    pub fn set_theme(&self, tokens: &Tokens) {
        if self.0.shown_tokens.get() == Some(*tokens) {
            return;
        }
        self.0.shown_tokens.set(Some(*tokens));
        self.0.provider.load_from_string(&tokens.css());
        // The window's classes: `glassy` asks for the compositor's blur, `own-corners` gives the
        // window OpenDeepCool's own theme's corners (Window opacity and corners).
        for (class, on) in [
            ("glassy", tokens.opacity == WindowOpacity::Glassy),
            ("own-corners", tokens.corners.is_some()),
        ] {
            if on {
                self.0.window.add_css_class(class);
            } else {
                self.0.window.remove_css_class(class);
            }
        }
        self.0.provider.set_prefers_color_scheme(if tokens.dark {
            gtk::InterfaceColorScheme::Dark
        } else {
            gtk::InterfaceColorScheme::Light
        });
        self.0.preview.set_theme(tokens);
        self.0.display.set_theme(tokens);
    }
}
