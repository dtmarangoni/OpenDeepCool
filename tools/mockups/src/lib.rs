//! The settings app's design mockups (`docs/design/settings-app.md`), one page per state the
//! design draws: the template below, filled with the state's title, its description, the links
//! to every page, the state as JSON, and the mockups' own `app.css` and `app.js` from
//! `docs/design/mockups-src/` (AGENTS.md rule 13: HTML, CSS and JS are the mockups' formats).
//! Readings, chips, PCI addresses and GPU names are made up (rule 10).

/// Where the mockups' style sheet and script live, and where the pages go.
pub const SOURCES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design/mockups-src");
pub const PAGES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design/mockups");

/// A JSON value whose objects keep their keys in order, as the mockups' script reads them.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// The value as text: `", "` between items, `": "` after a key, every character but `"`,
    /// `\` and the control characters as itself.
    pub fn text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Int(n) => out.push_str(&n.to_string()),
            Json::Str(s) => quote(s, out),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Object(pairs) => {
                out.push('{');
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    quote(key, out);
                    out.push_str(": ");
                    value.write(out);
                }
                out.push('}');
            }
        }
    }

    /// The object with `key` set to `value`: in its place when it has the key, last otherwise.
    ///
    /// # Panics
    ///
    /// When the value isn't an object: a programming error.
    #[must_use]
    pub fn with(mut self, key: &str, value: Json) -> Json {
        let Json::Object(pairs) = &mut self else {
            panic!("{key}: not an object")
        };
        match pairs.iter_mut().find(|(k, _)| k == key) {
            Some((_, old)) => *old = value,
            None => pairs.push((key.to_owned(), value)),
        }
        self
    }
}

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn s(text: &str) -> Json {
    Json::Str(text.to_owned())
}

