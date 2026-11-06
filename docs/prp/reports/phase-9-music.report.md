# Phase 9 report: Music

**Plan:** [phase-9-music.plan.md](../plans/phase-9-music.plan.md)
**PRD phase:** 9 — Music
**Branch:** `phase-9-music`
**Status:** complete, 2026-11-06: every task is committed and validated on the owner's PC, the owner signed the phase off, CI passed on the branch on its second run, and `main` was fast-forwarded to the branch (no pull request).

## Outcome

The service shows Music on the LP360, from boot and in every rotation: the sound the PC plays on its default output as OpenDeepCool's 14-column spectrum, with peak dots that fall at Winamp's stops. It's drawn full screen (13), under one of four emoji animations (14–17), or under a value (18–22), and there's OpenDeepCool's own Dancer (23), who moves to the bass. A helper in each user's session hands the service that sound through PipeWire's `pw-record`, and only while a Music mode is shown. With mode 23, every mode from 1 to 23 is drawn.

| Built | What it is |
|---|---|
| `tools/oracle`, `docs/spec/frames/` | The spectrum oracle: 81 test signals and their expected rows at *H* 12 and 6 for every peaks setting and stop (`spectrum/signals.txt`, `expected.txt`); the Dancer's 11 poses and 97 pose sequences (`dancer.txt`); the scenarios of modes 13–23 (`scenarios/mode-013.txt` to `mode-023.txt`, 15,054), with sound and the Music settings |
| `crates/odc-render` | `spectrum.rs` (the analysis with realfft, the bars' and peaks' fall, `Stop`, `MusicSettings`), `emoji.rs` (the four loops), `dancer.rs` (the poses and their rules), `music.rs` (the Music frame: `spectrum_height`, `value_picture`, `with_spectrum`; the modes join `DRAWN` one commit each) |
| `crates/odc-ipc`, `crates/odc-service` | `AttachAudio()` (polkit `feed-audio`), the Music setters and state keys, the dev `OverrideAudio(h)`; `audio.rs`, which reads each pipe on its own thread and hears silence after 200 ms without samples; the engine drawing every Music mode |
| `crates/odc-audio` | The helper `opendeepcool-audio` and its user unit, enabled for every user, which runs `pw-record` only while the service wants sound |
| `crates/odc-e2e` | `--only music` (a test tone into a null sink, the real capture, the delay), the `pipewire` capability, Music plays through `OverrideAudio` with exact frames on the real service, `Plan::of_signal` and `Plan::of_dance`, and the tour's Music items |
| `crates/odc-render/tests`, `crates/odc-service/tests` | `matrix_music`, `scenarios_music`, `differential_music`, `spectrum`, `emoji`, `dancer`, `music`; the engine's replay of every mode's file, one thread per mode |

| Commit | |
|---|---|
| `963b386` | The plan follows what Phases 5–8 built, with two owner decisions |
| `7e64d47`, `e89a0f2` | Task 0: the research into tagged music, and the owner's decision to keep the default output |
| `6e3c7dc`, `4f10622`, `deac7f5`, `a451e35` | Task 1: S23's mode 24, the spectrum oracle, the Dancer's poses, the Music scenarios |
| `efdf9a0` | Task 2: the spectrum, the emoji loops, the Dancer and the Music frame in `odc-render` |
| `7a6f01b` | Task 3: the service takes sound |
| `35270fe` | Task 4: the helper and its user unit |
| `3215c7b`, `ee45df4`, `0ef3253` | Task 5: `--only music`, and the Music plays of the display, service and tour runs |
| `1f25ce8` | Task 6: mode 13 |
| `bd6f274` | Fix found by mode 14's real run: Refresh and another GPU (below) |
| `c17f019`, `526ff85`, `1391c27`, `e4c3eea` | Task 7: modes 14–17 |
| `58e624c` | Fix found by mode 17's real run: the sound's pipe (below) |
| `8b5ced6` | The Music tests' speed (below) |
| `234bf64`, `d4ac429`, `6565605`, `9549163`, `01d06e6` | Task 8: modes 18–22 |
| `adab576` | Fix found by mode 21's display run: S1's floor line (below) |
| `49eea95` | Task 9: mode 23 |
| `94a60bf` | The delay check spreads its bursts across the tick (owner, 2026-11-06; below) |
| `60e2084`, `37753e8` | The plans' HTML companions removed, with every mention of them in the instructions (owner, 2026-11-06) |
| `fc67b90` | Task 10: README, the PRD's footprint, this report |
| `a556d89` | `jdx/mise-action` pinned to v5.2.7, the newest release at sign-off |
| `6d75e0e` | Fix found by CI's first run: the helper's unit check (below) |
| this commit | The phase complete |

