# Phase 8 report: Temperature and Power

**Plan:** [phase-8-temperature-power.plan.md](../plans/phase-8-temperature-power.plan.md)
**PRD phase:** 8 — Temperature & Power
**Branch:** `phase-8-temperature-power`
**Status:** built, 2026-11-05: every task is committed and validated on the owner's PC; the phase waits for the owner's sign-off, then CI on the branch and the fast-forward of `main`.

## Outcome

The service shows Temperature and Power on the LP360, from boot and in every rotation: the CPU's or the GPU's temperature in °C or °F, or the CPU's over the GPU's, and their power in W, alone or one over the other. Every value is drawn in full up to 9999 and capped there, static while it fits and scrolling by itself from 100 on. A temperature mode's picture restarts at home when the unit changes; a Power mode only saves it. Each mode was committed on its own, after its tests in all four layers, the run on the real cooler included.

| Built | What it is |
|---|---|
| `crates/odc-render` | The `°C`, `°F` and `W` glyphs; the °F rule, rounded once from the exact °C reading, and one cap for every number; `temperature.rs` (modes 7–9) and `power.rs` (modes 10–12), each mode joining its module's list of drawn modes in its own commit |
| `tools/oracle`, `docs/spec/frames/` | The scenario generator for modes 1–12, one file per mode (`scenarios/mode-001.txt` to `mode-012.txt`), modes 7–9 in °C and in °F; the unit in the scenario format; the dash cases of modes 7, 9, 10, 11 and 12 |
| `crates/odc-testkit` | `Scenario::unit` and `Reset::Unit`; `mode_scenarios(mode)`; `diff::cycle_match`, the ordered comparison of a captured scrolling run with a cycle |
| `crates/odc-e2e` | `--only display` (Phase 7's `--only load`, for every drawn mode), `--modes`, plans that carry their unit, the service checks and the tour for every drawn mode in each unit, and the first variant of each scenario type for each kind of mode |
| `crates/odc-render/tests`, `crates/odc-service/tests` | `matrix_temperature`, `scenarios_temperature`, `differential_temperature`, `matrix_power`, `scenarios_power`, `differential_power`; the shared scenario replay; the engine's replay of every mode's file; availability and D-Bus tests per mode |

| Commit | |
|---|---|
| `ba747fc` | The plan follows what Phases 5–7 built; `--only display` (owner, 2026-11-05) |
| `8df3fb1` | One scenario file per mode (owner, 2026-11-05) |
| `d559be3` | Task 1: the oracle's scenarios for modes 1–12 in each unit, and the dash cases |
| `82402b5` | Task 2: the glyphs, the °F rule and one cap |
| `3091d1c` | Task 3: `--only display`, `--modes`, `cycle_match` |
| `388dfa4`, `25492b9` | Tasks 4 and 5: modes 7 and 8 |
| `33cf61e`, `e715e3f` | Two fixes found by mode 9's real run (below) |
| `4b57420` | Task 6: mode 9 |
| `18cea74` | Task 7: mode 10 |
| `23da16a`, `1b62309` | Task 8: modes 11 and 12 |
| `d93be31` | Task 9: README and AGENTS.md |

## Acceptance

| Criterion | Result | Evidence |
|---|---|---|
| AC1 Temperature and Power as the spec draws them | Met | All 108 cases of `mode-007.txt`–`mode-012.txt` match in unit tests, Power's in both units, scrolling cases over their whole cycle. Every metric has its own filler, and a property test per kind checks that another metric's reading never changes the picture |
| AC2 Units | Met | The °F rule matches (9m + 162 500) div 5000, capped, at every millidegree from 0 to 9 999 000; 37.49 → `99°F`, 37.5 → `100°F`, 5536 → `9997°F`, 5537 → `9999°F`. S15 replays in every mode: a unit change resets to home in modes 7–9, and changes no frame in modes 1–6 and 10–12 (the oracle checks the frames with and without it) |
| AC3 Values in full, the cap, and dashes | Met | Values up to 9999 in full and capped past it (S8, S12, S20); negative and unavailable readings draw dashes with the symbol (`unavailable.txt`, S11, S19); CPU Power's first second `--W`, then `120W` (S17) |
| AC4 Movement and resets | Met | 10,403 scenarios replay through the renderer and through the engine in all four rotations, none skipped once modes 1–12 were drawn: every scenario S1–S26 that applies, both units where a temperature is shown, the change in the top value, the bottom one and both, every listed *k*, and all 132 ordered mode switches |
| AC5 DeepCreative's captures | Met | 70 captured steps, 2,512 frames, 0 differing pixels: two-digit °C frame for frame, three-digit °F and power through `cycle_match` over each window's second half. 122 steps skipped with their reasons (table below) |
| AC6 Selectable and available | Met | `SetMode` accepts 7–12 through `draws`; mode 13 is the one not yet available. `AvailableModes` drops each mode with the first reason of a lasting unavailable metric: no CPU temperature rule (7), no GPU or no rule (8), either temperature (9), the CPU energy counter denied (10, never `0W`), integrated or unknown-kind AMD GPU power, no rule, no GPU (11), either power (12) |
| AC7 Real cooler, unattended | Met | Before each mode's commit: `--only display --modes <n>` and, after updating the dev copy, `--only service --modes <n>` (31–35 checks each). Then, with no `--modes`: `--only display` 12 of 12 and `--only service` 102 of 102. Without a cooler or dev service, the same plans run through fakes and the engine (`cargo test`) |
| AC8 Records | Met | README (status, `busctl` examples, what the checks cover), AGENTS.md (layer 2 for `odc-render`, `--only display`, `--modes`), the PRD's row and Decisions Log, this report |

**Layer 2, per mode** (2026-11-05), the plan's recount of 2026-10-26 reproduced:

| Mode | Compared | Frames | Skipped | Skip reasons |
|---|---|---|---|---|
| 7 | 17 (8 °C, 9 °F) | 590 | 33 | rounding (12), 100 or more in °C (6), one digit (7), °F of 99 or less (8) |
| 8 | 24 (12 °C, 12 °F) | 772 | 17 | the same reasons |
| 9 | 5 (°F) | 180 | 28 | `°C` ends at column 14 in two-value modes (18), and the others |
| 10 | 6 | 270 | 23 | rounding (1), 1000 W or more (8), 99 W or less (14) |
| 11 | 12 | 470 | 11 | the same reasons |
| 12 | 6 | 230 | 10 | the same reasons |

The plan counted 32 skipped steps for mode 7; the 33rd is a one-off blank display the research script left out by name, which "°F of 99 or less" skips here.

## Parity checklist

| Item | Proof |
|---|---|
| D2 Glyphs | `glyphs_and_values`: each symbol equals the spec's picture, typed apart |
| D5, D6, D7 DeepCreative's rounding and limits | Replaced by D36 (no caps) and the nearest rule (owner, 2026-10-26): `glyphs_and_values` and the °F property; the differential tests skip the steps where DeepCreative's rules differ, each with its reason |
| D10 Scrolling | `cycle_match`: every compared scrolling step is consecutive windows of OpenDeepCool's 21- or 25-column cycle, two values moving together |
| D17–D25 Modes 7–12 | The matrix (108 cases) and the differential counts above; D18, D22 and D23's DeepCreative scrolling at 99 or less is replaced by D37 |
| D30 Rotation | Every case and scenario in the four rotations; the differential steps in their captured rotation |
| D35–D41 OpenDeepCool's own rules | The scenarios: S4 and S13 (D35), S8, S12 and S20 (D36), S1–S6 (D37), S14 and S15 (D38), S9, S10 and the alignment tests (D39), S16, S19 and S10b (D40), S15, S24–S26 (D41) |

## Changes from the plan

Each was agreed with the owner where marked; the plan, its companion and the PRD say so.

- **Before Task 1** (2026-11-05; AGENTS.md, How to work): the plan was compared with what Phases 5–7 built and corrected: `number` was already the °C and power rule, `picture.rs` needed no change, `SetMode` and availability already shared `draws`, the scenario format had no unit, the generator was Load-only, `load.txt`'s "not yet available" example was mode 9, and `odc-e2e` assumed Load in more files than listed.
- **`--only display`** (owner, 2026-11-05): Phase 7's direct check covers every drawn mode, renamed; each mode's commit ran it beside `--only service --modes <n>`.
- **One scenario file per mode** (owner, 2026-11-05): in one file per kind, `load.txt` came to 2.87 MB and `temperature.txt` to 2.72 MB, over the commit guard's 2 MiB limit, which stays. Per mode the largest is 1.47 MB (mode 9); 10,403 scenarios in 7.5 MB, from 1.8 MB for `load.txt`. Phase 9's Music scenarios follow (`mode-013.txt` to `mode-023.txt`).
- **Tick 0 names all seven metrics**, since the oracle shares no code with the tests' fillers; **S2 from one digit and S9 have no °F variant**: no reading draws one digit in °F.
- **S23's unavailable mode** (`33cf61e`, found by mode 9's real run): the next mode of the same kind didn't work for mode 9 (→ 7) and mode 12 (→ 10), whose only metric the scenario's own mode shows; it's now the first mode, same kind first, with a metric of its own: modes 9 and 12 refuse mode 1. Only 18 S23 lines changed, no frame. And a scenario asking for a mode refused as unavailable waits, in `odc-e2e`, until that mode is drawn.
- **The start check's race** (`e715e3f`, found by mode 9's real run): a run started within a second of the service's restart read the recording before its first flush. The check now waits up to the flush interval. A fix to Phase 7's harness, proven on the real service by the runs of modes 10–12, each started right after `odc-dev setup`.
- **One example of each test type per kind:** a unit change resets a Temperature mode and is only saved in Load or Power, so the service checks and the tour take the first variant of each scenario type for each kind (Load's checks keep Phase 7's names; `service_S15_temperature`, `service_S15_power`). A listed mode without a dash or scrolling case leaves that example out.
- **Engine availability tests** list the modes the readings allow among those drawn, so each mode's commit didn't edit them again.

