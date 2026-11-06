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

/// The fan inputs every page lists: made-up chips, labels and readings.
fn fans() -> Json {
    let fan = |chip: &str, label: &str, input: i64, rpm: i64| {
        obj(&[
            ("chip", s(chip)),
            ("label", s(label)),
            ("input", Json::Int(input)),
            ("rpm", Json::Int(rpm)),
        ])
    };
    Json::Array(vec![
        fan("acme6000", "", 1, 1180),
        fan("acme6000", "", 2, 2750),
        fan("acme6000", "", 3, 940),
        fan("acme6000", "", 4, 0),
        fan("acme6000", "", 5, 1620),
        fan("acme-ec", "Rear Fan", 1, 860),
    ])
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
/// connected, several GPUs on Automatic and both inputs chosen.
fn base() -> Json {
    obj(&[
        ("theme", s("omarchy")),
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
        ("fans", fans()),
        ("fan", Json::Int(0)),
        ("pump", Json::Int(1)),
        (
            "missing",
            obj(&[("fan", Json::Bool(false)), ("pump", Json::Bool(false))]),
        ),
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
            "The mode screen: unit at the top, 23 mode tiles in four groups, the live preview on the left. Click a tile: it applies at once (here: Temperature, CPU + GPU).",
            base(),
        ),
        page(
            "display-music.html",
            "Display screen, a Music mode",
            "Music, CPU temperature chosen; the status line adds the Music sound item. In the preview the spectrum is a still sample.",
            base().with("mode", Json::Int(18)),
        ),
        page(
            "display-scrolling.html",
            "Display screen, a value of 100 or more",
            "Power, CPU + GPU with GPU power at 156 W: the preview shows the strip at its home position (the app animates it at the cooler's 100 ms tick). Unit °F: the temperature tiles show °F.",
            base().with("mode", Json::Int(12)).with("unit", s("F")),
        ),
        page(
            "configuration.html",
            "Configuration screen",
            "Rotation, GPU (several GPUs, on Automatic: the line under it names the GPU it reads), the cooler's fan and pump inputs, the Music settings. Rotation is 90°, and the preview turns with it.",
            config().with("rotation", Json::Int(1)),
        ),
        page(
            "configuration-picker-open.html",
            "Configuration, fan picker open",
            "The cooler fans picker lists every fan input with its chip, its label when the driver has one, and its live RPM. None comes first.",
            config().with("open", s("fan")),
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
            "first-start.html",
            "First start",
            "Defaults: Load CPU, rotation 0°, °C, no fan inputs chosen, peak dots on, bar fall moderate, peak fall slow, the GPU on Automatic.",
            base()
                .with("mode", Json::Int(1))
                .with("fan", Json::Null)
                .with("pump", Json::Null),
        ),
        page(
            "cooler-disconnected.html",
            "Cooler disconnected",
            "The service finds no cooler (not-found). Everything still works and is saved; the banner and the status line say the cooler isn't connected.",
            base()
                .with("cooler", Json::Bool(false))
                .with("coolerState", s("not-found")),
        ),
        page(
            "cooler-no-access.html",
            "Cooler found, no access",
            "The cooler is plugged in, but the service can't open it (no-access), for example because its udev rule is missing or not applied yet. Its own red banner names the fix; nothing is disabled, and the preview still shows the service's picture.",
            base()
                .with("cooler", Json::Bool(false))
                .with("coolerState", s("no-access")),
        ),
        page(
            "cooler-write-failed.html",
            "Sending to the cooler fails",
            "The cooler opens, but writing the picture to it keeps failing (write-failed for 2 s or more; a shorter failure, such as while the cooler is unplugged, shows nothing). The banner says the service keeps trying.",
            base()
                .with("cooler", Json::Bool(false))
                .with("coolerState", s("write-failed")),
        ),
        page(
            "service-not-running.html",
            "Service not running",
            "The app can't read or change anything. It shows the start command (no start button: it's a privileged system action) and connects by itself once the service runs.",
            base().with("service", Json::Bool(false)),
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
            "A remote or inactive session. The service refuses it every read, change and preview frame, so the window shows only this message: no values, no preview, no controls, no Refresh. The app tries again, without polling, when its window becomes active and when the service's bus name gets a new owner.",
            base().with("notAvailable", Json::Bool(true)),
        ),
        page(
            "one-gpu.html",
            "One GPU",
            "With one GPU, dedicated or integrated, the configuration screen has no GPU control at all.",
            config().with("gpus", Json::Array(vec![dedicated])),
        ),
        page(
            "missing-input.html",
            "Saved pump input missing",
            "The saved pump input isn't found: its picker says so with a hint, the preview column says \"Input not found\", and nothing shows another input's RPM.",
            config().with(
                "missing",
                obj(&[("fan", Json::Bool(false)), ("pump", Json::Bool(true))]),
            ),
        ),
        page(
            "no-fan-sensors.html",
            "No fan sensors",
            "No fan inputs were found: both pickers are disabled with a note, and the preview column says so.",
            config()
                .with("fans", Json::Array(Vec::new()))
                .with("fan", Json::Null)
                .with("pump", Json::Null),
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
  <p style="margin:6px 0 0;font-size:12px">Everything above the window is mockup chrome, not part of the app. Readings, chips and GPU names are made up.</p>
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