## Acceptance

| Criterion | Result | Evidence |
|---|---|---|
| AC1 Spectrum as the spec draws it | Met | Every one of the 4,860 sections of `expected.txt` (81 signals × *H* 12 and 6 × every peaks setting and stop) matches at every tick in `spectrum.rs`'s tests; `matrix_music` draws each at *H* 12 in mode 13 and at *H* 6 under every upright case of modes 14–22 on the whole frame; the engine's replay and `--only service`'s plays feed it sound and compare every frame |
| AC2 Top areas | Met | Mode 13's rows 1–2 stay off (`matrix_music`, `differential_music`); each emoji loop is exact from its first picture and restarts on every reset in the scenarios, at every loop position; each value area follows `display.md`, the cap and dashes included (S11, S12, S19, S20 for 18–22), and a property test shows rows 9–14 are always the spectrum's, whatever the value does; mode 18 shows the CPU Temperature in both units (`mode-018.txt`, S15); the Dancer follows all 97 `dancer.txt` sequences, standing still in silence |
| AC3 Settings | Met | `each_music_setter_validates_stores_and_tells_the_watchers`; S15b in every Music mode: a change shows on the next frame with no reset, and in other modes it's only saved; a v1 file without them gets the defaults (`a_file_from_before_the_music_settings_gets_their_defaults`) |
| AC4 Sound only from the active session, only in Music | Met | `an_allowed_attach_gets_a_pipe_whose_samples_reach_the_window`, `a_refused_or_unwanted_attach_gets_no_pipe`, `an_attached_caller_that_stops_passing_polkit_loses_its_pipe`, `a_newer_attach_replaces_the_older_and_leaving_the_bus_closes_the_pipe`, `leaving_music_closes_the_pipe_and_audio_wanted_follows_the_mode`; on the cooler, `music_only_in_music`: no helper node in mode 1, one in mode 13, gone again in mode 1, each within a second |
| AC5 The helper | Met | `odc-audio`'s tests: no recorder until sound is wanted and it stops within a second; a refused attach logged once and tried again only on a change or the timer; a new active session tries at once; a recorder that ends after a while starts again at once (a wake-up), one that fails at once only after the timer; the service leaving stops the recorder and its return is watched. The unit runs for people only (`ConditionUser=!@system`) and `systemd-analyze verify` accepts it. On this PC the owner's own helper connected in mode 13 (Task 6). Following a change of default output is WirePlumber's own rule for `stream.capture.sink`, not tested here |
| AC6 Movement and resets | Met | All 25,457 scenarios of modes 1–23 replay through the renderer and the engine in all four rotations with none skipped once mode 23 was drawn: every ordered switch among the 23 modes, both units for 18–19, every listed loop and cycle position; entering a mode with a spectrum from another family starts at 0, and the levels continue between Music modes |
| AC7 DeepCreative's captures | Met, with one recorded exception | 79 agreeing steps, the plan's recount, match in their top area: mode 13's 6 with rows 1–2 off; 24 emoji steps through `cycle_match` (modes 14–17, 6 each); 49 value steps, still frame for frame and mode 19's °F through `cycle_match` (table below). Every captured Music frame keeps the layout both draw, except in mode 18 °F, DeepCreative's quirk path: in its loud step 7 of 80 frames break it, now recorded in `music.md`, cause unverified; the layout check leaves those 13 steps out |
| AC8 Real cooler, unattended | Met | Before each mode's commit, `--only display --modes <n>` and, after `odc-dev setup`, `--only service --modes <n>`; then with no `--modes`, `--only display` 23 of 23 and `--only service` with every mode; `--only music`: the real capture and the delay, within its target (measurements below). Each run sets the owner's settings, the Music settings, the null sink and the owner's helper back however it ends (its restore and drop guards) |
| AC9 Records | Met | README (status, the helper, `busctl` examples, `--only music`), the PRD's System Footprint (the helper's binary, `pipewire-audio`, the dev copy of the helper), AGENTS.md's commands (from Tasks 5–6), this report |