## Findings worth carrying into the build

- **Phase 9:** mode 13 is the "not yet available" example of S23 and of the D-Bus test; when Music draws it, its plan check moves or removes them. `--modes 13` is refused until then. Music's scenario files are per mode, under the 2 MiB limit.
- **The e2e service checks test the installed copy:** every mode's commit changed `odc-render`, which the service links, so each needed `odc-dev setup` (one password prompt) before `--only service`.
- **S25's real path** (switching GPUs on a PC with two) is still **unverified**: this PC has one GPU, so S25 runs in process.
- **`--attended`** covers modes 1–12 and each kind's scenario types, but has never run; it's the owner's optional tour.

## Validation

| Check | Result |
|---|---|
| `cargo build --workspace`, `cargo test --workspace`, clippy, fmt | Pass: 420 tests, 4 ignored (the local layer 2 runs) |
| Layer 2, local | 70 steps, 2,512 frames, 0 differing pixels (modes 7–12); Load's 77 steps unchanged |
| `odc-dev check` | Pass, 23 manifest entries |
| `cargo run -p odc-e2e -- --only display --require lp360,dev-service` | 12 of 12, modes 1–12, 78 s |
| `--only service --require dev-service` | 102 of 102: 15 mode examples (12 modes, °F for 7–9), rotated, scrolling and dash examples, 81 scenario types (27 per kind; S17, S22 and S25 in process with their reasons), the start check, durations and the timing run (3,000 frames, mean 99.998 ms, p50 96.26 ms, p99 104.16 ms); 700 s |
| Each mode's `--only service --modes <n>` | 7: 32, 8: 31, 9: 35, 10: 31, 11: 31, 12: 34, all passed |
| `--only baseline --require dev-service` | Mode 4, 600 s: mean CPU 0.21 % of one core, peak RSS 5.5 MiB; writes p50 4.31 ms, p99 8.22 ms, max 8.67 ms; snapshots p50 0.87 ms, p99 2.17 ms, max 2.80 ms. Phase 7's was 0.21 % and 5.4 MiB |
| CI on the branch | After the owner's sign-off |
