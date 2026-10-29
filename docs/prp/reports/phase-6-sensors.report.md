# Phase 6 report: sensors

**Plan:** [phase-6-sensors.plan.md](../plans/phase-6-sensors.plan.md)
**PRD phase:** 6 — Sensors
**Branch:** `phase-6-sensors`
**Status:** implemented, 2026-10-29. Every task is committed and validated on the owner's PC. It waits for the owner's sign-off, then the push, CI and the fast-forward of `main` (no pull request).

## Outcome

The display's seven readings come from built-in driver rules on any PC. Each is an exact fraction from its own source, or a reason it has none. A short-lived helper tells each GPU's kind, and a once-per-board detection loads the board's fan driver. Every test layer runs unattended: real where the PC has the device, through fakes where it hasn't.

| Built | What it is |
|---|---|
| `crates/odc-sensors` | The readings port and `Exact`. The registry binds one source per metric from the rules, which are data, and looks for late sources every 5 s for two minutes, then every 60 s. The rules: CPU temperature, usage and power, memory, and Gpu temperature, usage and power for amdgpu, i915, xe and NVIDIA (NVML). Also the GPU list, ranking and choice, the fan inputs, and a strict JSON reader |
| `crates/odc-gpu-rank` | The one-shot helper that asks each GPU's driver for its kind and writes `gpu-rank.json` atomically. It holds the only `unsafe` code, in `bindings.rs` (generated) and `ffi.rs`. It opens a render node only once it's the character device sysfs names for that GPU |
| `crates/odc-sensors-detect` | The root one-shot that runs `sensors-detect --auto` once per board, only without an lm-sensors setup or a readable fan input. It records yes/no facts, never a path, in its own directory, which must be its account's alone. It uses the standard library only |
| `crates/odc-testkit` | Fixtures for every rule; capabilities for the CPU, memory, GPUs, board fans and the tools, found with the product's own rules |
| `crates/odc-e2e` | The areas `gpu-rank` and `sensors` |
| `tools/bindings` | `odc-bindings`: writes the helper's kernel structures from the installed headers with bindgen; `--check` compares |
| `packaging/` | The udev rule's CPU power line; the GPU-rank and detection units |
| `tools/dev` | `odc-dev` sets up, checks and undoes the group's read access to the CPU power counters |

## Acceptance

