import json, copy, pathlib, sys

HERE = pathlib.Path(__file__).parent
OUT = pathlib.Path(sys.argv[1])
CSS = (HERE / "app.css").read_text()
JS = (HERE / "app.js").read_text()

FANS = [  # made-up chips, labels and readings
    {"chip": "acme6000", "label": "", "input": 1, "rpm": 1180},
    {"chip": "acme6000", "label": "", "input": 2, "rpm": 2750},
    {"chip": "acme6000", "label": "", "input": 3, "rpm": 940},
    {"chip": "acme6000", "label": "", "input": 4, "rpm": 0},
    {"chip": "acme6000", "label": "", "input": 5, "rpm": 1620},
    {"chip": "acme-ec", "label": "Rear Fan", "input": 1, "rpm": 860},
]
GPUS = [  # made-up product names and PCI addresses
    {"kind": "Dedicated", "name": "Contoso Graphics R870", "driver": "exgpu", "pci": "0000:03:00.0"},
    {"kind": "Integrated", "name": "Fabrikam Integrated Graphics 7", "driver": "exgpu", "pci": "0000:00:02.0"},
    {"kind": "Unknown type", "name": "", "driver": "othergpu", "pci": "0000:07:00.0", "norule": True},
]
LIVE = {"cpu_usage": 23, "gpu_usage": 71, "ram": 47, "cpu_temp": 49, "gpu_temp": 44, "cpu_power": 41, "gpu_power": 156}

BASE = {
    "theme": "omarchy", "page": "display", "service": True, "cooler": True, "audio": True, "notAvailable": False,
    "mode": 9, "unit": "C", "rotation": 0, "gpus": GPUS, "gpu": 0, "gpuInUse": 0,
    "fans": FANS, "fan": 0, "pump": 1, "missing": {"fan": False, "pump": False},
    "unavailable": {}, "live": LIVE, "peaks": True, "barFall": 2, "peakFall": 1, "open": None,
}

def cfg(**kw):
    c = copy.deepcopy(BASE)
    for k, v in kw.items():
        c[k] = v
    return c

PAGES = [
    ("display.html", "Display screen", "The mode screen: unit at the top, 23 mode tiles in four groups, the live preview on the left. Click a tile: it applies at once (here: Temperature, CPU + GPU).",
     cfg()),
    ("display-music.html", "Display screen, a Music mode", "Music, CPU temperature chosen; the status line adds the Music sound item. In the preview the spectrum is a still sample.",
     cfg(mode=18)),
    ("display-scrolling.html", "Display screen, a value of 100 or more", "Power, CPU + GPU with GPU power at 156 W: the preview shows the strip at its home position (the app animates it at the cooler's 100 ms tick). Unit °F: the temperature tiles show °F.",
     cfg(mode=12, unit="F")),
    ("configuration.html", "Configuration screen", "Rotation, GPU (several GPUs), the cooler's fan and pump inputs, the Music settings. Rotation is 90°, and the preview turns with it.",
     cfg(page="config", rotation=1)),
    ("configuration-picker-open.html", "Configuration, fan picker open", "The cooler fans picker lists every fan input with its chip, its label when the driver has one, and its live RPM. None comes first.",
     cfg(page="config", open="fan")),
    ("first-start.html", "First start", "Defaults: Load CPU, rotation 0°, °C, no fan inputs chosen, peak dots on, bar fall moderate, peak fall slow, the dedicated GPU.",
     cfg(mode=1, fan=None, pump=None)),
    ("cooler-disconnected.html", "Cooler disconnected", "Everything still works and is saved; the banner and the status line say the cooler isn't connected.",
     cfg(cooler=False)),
    ("service-not-running.html", "Service not running", "The app can't read or change anything. It shows the start command (no start button: it's a privileged system action) and connects by itself once the service runs.",
     cfg(service=False)),
    ("unavailable-metric.html", "Unavailable metrics", "The chosen GPU reports no usage and CPU power can't be read: those tiles are disabled, and each group names the reason once.",
     cfg(mode=9, unavailable={"gpu_usage": "no reading for this GPU's driver", "cpu_power": "no permission to read it"})),
    ("reading-lost.html", "The saved mode lost a reading", "GPU temperature was chosen, then its reading went away: the mode stays selected, the preview shows dashes like the cooler, and the reason shows under the preview.",
     cfg(mode=8, unavailable={"gpu_temp": "the sensor is no longer found"})),
    ("not-available.html", "Not available in this session", "A remote or inactive session. The service refuses it every read, change and preview frame, so the window shows only this message: no values, no preview, no controls, no Refresh. The app tries again, without polling, when its window becomes active and when the service's bus name gets a new owner.",
     cfg(notAvailable=True)),
    ("one-gpu.html", "One GPU", "With one GPU, dedicated or integrated, the configuration screen has no GPU control at all.",
     cfg(page="config", gpus=[GPUS[0]])),
    ("missing-input-gpu.html", "Saved fan input and GPU missing", "The saved pump input and the saved GPU aren't found: both say so, the GPU in use is named, and nothing shows another input's RPM.",
     cfg(page="config", gpus=[dict(GPUS[0], missing=True), GPUS[1]], gpu=0, gpuInUse=1, missing={"fan": False, "pump": True})),
    ("no-fan-sensors.html", "No fan sensors", "No fan inputs were found: both pickers are disabled with a note, and the preview column says so.",
     cfg(page="config", fans=[], fan=None, pump=None)),
]

TEMPLATE = """<!DOCTYPE html>
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
"""

OUT.mkdir(parents=True, exist_ok=True)
nav = "".join(f'<a href="{f}">{t}</a>' for f, t, _, _ in PAGES)
for f, t, d, c in PAGES:
    html = TEMPLATE
    for k, v in (("TITLE", t), ("DESC", d), ("NAV", nav), ("CONFIG", json.dumps(c, ensure_ascii=False)), ("CSS", CSS), ("JS", JS)):
        html = html.replace("@@" + k + "@@", v)
    (OUT / f).write_text(html)
print("wrote", len(PAGES))