fn obj(pairs: &[(&str, Json)]) -> Json {
    Json::Object(
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

/// The GPUs: made-up product names and PCI addresses.
fn gpus() -> Vec<Json> {
    let gpu = |kind: &str, name: &str, driver: &str, pci: &str| {
        obj(&[
            ("kind", s(kind)),
            ("name", s(name)),
            ("driver", s(driver)),
            ("pci", s(pci)),
        ])
    };
    vec![
        gpu(
            "Dedicated",
            "Contoso Graphics R870",
            "exgpu",
            "0000:0a:00.0",
        ),
        gpu(
            "Integrated",
            "Fabrikam Integrated Graphics 7",
            "exgpu",
            "0000:00:02.0",
        ),
        gpu("Unknown type", "", "othergpu", "0000:07:00.0").with("norule", Json::Bool(true)),
    ]
}

/// The state every page starts from: the Display page in Temperature · CPU + GPU, everything
/// connected and several GPUs on Automatic.
fn base() -> Json {
    obj(&[
        // The theme card (Task 11): an Omarchy theme found, the system preferring dark, and the
        // service's two settings at their defaults. The page draws the theme these give.
        ("omarchyTheme", Json::Bool(true)),
        ("colorScheme", s("dark")),
        ("useSystemTheme", Json::Bool(true)),
        ("darkTheme", Json::Bool(true)),
        ("page", s("display")),
        ("service", Json::Bool(true)),
        ("cooler", Json::Bool(true)),
        ("audio", Json::Bool(true)),
        ("notAvailable", Json::Bool(false)),
        ("mode", Json::Int(9)),
        ("unit", s("C")),
        ("rotation", Json::Int(0)),
        ("gpus", Json::Array(gpus())),
        ("gpu", Json::Null),
        ("gpuInUse", Json::Int(0)),
        ("unavailable", obj(&[])),
        (
            "live",
            obj(&[
                ("cpu_usage", Json::Int(23)),
                ("gpu_usage", Json::Int(71)),
                ("ram", Json::Int(47)),
                ("cpu_temp", Json::Int(49)),
                ("gpu_temp", Json::Int(44)),
                ("cpu_power", Json::Int(41)),
                ("gpu_power", Json::Int(156)),
            ]),
        ),
        ("peaks", Json::Bool(true)),
        ("barFall", Json::Int(2)),
        ("peakFall", Json::Int(1)),
        ("open", Json::Null),
    ])
}

/// One mockup page.
pub struct Page {
    pub file: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub state: Json,
}

/// Every mockup page, in the order of their links.
pub fn pages() -> Vec<Page> {
    let page = |file, title, description, state| Page {
        file,
        title,
        description,
        state,
    };
    let config = || base().with("page", s("config"));
    let [dedicated, integrated, no_rule] = <[Json; 3]>::try_from(gpus()).expect("three GPUs");
    vec![
        page(
            "display.html",
            "Display screen",
            "The mode screen: the header with the title in the middle and a gear and a close icon at the right; the page bar with the Display title and the Refresh icon; 23 mode tiles in four groups; the live preview on the left. Click a tile: it applies at once (here: Temperature, CPU + GPU). Nothing stands under the preview's caption but the status items. Click the gear for Configuration. Every tile shows its mode drawn with the same readings as the preview, one moment of what the app animates.",
            base(),
        ),
        page(
            "display-music.html",
            "Display screen, a Music mode",
            "Music, CPU temperature chosen, with sound attached; the status line adds the Music sound item. The preview's spectrum and the Music tiles' spectra are one moment of the same live levels, which the app animates.",
            base().with("mode", Json::Int(18)),
        ),
        page(
            "display-music-no-sound.html",
            "Display screen, a Music mode without sound",
            "Music, CPU temperature chosen, but no sound is attached (the status reads \"No sound: spectrum is silent\"). The preview shows the silent spectrum, one bar row, as the cooler does, and the Music tiles show their still sample: they are live only while sound is attached and a Music mode is shown.",
            base()
                .with("mode", Json::Int(18))
                .with("audio", Json::Bool(false)),
        ),
        page(
            "display-scrolling.html",
            "Display screen, a value of 100 or more",
            "Power, CPU + GPU with GPU power at 156 W; the preview shows the strip at its home position, as does every tile whose value is 100 or more (the app scrolls them at the cooler's 100 ms tick, together). The unit is °F, set on the Configuration page: the temperature tiles show 120 and 111 °F, three digits, so they scroll too.",
            base().with("mode", Json::Int(12)).with("unit", s("F")),
        ),
        page(
            "configuration.html",
            "Configuration screen",
            "Opened by the gear, which is pressed; the back icon, the gear again, Escape or Alt+Left return to Display. Rotation, Temperature unit (after Rotation), GPU (several GPUs, on Automatic: the line under it names the GPU it reads), the Music settings, and Start at boot, last (on). Rotation is 90°, and the preview turns with it. Each label sits beside its control, left-aligned, with the controls in one column; the Peak dots and Start at boot switches are 40 × 22.",
            config().with("rotation", Json::Int(1)),
        ),
        page(
            "configuration-gpu-list.html",
            "Configuration, GPU list open",
            "Three GPUs: an integrated one, a graphics card whose kind query failed, and one without readings. Automatic reads the integrated GPU, the first in the ranking, and the line under the dropdown names it. The card is listed as Unknown type with its note; the GPU without readings comes last.",
            config()
                .with(
                    "gpus",
                    Json::Array(vec![
                        integrated,
                        no_rule,
                        obj(&[
                            ("kind", s("Unknown type")),
                            ("name", s("Contoso Graphics R870")),
                            ("driver", s("exgpu")),
                            ("pci", s("0000:0a:00.0")),
                            ("queryFailed", Json::Bool(true)),
                        ]),
                    ]),
                )
                .with("gpuInUse", Json::Int(0))
                .with("open", s("gpu")),
        ),
        page(
            "configuration-theme-system.html",
            "Configuration, the system's light preference",
            "No Omarchy theme, and the system prefers light, so the window opens in OpenDeepCool's light theme. The switch is on, Dark or light is dimmed with Light pressed, and the line says \"Using OpenDeepCool's light theme, as your desktop prefers.\" Turn the switch off in the mockup: the toggles come alive and the theme follows the service's dark theme setting.",
            config()
                .with("omarchyTheme", Json::Bool(false))
                .with("colorScheme", s("light")),
        ),
        page(
            "configuration-theme-own.html",
            "Configuration, OpenDeepCool's own theme",
            "An Omarchy theme is found but isn't followed: the switch is off and sensitive, Light is pressed and both toggles are sensitive, and the window is in OpenDeepCool's light theme. Click Dark, and the window turns dark.",
            config()
                .with("useSystemTheme", Json::Bool(false))
                .with("darkTheme", Json::Bool(false)),
        ),
        page(
            "configuration-theme-not-found.html",
            "Configuration, no system theme found",
            "The switch is off and dimmed, the warning line says \"OpenDeepCool can't find the system's theme and is using its own.\", and Dark is pressed and both toggles are sensitive, in OpenDeepCool's dark theme.",
            config()
                .with("omarchyTheme", Json::Bool(false))
                .with("colorScheme", s("unsupported")),
        ),
        page(
            "first-start.html",
            "First start",
            "Defaults: Load CPU, rotation 0°, °C, peak dots on, bar fall moderate, peak fall slow, the GPU on Automatic.",
            base().with("mode", Json::Int(1)),
        ),
        page(
            "cooler-disconnected.html",
            "Cooler disconnected",
            "The service finds no cooler (not-found). Everything still works and is saved; the banner and the cooler's status item say the cooler isn't connected, and no line under the caption does.",
            base()
                .with("cooler", Json::Bool(false))
                .with("coolerState", s("not-found")),
        ),
        page(
            "cooler-no-access.html",
            "Cooler found, no access",
            "The cooler is plugged in, but the service can't open it (no-access), for example because its udev rule is missing or not applied yet. Its own red banner names the fix; nothing is disabled, and the preview still shows the service's picture, with no line under the caption.",
            base()
                .with("cooler", Json::Bool(false))
                .with("coolerState", s("no-access")),
        ),
        page(
            "cooler-write-failed.html",
            "Sending to the cooler fails",
            "The cooler opens, but writing the picture to it keeps failing (write-failed for 2 s or more; a shorter failure, such as while the cooler is unplugged, shows nothing). The banner says the service keeps trying, and the status item reads \"Cooler: sending fails\".",
            base()
                .with("cooler", Json::Bool(false))
                .with("coolerState", s("write-failed")),
        ),
        page(
            "service-not-running.html",
            "Service not running",
            "The display service is set to start at boot, but it failed (a crashing service waits to restart, which systemd shows as activating; both read as failed). No banner: the off screen is headed \"OpenDeepCool isn't running\", the status dot is red, and the switch reads off with a note saying what's missing and pointing to the journal. The gear is dimmed, and there is no Refresh (it belongs to the Display page). A flip on starts it again; the terminal commands sit under the card.",
            base()
                .with("service", Json::Bool(false))
                .with("serviceRun", s("failed")),
        ),
        page(
            "boot-off.html",
            "OpenDeepCool off, a new install",
            "The package turns nothing on at install, so the display service's unit is disabled and the service isn't running. No error banner: the window says OpenDeepCool is off, with the Start at boot switch and, under it, the terminal commands to turn it on. The gear is dimmed, and there is no Refresh (it belongs to the Display page). Flip the switch: the mockup skips the password dialog, then shows the normal window.",
            base()
                .with("service", Json::Bool(false))
                .with("boot", s("off")),
        ),
        page(
            "boot-changing.html",
            "Turning on",
            "Start at boot was flipped on. The desktop's own polkit dialog asks for the administrator's password (not drawn), then the program runs. The handle shows the flip, the trough systemd's state (still off), and the switch is insensitive until the program ends. Turning off looks the same the other way.",
            base()
                .with("service", Json::Bool(false))
                .with("boot", s("off"))
                .with("bootFlip", s("on")),
        ),
        page(
            "boot-not-changed.html",
            "Not turned on",
            "pkexec ended with 127: the password wasn't accepted, or no polkit agent runs in this session; the app doesn't tell them apart, since pkexec's messages are unverified. The switch is back at systemd's state, off, and the reason shows under it. A dismissed dialog (126) looks the same, with its own sentence.",
            base()
                .with("service", Json::Bool(false))
                .with("boot", s("off"))
                .with(
                    "bootFailed",
                    obj(&[("to", s("on")), ("exit", Json::Int(127))]),
                ),
        ),
        page(
            "unavailable-metric.html",
            "Unavailable metrics",
            "The chosen GPU reports no usage and CPU power can't be read: those tiles are disabled, and each group names the reason once.",
            base().with("mode", Json::Int(9)).with(
                "unavailable",
                obj(&[
                    ("gpu_usage", s("no reading for this GPU's driver")),
                    ("cpu_power", s("no permission to read it")),
                ]),
            ),
        ),
        page(
            "reading-lost.html",
            "The saved mode lost a reading",
            "GPU temperature was chosen, then its reading went away: the mode stays selected, the preview shows dashes like the cooler, and the reason shows under the preview.",
            base().with("mode", Json::Int(8)).with(
                "unavailable",
                obj(&[("gpu_temp", s("the sensor is no longer found"))]),
            ),
        ),
        page(
            "not-available.html",
            "Not available in this session",
            "A remote or inactive session. The service refuses it every read, change and preview frame, so the window is as small as the message needs, shows only this message, and the header keeps only the title and the close icon: no values, no preview, no controls, no gear, no Refresh. The app tries again, without polling, when its window becomes active and when the service's bus name gets a new owner.",
            base().with("notAvailable", Json::Bool(true)),
        ),
        page(
            "one-gpu.html",
            "One GPU",
            "With one GPU, dedicated or integrated, the configuration screen has no GPU control at all.",
            config().with("gpus", Json::Array(vec![dedicated])),
        ),
        page(
            "boot-partly.html",
            "Start at boot not fully on",
            "The display service runs, but someone only started it (sudo systemctl start): it doesn't start at boot, and the sound for Music is off. The switch is on only when OpenDeepCool is ready to use, so it reads off, with one note naming both missing parts. One flip finishes the setup.",
            config()
                .with("serviceEnabled", Json::Bool(false))
                .with("soundEnabled", Json::Bool(false))
                .with("soundRun", Json::Bool(false)),
        ),
        page(
            "boot-unknown.html",
            "Start at boot unknown",
            "Reading the units' state from systemd failed. The switch can't show on or off truthfully, so it's insensitive at off, with the reason and the commands to turn it on from a terminal.",
            config().with("boot", s("unknown")),
        ),
    ]
}

const TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en" data-theme="omarchy">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>OpenDeepCool mockup: @@TITLE@@</title>
<style>
@@CSS@@
</style>
</head>
<body>
<div class="mock-bar" aria-label="Mockup controls">
  <h1>Mockup: @@TITLE@@</h1>
  <p>@@DESC@@</p>
  <div class="row2">
    <span class="mock-theme"><span class="label">Theme:</span>
      <button data-t="omarchy" onclick="setTheme('omarchy')">Sample Omarchy theme (made-up colours)</button>
      <button data-t="dark" onclick="setTheme('dark')">Default theme, dark</button>
      <button data-t="light" onclick="setTheme('light')">Default theme, light</button></span>
  </div>
  <nav class="row2" style="margin-top:6px"><span class="label">Mockups:</span>@@NAV@@</nav>
  <p style="margin:6px 0 0;font-size:12px">Everything above the window is mockup chrome, not part of the app. Readings, chips and GPU names are made up. The window is drawn at its natural size (930 × 1007 px here); a browser window narrower or shorter than that scrolls the page inside it, as the app does on a small monitor. The preview and the tiles move in the app; a mockup shows one moment of them, with each strip at its home position and each loop at its first picture.</p>
</div>
<div id="win" class="window"></div>
<script>
const CONFIG = @@CONFIG@@;
@@JS@@
</script>
</body>
</html>
"#;

/// Every page's file name and text, with the mockups' style sheet `css` and script `js`. The
/// fields are filled in this order, each everywhere it appears, and none brings a `@@` of its own.
pub fn render(css: &str, js: &str) -> Vec<(&'static str, String)> {
    let pages = pages();
    let nav: String = pages
        .iter()
        .map(|p| format!(r#"<a href="{}">{}</a>"#, p.file, p.title))
        .collect();
    pages
        .iter()
        .map(|p| {
            let mut html = TEMPLATE.to_owned();
            for (field, value) in [
                ("TITLE", p.title),
                ("DESC", p.description),
                ("NAV", nav.as_str()),
                ("CONFIG", p.state.text().as_str()),
                ("CSS", css),
                ("JS", js),
            ] {
                html = html.replace(&format!("@@{field}@@"), value);
            }
            (p.file, html)
        })
        .collect()
}