**Layer 2, per mode** (2026-11-06):

| Mode | Compared in the top area | Frames | Skipped | Skip reasons |
|---|---|---|---|---|
| 13 | 6 (rows 1–2 off) | 190 | 0 | |
| 14, 15, 16, 17 | 6 each (the loop, `cycle_match`) | 351, 350, 350, 350 | 0 | |
| 18 | 6 (°C, still) | 190 | 21 | °F: DeepCreative draws the Gpu temperature (13), a value below 10 (4), 100 or more (4) |
| 19 | 19 (10 °C still, 9 °F scrolling) | 552 | 8 | rounding (4), a value below 10 (4) |
| 20 | 6 | 191 | 8 | a value below 10 (4), 100 or more (4) |
| 21 | 8 | 160 | 6 | a value below 10 (6) |
| 22 | 10 | 269 | 4 | a value below 10 (4) |

The layout check covered every step of each mode, 27 in mode 19 and 14 in each other value mode, mode 18's 13 °F steps aside.

## Parity checklist

| Item | Proof |
|---|---|
| M1 Spectrum layout | `differential_music`'s layout check over every captured Music frame (mode 18 °F aside, above); `matrix_music` and the spectrum properties: solid bars from a lit floor, at most *H* rows |
| M2 Silence is the floor line | Every case of modes 13–22 in silence; `--only music`'s capture: the floor line in silence on the real cooler |
| M3 OpenDeepCool's analysis | `analyse` against `expected.txt`'s 81 signals; a tone moving from 40 Hz to 16 kHz moves its highest column from 1 to 14 |
| M4 Full screen | `mode-013.txt` and its scenarios; 6 captured steps |
| M5–M8 Emoji 1–4 | Each loop's pictures and durations against `mode-014.txt`–`mode-017.txt` and a copy typed from `music.md`; DeepCreative's 6 steps each in order through `cycle_match` |
| M9 Mode 18 °C | 6 captured steps frame for frame; `mode-018.txt` |
| M10 Mode 18 °F | DeepCreative draws the Gpu temperature; OpenDeepCool draws the CPU Temperature (`mode-018.txt` °F cases, S15), so its 13 captured steps are skipped with that reason |
| M11, M12 Mode 19 in °C and °F | 10 still and 9 scrolling captured steps; `mode-019.txt`; °F of 99 or less stays still (D37) |
| M13–M15 Modes 20–22 | 6, 8 and 10 captured steps; their files and scenarios |
| M16 OpenDeepCool's spectrum | `expected.txt` at both heights, every stop, peaks on and off, in unit tests, the engine and the real service (`service_mode_13_signal`) |
| M17 The Music settings | The setters' test and S15b in every Music mode; stored and served over D-Bus (the app's screen is Phase 10's) |
| M18 Delay at most 150 ms for 95 % | `--only music`: 95th percentile 95.9 ms over 200 bursts spread across the tick (below) |
| M19 A reset restarts an emoji loop | S14, S16, S25 and S26 at every loop position in modes 14–17; on the real service S16 and S26 too |
| M20 The Dancer | `dancer.txt`'s 97 sequences through `Dancer`, and through the engine from the samples; on the real service `service_mode_23_signal`, poses 3 to 10 frame for frame |
| M21 Sound only in a Music mode | AC4's tests; `music_only_in_music` on the real cooler |

## Measurements

