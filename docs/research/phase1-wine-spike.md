# Phase 1 — Wine reference spike: report

**Date:** 2026-10-22
**Verdict:** **PROVEN.** DeepCreative 1.2.14 runs under Wine on this PC, drives the real LP360, and every byte it sends can be recorded. On 2026-10-22 the owner confirmed on the physical display what the walkthrough showed: scrolling power values, music faces and spectrum, the custom test image, and rotation.

All observations below were made on this PC. Raw captures, screenshots and DeepCool files stay local in the git-ignored `research/` folder and are never committed. This report describes facts in our own words.

## Setup

| Item | Value |
|---|---|
| Wine | 11.17 (Arch `wine`, already installed), dedicated prefix, `winemenubuilder` disabled |
| Isolation | `bwrap --unshare-net`: loopback only, so no internet (no auto-update, no firmware download) |
| .NET | Wine Mono 11.3.0 from WineHQ, installed **in the prefix only**. DeepCreative's `electron-edge-js` won't start without `mscoree.dll` |
| App | `DeepCool.exe` run from the extracted installer payload; no installer, no Windows services registered |
| Sensor helper | `JZFSSensorBridgeServer.exe --console` (HWiNFO 8.22 based). Without it the LP page fails with `reading 'mainboard'` |
| Recording | LD_PRELOAD shim on the LP360's hidraw node only (other HID devices, e.g. the keyboard, are never logged). DeepCreative writes to the cooler from native code, so a JavaScript-level hook sees nothing |
| UI control | Chrome DevTools Protocol on the renderer, bridged out of the sandbox through a Unix socket. REA's Electron tools reject the `file:///Z:/` Wine URLs |

## Protocol facts observed

- **Report**: every message is a 64-byte HID output report:
  `0x10 | 0x68 0x01 0x05 | len | payload (len bytes) | checksum | 0x16 | zero padding`
  - checksum = sum of bytes 1 … 4+len, mod 256
  - every captured message passed this check
- **Frame**: payload `0x01` + 28 pixel bytes (len = 29). The 14×14 picture is one bit per pixel:
  - The first 14 bytes hold rows 1, 3, …, 13 for columns 1→14.
  - The last 14 bytes hold rows 2, 4, …, 14 for columns 14→1.
  - Within a byte, row pairs (1–2, 3–4, …, 13–14) map to bits `0x10 0x20 0x40 0x80 0x01 0x02 0x04`.
  - This matches the community notes (Nortank12/deepcool-digital-linux). The decoded frames show legible digits.
- **Rate**: DeepCreative sends a full frame every **100 ms (10/s)** in every mode, even when the picture doesn't change.
- **Other command**: payload `03 08` (len 2), sent once at start-up. Meaning unknown (open question).
- **Host-side features**: no other command was seen while changing modes, rotating or switching °C/°F. Rotation, scrolling and animation are all drawn by the PC; the cooler only shows the pixels it receives.

## Display options (complete list from the app's UI code, all exercised)

Mode numbers are DeepCreative's internal values (`lpStore.mode.value`).

| Personalization setting | Option | Mode | What the cooler shows |
|---|---|---|---|
| Load Mode | Separate: CPU / GPU / RAM usage | 1 / 2 / 3 | One value + `%`, centred, static |
| Load Mode | Combined: CPU+GPU / CPU+RAM / GPU+RAM usage | 4 / 5 / 6 | Two values + `%`, top and bottom rows, static |
| Temperature Guardian | Separate: CPU / GPU temperature | 7 / 8 | One value + `°C`, static. With °F the value + `°F` **scrolls** right-to-left |
| Temperature Guardian | Combined: CPU+GPU temperature | 9 | Two values + `°C`. Scrolls with °F |
| Power Guardian | Separate: CPU / GPU power | 10 / 11 | Value + `W`, **scrolling** marquee, 1 column per frame |
| Power Guardian | Combined: CPU+GPU power | 12 | Two values + `W`, scrolling |
| Music Mode | Full Screen | 13 | Live audio spectrum, 14 columns rising from the bottom row |
| Music Mode | Split: Emoji Animation + Spectrum (4 emojis) | 14 / 15 / 16 / 17 | Animated face (top) + spectrum (bottom) |
| Music Mode | Split: Temperature + Spectrum (CPU / GPU) | 18 / 19 | Temperature (top) + spectrum. Scrolls with °F |
| Music Mode | Split: Usage + Spectrum (CPU / GPU / RAM) | 20 / 21 / 22 | Usage (top) + spectrum |
| Creative Mode | Blank canvas / saved custom images | 100 / 100+index | Static 14×14 one-bit image drawn in the app's editor (add, edit, delete, import, export). A test image was saved, shown, and removed again |

**Other settings that affect the cooler:**
- **Screen Rotation** ("Rotate Screen" button) cycles 0°, 90°, 180°, 270°, done on the PC side. Rotating manually also sets a stored `gyroStatus` flag to 0.
- **Temperature Display** °C/°F, on the global Settings page.
- **Values**: usage and °C temperatures are clamped to 99. A leading zero is drawn as a blank.

**Not shown on the cooler:**
- CPU fan and pump RPM. The "CPU Fan I/O" and "Pump Fan I/O" settings only choose which motherboard header the app reads for its own device card. Under Wine both list only "N/A", because HWiNFO can't read the fan chip there.
- The stored settings `screenBrightness`, `screenStatus`, `noLoadScreen` and `rgbOptical` exist in the LP data model, but no LP-page control sets them (their meaning is an open question).

**Global Settings page**: auto-start, temperature unit, language, update check.

## Limits of the Wine run

- **Readings**: CPU usage and RAM usage come through. Temperatures, power and fan speeds read 0, because HWiNFO needs a Windows kernel driver. Layouts and animations are unaffected.
- **Mode switches**: after a switch, the previous content keeps scrolling out for about 1 s. The exact transition behaviour still needs to be specified.
- **Audio**: capture works under Wine. With music playing, Music Mode showed a live spectrum (every frame different).

## Open questions carried forward

- What does the start-up command `03 08` mean, and is it required?
- What exactly is the scroll and transition behaviour (buffer contents, speed, wrap)? To specify from captures in Phase 3.
- What do `gyroStatus`, `screenBrightness`, `screenStatus`, `noLoadScreen` and `rgbOptical` do for the LP360, if anything?
- How is the audio spectrum computed (bands, scaling, peak behaviour)?
