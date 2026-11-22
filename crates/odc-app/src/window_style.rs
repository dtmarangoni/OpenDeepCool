//! The window's opacity under an Omarchy theme (`docs/design/settings-app.md`, Window opacity and
//! corners; Phase 11a Task 45): Omarchy's own Appearance > Windows > Window Opacity, read from the
//! two toggle files its `omarchy-hyprland-window-transparency` writes (read in `/usr/share/omarchy`
//! on 2026-11-22), never by running its programs.
//!
//! `opaque` is the file `toggles/hypr/window-opaque.lua` in Omarchy's state folder, `glassy` is
//! `window-glassy.lua`, and with neither it is `transparent`, Omarchy's default; when both exist,
//! opaque wins, as in its `current_mode()`. A file that is a folder, or can't be read as a file,
//! counts as absent.

use std::path::{Path, PathBuf};

use odc_ipc::WindowOpacity;

/// The alpha of the window's ground, and of the cards on it, by opacity (Phase 11a Tasks 45 and
/// 51; `docs/design/settings-app.md`, Window opacity and corners): solid for `opaque`; 0.985 for
/// `transparent`, Omarchy's standard opacity for a see-through window when focused; 0.75 for
/// `glassy`, the opacity Omarchy gives Nautilus under glass, with the compositor's blur asked for
/// by the window's class `glassy` (`window.glassy` in the stylesheet, Omarchy's own
/// `backdrop-filter: blur(20px)`).
pub fn alpha(opacity: WindowOpacity) -> f64 {
    match opacity {
        WindowOpacity::Opaque => 1.0,
        WindowOpacity::Transparent => 0.985,
        WindowOpacity::Glassy => 0.75,
    }
}

/// Omarchy's toggle files for Hyprland, in its state folder.
pub fn toggles_dir(state: &Path) -> PathBuf {
    state.join("toggles").join("hypr")
}

/// The window opacity Omarchy has chosen in `state`.
pub fn omarchy_opacity(state: &Path) -> WindowOpacity {
    let dir = toggles_dir(state);
    if dir.join("window-opaque.lua").is_file() {
        WindowOpacity::Opaque
    } else if dir.join("window-glassy.lua").is_file() {
        WindowOpacity::Glassy
    } else {
        WindowOpacity::Transparent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn state(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("odc-app-style-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("a folder");
        root
    }

    fn toggle(state: &Path, file: &str) {
        let dir = toggles_dir(state);
        fs::create_dir_all(&dir).expect("the toggles folder");
        fs::write(dir.join(file), "-- a toggle\n").expect("a toggle");
    }

    #[test]
    fn with_no_toggle_file_the_windows_are_transparent_as_omarchy_s_default() {
        let root = state("none");
        assert_eq!(omarchy_opacity(&root), WindowOpacity::Transparent);
        // The folder without the files, too.
        fs::create_dir_all(toggles_dir(&root)).expect("the folder");
        assert_eq!(omarchy_opacity(&root), WindowOpacity::Transparent);
        // A state folder that isn't there.
        assert_eq!(
            omarchy_opacity(&root.join("nowhere")),
            WindowOpacity::Transparent
        );
        fs::remove_dir_all(&root).expect("cleaned");
    }

    #[test]
    fn each_toggle_file_gives_its_mode_and_opaque_wins_over_glassy() {
        let root = state("modes");
        toggle(&root, "window-glassy.lua");
        assert_eq!(omarchy_opacity(&root), WindowOpacity::Glassy);
        toggle(&root, "window-opaque.lua");
        assert_eq!(omarchy_opacity(&root), WindowOpacity::Opaque);
        fs::remove_file(toggles_dir(&root).join("window-glassy.lua")).expect("removed");
        assert_eq!(omarchy_opacity(&root), WindowOpacity::Opaque);
        fs::remove_file(toggles_dir(&root).join("window-opaque.lua")).expect("removed");
        assert_eq!(omarchy_opacity(&root), WindowOpacity::Transparent);
        fs::remove_dir_all(&root).expect("cleaned");
    }

    #[test]
    fn a_folder_with_a_toggle_s_name_counts_as_absent() {
        let root = state("folder");
        fs::create_dir_all(toggles_dir(&root).join("window-opaque.lua")).expect("a folder");
        fs::create_dir_all(toggles_dir(&root).join("window-glassy.lua")).expect("a folder");
        assert_eq!(omarchy_opacity(&root), WindowOpacity::Transparent);
        fs::remove_dir_all(&root).expect("cleaned");
    }

    #[test]
    fn the_other_toggles_of_omarchy_change_nothing() {
        let root = state("others");
        for file in ["corners-sharp.lua", "window-no-gaps.lua", "bar-glassy.lua"] {
            toggle(&root, file);
        }
        assert_eq!(omarchy_opacity(&root), WindowOpacity::Transparent);
        fs::remove_dir_all(&root).expect("cleaned");
    }
}
