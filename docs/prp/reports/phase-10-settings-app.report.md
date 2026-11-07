# Phase 10 report: the settings app

**Plan:** [phase-10-settings-app.plan.md](../plans/phase-10-settings-app.plan.md)
**PRD phase:** 10 — Settings app
**Branch:** `phase-10-settings-app` (not pushed)
**Status:** signed off, 2026-11-07. Built unattended as the owner asked (2026-11-06); every choice made without the owner is in the [decisions log](phase-10-settings-app.decisions.md) (D1–D84). Back on 2026-11-07, the owner ran `odc-dev setup`, every real check passed, and the owner signed the phase off on that condition. CI passed on the branch on its second run (42403544852), after two fixes the first run (42390550617) called for (decisions log D85); `main` was fast-forwarded to the branch (no pull request).

## Outcome

| Task | Commit | What it built |
|---|---|---|
| Before Task 0 | `eb5edce` | The plan compared with what Phases 5–9 built (D1–D3); the decisions log |
| 0 | `aa8a25d` | `odc-mockups`, the mockups' generator in Rust, byte for byte the old one's output |
| 1 | `86c906e` | The design's open GTK points answered, and Automatic, the cooler's access states and a GPU that couldn't be asked designed (17 mockups) |
| 2 | `b74771f` | The service sends each frame watcher its frames, every fan input's speed, the GPU in use and the GPUs' IDs |
| 3 | `33112d0` | `odc-app`: the pure view model, the theme and the PCI names; a headless weston for the tests |
| 4 | `2939bdd` | The window: header bar, preview, banners, status and the D-Bus client |
| 5 | `dca035a` | The Display page: the unit and the 23 tiles |
| 6 | `09a605b` | The Configuration page: rotation, GPU, inputs and Music |
| 7 | `ffa0ac8` | The Omarchy theme followed live, and the default theme elsewhere |
| 8 | `eb7cfa8` | Layer 4: `odc-ui-e2e` drives the real app through AT-SPI in a headless weston against a fake service, 23 scenarios; five accessibility faults found and fixed |
| 9 | `7fbe38f`, `e3afd05` | The launcher entry and icon; the dev copy of the app; the real run against the dev service, split around Phase 7's test session (D83), with the app's frame log (D84) |
| 10 | `d94f483` | CI's `ui` job, the README and the PRD's footprint |

## Acceptance

| Criterion | Status |
|---|---|
| AC1 — Everything reachable | Met against the fake: every coverage item through the real app, each call exact at the fake (`--only app`, 24 scenarios). The app gained the AT-SPI actions this needed (D67, D69) |
| AC2 — Instant apply, the service's word | Met against the fake for every control, refusals of each kind, and the not-available state, and against the real service for every tile, the unit and each rotation; the switch and the scales follow D63 and D61 as built (D79) |
| AC3 — The preview is the cooler | Layer 3 (every frame of three modes drawn exactly), Task 2's frame tests, and the real run: every frame written to the cooler reached the app, at most 4 ms apart (D83, D84) |
| AC4 — Every state as designed | Met in layers 1 and 3, and through AT-SPI for every States row AT-SPI can read |
| AC5 — Theme | Met: layer 3 checks the colours; layer 4 Omarchy's switch and the default theme |
| AC6 — Contract additions | Met in layer 3 (Task 2), and `--only service` passed on the new dev copy, every mode |
| AC7 — Accessible and keyboard-usable | Every name of the design's table read through AT-SPI with GTK's role; the shortcuts' actions through AT-SPI and their keys in-process (D71) |
| AC8 — Real system, unattended | Met: the real app changed the real dev service, headless, and set the owner's settings back exactly. GLib finds the launcher entry; the launcher itself wasn't looked at |
| AC9 — Records | Met: AGENTS.md, the README, the PRD's footprint, the design's answered points, CI |

## Changes from the plan

Every change is in the decisions log, each pending the owner's review. The largest:

- **The real run (Task 9)** can't have the app change settings while the dev recording runs: a test session makes the service refuse every other caller's change. It's split into the app's own changes with no session, then a session where the harness changes the settings and the app's frames are compared with the recording (D83).
- **A third test-only flag**, `--frame-log`, for that comparison (D84).
- **What AT-SPI showed** that the in-process tests couldn't: no action chose a tile or an open dropdown row, buttons were named by their labels, the switcher's toggles read as usable while the service was down, and GTK names every label by its text. Each is fixed in the app (D67–D70, D72).
- **The test environment:** a private accessibility bus with the AT-SPI registry started by the harness (D74), and weston's desktop shell with a fake seat (D75); the fake service takes TOML (D76).
- **atspi on zbus 5**, with two `deny.toml` skips (D73).

## Findings worth carrying into the build

- **Idle CPU doubled** (0.25 % to 0.53 % of one core, no target set): new in this phase, and not yet measured apart. Two candidates: the D-Bus side's frame task wakes at every tick, 10 times a second, even with no frame watcher (`tell_frame_watchers` takes each posted frame), and the sensor thread reads every fan input's speed each second (D22). The plan meant the frames to cost nothing while nobody watches. Left for after the sign-off.

- GTK names a label by its own text, again at every change; a fixed name needs a named group around it (D72).
- A button with a text label is labelled by it, which wins over the accessible label the app sets (D68).
- `gtk_widget_set_sensitive` marks only the widget itself disabled for assistive technology (D70).
- A widget offers AT-SPI its own enabled actions without parameters, only while sensitive (D67).
- On Wayland, AT-SPI's key events don't reach an app (D71).
- at-spi2-core's launcher starts its bus through dbus-broker where installed, which starts the registry only through systemd; a registry started on demand outlives its bus (D74).

## Measurements

- **Baseline** (`--only baseline`, mode 4, 600 s, 2026-11-07): mean CPU 0.53 % of one core, peak RSS 7.5 MiB; 6,001 writes, p50 4.32 ms, p99 8.24 ms, max 8.97 ms; 597 snapshots, p50 0.85 ms, p99 2.34 ms, max 2.73 ms. Phase 9's was 0.25 % and 7.8 MiB: the CPU doubled (findings).
- **`--only service`** timing: 3,000 frames, mean 100.001 ms, p50 96.19 ms, p99 104.09 ms, max 104.21 ms.
- **`--only app`**: about 75 s for the fake run, about 15 s more for the real one.

## Validation

| Gate | Result |
|---|---|
| Format, lint | `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass |
| Layers 1 and 3, no display | `cargo test --workspace`: 609 passed, 0 failed, 6 ignored |
| Layer 3, headless weston | `cargo test -p odc-app --features weston-tests`: its five display binaries, 26 passed, 0 failed, 1 ignored (the killed-test child) |
| Layer 4, the fake service | `cargo run -p odc-e2e -- --only app --require weston`: 24 of 24 scenarios, about 75 s, repeated runs alike; nothing left running |
| Layer 2 and `--only display` | Passed for Task 5's change to `odc-render` (23 of 23 modes on the cooler) |
| Commit guard | Passed for every commit |
| The owner's setup, 2026-11-07 | `odc-dev check` exits 0 (36 manifest entries); `--only app --require dev-service,weston` 30 of 30; `--only service --require dev-service` 144 passed, modes 1–23; `--only baseline` passed (measurements) |
| CI | Run 42403544852 passed: the `test` job 609 passed, 6 ignored, with `cargo deny check`; the `ui` job layer 3 and 24 of 24 app scenarios, the real run not run there (no dev service); `privacy` passed. The first run, 42390550617, failed `cargo deny` (three unmaintained zbus 5 crates, now ignored with their reason) and one weston test (a killed weston left a zombie in the container); D85 |