| Criterion | Result | Evidence |
|---|---|---|
| AC1 The metric it names | Met | Each metric reads its rule's file; every fixture gives each metric a distinct value. Integrated AMD graphics never show Gpu power, and no CPU reading stands in for the GPU's (`gpu_metrics`, `cpu_temperature` tests) |
| AC2 Any PC | Met | The coretemp, k10temp, amdgpu, i915, xe and NVML rules pass on fake trees. Binding is by name, driver or PCI address; hwmon number changes are re-found. A missing driver gives a reason |
| AC3 Exact rates | Met | 23 busy jiffies of 40 read exactly 57.5 %; the first sample, and the first after `restart_intervals`, is `NotYet`. Energy counters wrap at their range. Property tests keep the share within 0–100 % |
| AC4 CPU power for the group only | Met | The udev rule's line matches package zones of `intel-rapl` only. On the owner's PC the counter is root:`opendeepcool` 0440 and its sub-zone stays root-only. The real check read CPU Power as a group member, and refuses to run its real part as root |
| AC5 GPU choice | Met | Dedicated, integrated, unknown, then by address; a saved choice wins and is kept while missing; a broken or stale ranking is deleted as the rules say (`gpu_choice` tests) |
| AC6 The helper boundary | Met | The workspace forbids `unsafe`; the helper denies it except in its two files. It runs only while no ranking exists (its unit's condition, tested). `bindings.rs` comes from bindgen and `--check` passes. Its layout checks are compile-time, so an edit fails the build. The three request numbers equal what the headers' own macros give, printed by a C program |
| AC7 Detection never overwrites | Met | Tests: an existing file of either kind is never touched (byte-compared); once per board; a new board runs again; a linked record or an open directory is refused. On the owner's PC, right after the one run, `lm_sensors.service` was enabled and active and the board's fan chip was readable, with no reboot or manual `modprobe`. The record read back as expected |
| AC8 Fan inputs | Met | Every input but a graphics card's is listed; a chosen one is re-found after its hwmon number changes; a missing one is `NoDevice` |
| AC9 Real where present | Met | The full layer 4 run probed every capability. All ten capabilities the owner's PC requires ran real, and the drivers it hasn't ran through fakes. The real readings agreed with `sensors -j`, `free -b` and `top` within the plan's tolerances, under the loads the check makes itself. A required capability that's absent, or one that's unusable, fails with its fix |
| AC10 Records | Met | `sensors.md` (rules, GPU choice, CPU power, detection, the CPU usage formula checked against `top`), the parity checklist, the PRD (decisions, both footprints, other coolers after v1) and AGENTS.md (layout, commands, rule 13) |

## Changes from the plan

- **Before Task 1** (owner, 2026-10-28): the plan was brought in line with what Phase 5 built (`55cec8d`). The registry's core moved into Task 2, and the late-source tests on fake trees moved to Tasks 4 and 8 (`3e0642f`). Two spec lines that named the owner's drivers were fixed first (`bbbb997`).
- **Licences:** ISC for libloading (Task 8) and BSD-3-Clause for bindgen in the dev tool (Task 9), both the owner's decisions.
- **bindgen 0.72, not 0.73:** 0.73 depends on `syn` 3, beside `nvml-wrapper`'s `syn` 2. The plan's risk row allowed aligning, and the workspace keeps one version of each crate.
- **The generated file** also holds `DRM_IOCTL_BASE`, which the request numbers are built from. bindgen's layout checks in this version are compile-time assertions.
- **The helper is a binary with a library** (`lib.rs`, `drivers.rs`, `fake.rs`), so its tests and `odc-e2e` share its code. The end-to-end run uses the library in-process. The program has `--sys` and `--dev` for its tests, as the detection has its flags.
- **The helper's unit** allows NVIDIA's device groups as `char-nvidia*`: systemd matches device groups with wildcards. It stays unverified until Phase 11 checks it on a PC with NVIDIA's driver.
- **`systemd-analyze verify`** names the helpers' binaries, not yet installed, rather than `opendeepcool.service`.
- **The detection's directory** must belong to the account that runs it: root in the unit and through `pkexec`, the test account in the tests. Other differences:
  - `--systemctl` names the program it asks;
  - the record starts with `"version": 1`;
  - a record that doesn't read is replaced, with a warning;
  - a `sensors-detect` that can't start leaves no record;
  - the fake `sensors-detect` and `systemctl` are the test binary itself.
- **The probe:**
  - its roots gain the tools' folder;
  - the testkit depends on `odc-sensors`, so the probe uses the product's rules;
  - a missing tool fails only a real check that needs it;
  - a GPU on NVIDIA's driver runs real only where NVML starts.
- **`odc_sensors::json` is public,** so the checks read `sensors -j` with each number as its text.
- **The GPU load** is `ffmpeg`'s `nlmeans_vulkan` filter on a Vulkan device derived from the GPU's render node, stopped by the check after 5 s. `ffmpeg`'s `-t` counts stream time, not wall time.
- **clang** was installed through the password dialog, as the owner asked. pacman also added `llvm-lit`, which wasn't in the list first shown; the PRD lists it.
- **AGENTS.md's layout** also gained `odc-manifest` and `odc-e2e`, missing since Phase 5.

## Findings worth carrying into the build

- **GPU kind:** the real query took no visible time on the owner's PC. The ranking was the same in two runs, and the GPU-rank file the service reads back is the one written.
- **Gpu usage** reached 99 % within half a second of the Vulkan load and fell to about 10 % within 5 s after it. Gpu power moves within a second: two reads in the same second differed by 2 W.
- **CPU power** rose from a few watts at idle to well over 100 W under the load, read through the group without root.
- **Detection:** in automatic mode `sensors-detect` skipped the SMBus adapters (default no). The board's fan chip appeared as soon as `lm_sensors.service` started.
- **`sensors -j`** names a PCI device's chip by libsensors' address, (domain << 16) + (bus << 8) + (device << 3) + function, in hexadecimal; checked for domain 0 only.
- **Render nodes** on Arch Linux are open to every account (mode 0666), so the helper needs no extra group.

## Validation

| Gate | Result |
|---|---|
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` | Clean |
| `cargo test --workspace` | Passes |
| `udevadm verify --resolve-names=never` on both rules files | 2 of 2 |
| `systemd-analyze verify` on both units | Only the missing binaries, as expected |
| `cargo run -p odc-dev -- check` | All nine items ok, the CPU power counter's group read included |
| `cargo run -p odc-bindings -- --check` | The committed file is what the installed headers give |
| Detection, once: `pkexec target/debug/odc-sensors-detect` | Ran 2026-10-29 with the owner's OK; every check of Task 10 passed |
| Layer 4, all areas, with the owner's `--require` list (`docs/local/`) | `test_images`, `reconnect`, `reset`, the six GPU-rank checks and the ten sensor checks passed; every required capability real |
| CI on the branch | Runs after the owner's sign-off and the push |

## Commits

On `phase-6-sensors`, from `55cec8d` (the plan follows Phase 5) to `af96873` (the layer 4 sensor checks): the plan and spec updates, then one commit for each task, each after its tests passed. Then this report.
