//! The expected-frame files `docs/spec/frames/mode-001.txt` to `mode-022.txt`, in the format of
//! `docs/spec/frames/README.md`: every mode × the four rotations × °C and °F where a temperature
//! is shown, with three value sets for the modes with values.

use crate::drawing::{self, Frame, Pixel, SIZE};
use crate::rules::{self, Picture, Ratio};

type Profile = [(&'static str, i64); 7];

/// The three value sets: every metric distinct and non-zero (README, Readings used).
const PROFILES: [Profile; 3] = [
    [
        ("CPU Usage", 3),
        ("Gpu usage", 6),
        ("Memory Load", 8),
        ("CPU Temperature", 4),
        ("Gpu temperature", 7),
        ("CPU Power", 5),
        ("Gpu power", 9),
    ],
    [
        ("CPU Usage", 42),
        ("Gpu usage", 67),
        ("Memory Load", 85),
        ("CPU Temperature", 57),
        ("Gpu temperature", 38),
        ("CPU Power", 65),
        ("Gpu power", 250),
    ],
    [
        ("CPU Usage", 100),
        ("Gpu usage", 91),
        ("Memory Load", 77),
        ("CPU Temperature", 120),
        ("Gpu temperature", 93),
        ("CPU Power", 1234),
        ("Gpu power", 999),
    ],
];

/// Each file's title: the mode as DeepCreative names it.
const TITLES: [&str; 22] = [
    "Load Mode, Separate, CPU",
    "Load Mode, Separate, GPU",
    "Load Mode, Separate, RAM",
    "Load Mode, Combined, CPU / GPU",
    "Load Mode, Combined, CPU / RAM",
    "Load Mode, Combined, GPU / RAM",
    "Temperature Guardian, Separate, CPU",
    "Temperature Guardian, Separate, GPU",
    "Temperature Guardian, Combined, CPU / GPU",
    "Power Guardian, Separate, CPU",
    "Power Guardian, Separate, GPU",
    "Power Guardian, Combined, CPU / GPU",
    "Music Mode, Full Screen",
    "Music Mode, Emoji 1 + Spectrum",
    "Music Mode, Emoji 2 + Spectrum",
    "Music Mode, Emoji 3 + Spectrum",
    "Music Mode, Emoji 4 + Spectrum",
    "Music Mode, Temperature + Spectrum, CPU",
    "Music Mode, Temperature + Spectrum, GPU",
    "Music Mode, Usage + Spectrum, CPU",
    "Music Mode, Usage + Spectrum, GPU",
    "Music Mode, Usage + Spectrum, RAM",
];

/// Each emoji loop: picture and duration in ms, in order (music.md).
fn emoji_loop(mode: u16) -> &'static [(usize, u32)] {
    match mode {
        14 => &[(1, 400), (0, 100), (1, 100), (0, 100), (1, 100), (0, 100)],
        15 | 16 => &[(0, 400), (1, 100), (0, 100), (1, 100), (0, 100), (1, 100)],
        17 => &[(0, 100), (1, 100), (2, 100), (1, 100), (0, 200), (3, 500)],
        _ => &[],
    }
}

fn reading(profile: &Profile, name: &str) -> i64 {
    profile
        .iter()
        .find(|(n, _)| *n == name)
        .map(|&(_, v)| v)
        .expect("every metric has a reading")
}

/// Every file: its name and its contents.
pub fn files() -> Vec<(String, String)> {
    (1..=22)
        .map(|mode| (format!("mode-{mode:03}.txt"), file(mode)))
        .collect()
}

fn file(mode: u16) -> String {
    let mut lines = vec![
        format!("# Mode {mode}: {}", TITLES[usize::from(mode) - 1]),
        String::new(),
    ];
    let units: &[bool] = if matches!(mode, 7 | 8 | 9 | 18 | 19) {
        &[false, true]
    } else {
        &[false]
    };
    let profiles: Vec<Option<&Profile>> = if drawing::value_mode(mode).is_some() {
        PROFILES.iter().map(Some).collect()
    } else {
        vec![None]
    };
    for rotation in 0..4 {
        for &fahrenheit in units {
            for &profile in &profiles {
                let (frames, note) = case(mode, rotation, fahrenheit, profile);
                let mut head = format!("## rotation {rotation}");
                if units.len() > 1 {
                    head += if fahrenheit { ", °F" } else { ", °C" };
                }
                if let Some(profile) = profile {
                    head += ", ";
                    head += &readings(mode, profile);
                }
                lines.push(head);
                lines.push(String::new());
                if let Some(note) = note {
                    lines.push(note);
                    lines.push(String::new());
                }
                block(&frames, &mut lines);
            }
        }
    }
    let mut text = lines.join("\n").trim_end().to_owned();
    text.push('\n');
    text
}

/// The readings a heading names, in the order of the mode table.
fn readings(mode: u16, profile: &Profile) -> String {
    let (_, names) = drawing::value_mode(mode).expect("a mode with values");
    names
        .iter()
        .map(|name| format!("{name} = {}", reading(profile, name)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One case's frames, and the line that says how to read them, if any.
fn case(
    mode: u16,
    rotation: u8,
    fahrenheit: bool,
    profile: Option<&Profile>,
) -> (Vec<Frame>, Option<String>) {
    if mode == 13 {
        let g = drawing::with_spectrum(&[[Pixel::Off; SIZE]; 2], 2);
        return (vec![drawing::rotate_cw(&g, rotation)], None);
    }
    if drawing::is_emoji(mode) {
        let pictures = drawing::emoji(mode);
        let steps = emoji_loop(mode);
        let frames = steps
            .iter()
            .map(|&(i, _)| {
                let mut top = [[Pixel::Off; SIZE]; 8];
                for (row, line) in top.iter_mut().zip(pictures[i]) {
                    *row = drawing::row(line);
                }
                drawing::rotate_cw(&drawing::with_spectrum(&top, 8), rotation)
            })
            .collect();
        let durations = steps
            .iter()
            .map(|&(_, ms)| format!("{ms} ms"))
            .collect::<Vec<_>>()
            .join(", ");
        let note = format!("one loop, picture durations in order: {durations}");
        return (frames, Some(note));
    }
    let (_, names) = drawing::value_mode(mode).expect("a mode with values");
    let profile = profile.expect("a mode with values has readings");
    let values: Vec<Option<Ratio>> = names
        .iter()
        .map(|name| Some(Ratio::whole(reading(profile, name))))
        .collect();
    let music = drawing::is_music_value(mode);
    let finish = |g: Frame| {
        let g = if music {
            drawing::with_spectrum(&g[..8], 8)
        } else {
            g
        };
        drawing::rotate_cw(&g, rotation)
    };
    match rules::picture(mode, &values, fahrenheit) {
        Picture::Static(g) => (vec![finish(g)], None),
        Picture::Scrolling(cycle) => {
            let frames: Vec<Frame> = (0..cycle.len()).map(|k| finish(cycle.window(k))).collect();
            let note = format!(
                "{} frames in time order, one per 100 ms tick, then the cycle repeats; \
                 the first is the home position, shown on the first frame after a mode switch",
                frames.len()
            );
            (frames, Some(note))
        }
    }
}

fn text(frame: &Frame) -> [String; SIZE] {
    frame.map(|row| {
        row.iter()
            .map(|px| match px {
                Pixel::On => '#',
                Pixel::Spectrum => '?',
                Pixel::Off => '.',
            })
            .collect()
    })
}

/// Frames seven to a line, two spaces apart, each line of frames followed by a blank line.
fn block(frames: &[Frame], lines: &mut Vec<String>) {
    for part in frames.chunks(7) {
        let texts: Vec<[String; SIZE]> = part.iter().map(text).collect();
        for r in 0..SIZE {
            let row: Vec<&str> = texts.iter().map(|t| t[r].as_str()).collect();
            lines.push(row.join("  "));
        }
        lines.push(String::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn committed() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/frames")
    }

    /// The port draws what the Python generator drew, which Phase 3 checked against DeepCreative's
    /// captures; from now on this is the regeneration check (Phase 5 plan, Tasks 0 and 10).
    #[test]
    fn regenerates_every_committed_frame_file_byte_for_byte() {
        let generated = files();
        let mut committed_names: Vec<String> = fs::read_dir(committed())
            .expect("docs/spec/frames/ is readable")
            .map(|entry| entry.expect("a directory entry").file_name())
            .map(|name| name.into_string().expect("a UTF-8 file name"))
            .filter(|name| name != "README.md")
            .collect();
        committed_names.sort();
        let generated_names: Vec<&str> = generated.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(committed_names, generated_names, "the files differ in name");
        for (name, text) in &generated {
            let committed = fs::read_to_string(committed().join(name)).expect("a committed file");
            let differs = committed
                .lines()
                .zip(text.lines())
                .enumerate()
                .find(|(_, (c, g))| c != g);
            if let Some((i, (c, g))) = differs {
                panic!("{name}:{}: committed `{c}`, generated `{g}`", i + 1);
            }
            assert!(committed == *text, "{name}: the files differ in length");
        }
        let cases: usize = generated
            .iter()
            .map(|(_, text)| text.lines().filter(|l| l.starts_with("## ")).count())
            .sum();
        assert_eq!(cases, 284);
    }
}