- **Delay** (`--only music`, 2026-11-06, after `94a60bf`): 200 bursts, median 51.1 ms, 95th percentile 95.9 ms, 99th 101.8 ms, maximum 115.8 ms; written to `pw-play` 5 ms at a time, at most 10 ms queued. A second run gave 50.2, 95.0, 99.9 and 104.0 ms. Before the fix the bursts all fell at one point of the tick: 68.3 ms (median) in Task 6 and 6.5 ms in Task 9, each a single point (below).
- **The null sink's node** (Task 5, read on this PC with the owner's OK): `pactl load-module module-null-sink sink_name=odc-e2e` makes one node, `node.name` `odc-e2e` (`media.class` `Audio/Sink`), whose monitor is its own ports; `pw-record --target odc-e2e` with `stream.capture.sink` captures it. `pw-play` needs `-P '{ "state.restore-props": false }'`, or WirePlumber gives it the volume it keeps for `pw-play`.
- **The helper's sandbox** (P9-04): the unit runs as written, with `NoNewPrivileges`, `PrivateTmp`, `ProtectSystem=strict`, `RestrictAddressFamilies=AF_UNIX` and `LockPersonality`; no setting had to go, and in mode 13 the owner's own helper captured and connected (Task 6). `RestrictRealtime` stays `no`. During an `--only music` run on 2026-11-06, both of the capture's `pw-record` threads (`pw-record` and `module-rt`) ran time-sharing in all 178 samples, never realtime; that run starts the helper outside its unit, so with no restriction at all. Whether capture works with `RestrictRealtime=yes` is **unverified**: no run had it set.
- **Baseline** (`--only baseline`, mode 4, 600 s, 2026-11-06): mean CPU 0.25 % of one core, peak RSS 7.8 MiB; 6,001 writes, p50 4.43 ms, p99 8.32 ms, max 8.48 ms; 600 snapshots, p50 0.91 ms, p99 2.18 ms, max 2.53 ms. Phase 8's were 0.21 % and 5.5 MiB; what the extra 2.3 MiB is wasn't measured.

## Changes from the plan

Each was agreed with the owner where marked; the plan and the PRD say so.

- **Before Task 0** (2026-11-05; AGENTS.md, How to work): the plan was compared with what Phases 5–8 built. S23 asks for mode 24, refused as outside the table, in place of a mode not drawn yet; Task 0 measured the installed players and nothing was installed (owner).
- **Task 0's outcome** (owner, 2026-11-05): of six players only Spotify tags its sound as music, so the source stays everything on the default output, as DeepCreative does on Windows.
- **Owner decisions during the tasks** (PRD Decisions Log): a peak fall change keeps a falling dot in place; the Dancer's reset tick runs its rules; sound that stops coming is silence after 200 ms; a missed tick moves nothing in Music; the dev recording stamps `CLOCK_MONOTONIC`; Task 4 was validated in mode 1, and its check of the owner's helper moved to Task 6.
- **Refresh and another GPU** (`bd6f274`, found by mode 14's real run; owner, 2026-11-06): the engine asked for their fresh snapshot only on the tick that took the call, so on the real service their reset landed a tick after the next frame, against `display.md` S25 and S26. The static pictures of Phases 7–8's real runs hid it, and the in-process replays answered at once. The D-Bus side now sends the new choices and a numbered request as the call arrives, and the engine counts an answer it took before the call reached it. The owner chose this over changing the spec, and one `odc-dev setup` served both this commit and mode 14's.
- **The sound's pipe** (`58e624c`, found by mode 17's real run): the real play closed its `OverrideAudio` pipe right after writing the last tick's samples, which the service takes as silence at once; mode 17's 16-tick scenarios were the first with a burst on their last tick.
- **The Music tests' speed** (`8b5ced6`): mode 18's ~2,800 scenarios took the debug replays from 12 to 46 s (odc-render) and from 22 to 58 s (the engine). The renderer's replay and `matrix_music` analyse each signal once; the engine analyses each tick itself, as the service does, so its replay runs one thread per mode. No build profile changed.
- **S1's floor line** (`adab576`, found by mode 21's display run): where a mode's cases have no scrolling value, the display run shows scenario S1's, and drew it without the floor line its frames have in silence.
- **The delay check** (`94a60bf`; owner, 2026-11-06): its bursts came every 400 ms from the run's start, a whole number of ticks, so each run measured one point of the tick. They now come 405 ms apart, each 5 ms later in the tick, at 20 points 10 times each; a compile-time check keeps the spacing a whole number of the 5 ms chunks that stamp the bursts, and never of ticks.
- **Mode 18 °F in DeepCreative** (2026-11-06): besides drawing the Gpu temperature, its loud captured step breaks the spectrum layout in 7 of 80 frames, recorded in `music.md`. The layout check leaves out mode 18's °F steps.
- **The tour** gains a Refresh mid-loop for each emoji mode, at the first tick from halfway on whose picture isn't the loop's first (tick 4 in modes 14–16, 6 in mode 17); each value mode's values of 100 or more over the burst train for a whole cycle (S14 into mode 13 at *k* 20); and the Dancer to `dancer.txt`'s rising beat train. No "demo burst train" is defined in the spec, so the owner chose that one (2026-11-06).

## Findings worth carrying into the build

- **Phase 10:** the app's Music screen offers the peak dots and the two falls (`settings.md`), and Status shows whether sound is coming in (`AudioConnected`). Every mode is drawn now, so `NotYetAvailable` is never returned; it stays in the D-Bus contract.
- **Phase 11:** the package depends on `pipewire-audio` for `pw-record`, and ships the helper's binary and user unit with its enable link; the upgrade and remove hooks restart and stop each logged-in user's helper (PRD, Decisions Log).
- **`RestrictRealtime=yes`** for the helper is untried; the evidence above suggests it wouldn't hurt capture.
- **S25's real path** (switching GPUs on a PC with two) is still **unverified**: this PC has one GPU, so S25 runs in process, now with the snapshot asked for at once.
- **`--attended`** now has the Music items above but has never run; it's the owner's optional tour.
- **Every mode's commit changed `odc-render`**, which the service links, so each needed `odc-dev setup` (one password prompt) before `--only service`.
- **CI's actions at sign-off:** `actions/checkout` v7.0.1 was still the newest; `jdx/mise-action` moved to v5.2.7, released that day. `deny.toml`'s `syn` 2 skip stays: the newest nvml-wrapper (0.13.0), wrapcenum-derive (0.4.1) and tracing-attributes (0.1.31) still build with it.

## Validation

| Check | Result |
|---|---|
| `cargo build --workspace`, `cargo test --workspace`, clippy, fmt | Pass: 520 tests, 6 ignored (the local layer 2 runs) |
| Layer 2, local | 79 agreeing Music steps match in their top area; every captured Music frame keeps the layout, mode 18 °F aside; Load's, Temperature's and Power's counts unchanged |
| `odc-dev check` | Pass, 28 manifest entries |
| `--only baseline --require dev-service` | Mode 4, 600 s: 0.25 % of one core, peak RSS 7.8 MiB (measurements above) |
| `cargo run -p odc-e2e -- --only display --require lp360,dev-service` | 23 of 23, modes 1–23 |
| `--only service --require dev-service` | Every check passed with all 23 modes: 551 s of examples and scenarios; timing p99 104.19 ms over 3,000 frames |
| Each mode's `--only service --modes <n>` | 13: as in Task 6; 14: passed after `bd6f274`; 15, 16, 17 (after `58e624c`), 18, 19 (35 checks), 20 (34), 21 (33), 22 (34), 23 (17): all passed |
| `--only music --require dev-service,pipewire` | Passed: only in Music, the capture (17 frames of the tone, the floor line in silence), the delay above |
| CI on the branch | Passed on its second run (2026-11-06): the dependency audit (`cargo deny check`: advisories, bans, licences and sources ok), format, lint, the workspace's tests as root (521, 6 ignored, as on the owner's PC), the expected-frame regeneration check, the udev rules and the privacy checks. The first run failed `systemd_accepts_the_unit`: `systemd-analyze verify --user` needs a user runtime folder, which CI's container has no session to give; the test now gives it its own (`6d75e0e`) |
