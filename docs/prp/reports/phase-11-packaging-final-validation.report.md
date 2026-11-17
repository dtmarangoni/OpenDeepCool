# Phase 11 report: packaging and final validation

**Plan:** [phase-11-packaging-final-validation.plan.md](../plans/phase-11-packaging-final-validation.plan.md)
**PRD phase:** 11 — Packaging & final validation
**Branch:** `phase-11-packaging-final-validation` (not pushed)
**Status:** implemented, 2026-11-17; awaiting the owner's sign-off. Then the branch is pushed, CI must pass, and `main` is fast-forwarded to it (AGENTS.md, How to work).

## Outcome

OpenDeepCool is an Arch package that installs, turns on only when the user says so, upgrades, and uninstalls leaving nothing of itself, and it has passed the full unattended run against the real, installed package on the real cooler.

- **The package:** one PKGBUILD, built from a signed tag the same way from the AUR's four files and from a GitHub clone; an install script that prints the three turn-on commands and enables and starts nothing; a restart hook; Arch's 0BSD license for the AUR copy; `.SRCINFO`; no debug package. Every installed path, its owner and mode is in `packaging/FILES.md`, which a test checks against the repository.
- **Programs in `/usr/lib/opendeepcool/`**, off `PATH`: the display service, the GPU-rank helper, the sensor-detection helper, the sound helper and the Start at boot switch's root program. The app, its desktop entry and its icon are the only user-facing files.
- **Hardening:** every unit as tight as its job allows, each score capped by a test; overflow checks in the release build.
- **Start at boot:** a switch in the settings app, run through `pkexec` under its own polkit action, `auth_admin` at every change, for `on` and `off` only.
- **The package test** (`odc-pkg-test`, `odc-e2e --only package`): builds the package in a rootless throwaway root, installs and uninstalls it both ways, upgrades it, feeds it forged records, and requires the root to end as it started.
- **The owner's PC** moved from the dev copy to the package: teardown, install, the app's switch, the test access, and a real upgrade from release 1 to release 2.
- **The parity run:** the whole `odc-e2e` against the installed package, with the dev interface added for the run by one approved root step that removes it when the run ends. The parity checklist names the test that proves each of its rows, and a test keeps the names true.

## Acceptance

| AC | Result | Evidence |
|---|---|---|
| AC1 Installs both ways, on only when the user says so | Met | Package test: both ways build the same package and install every path of `FILES.md` with its mode; nothing is enabled; the message prints the commands; the switch's program turns both units on and off. On the owner's PC the app's switch turned the service on and the sound helper on for the session; after the install nothing was enabled or running |
| AC2 Uninstall leaves nothing | Met | Package test: the root equals its snapshot after install, an administrator's changes under OpenDeepCool's names and uninstall, both ways, but for the listed exceptions (the package cache, ldconfig's cache, pacman's log, the account files' backups); the upgrade keeps the links, account, record and settings; the lm-sensors setup is removed only when the record says the detection created it; four forged records removed nothing |
| AC3 The package touches only its own files | Met | Package test; on the owner's PC the teardown removed all 36 entries of the dev manifest before the install, and `pacman -Qkk` reported 0 altered files after both releases |
| AC4 Packaging rules | Met | Nothing in `/usr/local`, nothing in `/etc` from the package; namcap: 0 errors on the PKGBUILD and the built package, and none of its binary checks (RELRO, PIE, executable stack, text relocations, RPATH) fired; `.SRCINFO` current; the AUR files hold no binary and no identity |
| AC5 Parity at 100 % | Met | The full run passed (below); every row of the checklist is proven by a test, replaced by a row that is, or is the cooler's own behaviour |
| AC6 Test access and a real upgrade | Met | After the package replaced the dev copy the test access came back in the dev manifest; the upgrade restarted the service (main PID changed, under a second) and the sound helper (its main PID changed), the settings were identical, and the group access took effect after a reboot |
| AC7 Only committed, verifiable code is built | Met | The package test: a tree with uncommitted changes is refused; the AUR way's two builds are alike byte for byte; a signed tag by its key builds, an unsigned one and one signed by another key are refused |
| AC8 Least privilege | Met | Scores below, each capped by a test; the switch's program refuses everything but `on` and `off` (and its callers), its action asks for the password at every change |
| AC9 Records | Met | README (install, turn on and off, uninstall, installed on your system, reporting a crash), `FILES.md`, the PRD footprint and its table, AGENTS.md, the design and its mockups, the entry-points table, the plan's "As built" for each task and this report |

## The parity checklist

[`docs/spec/parity-checklist.md`](../../spec/parity-checklist.md) has 89 rows, and every one is at 100 %. A row is either proven by tests it names, replaced by a row that is, or the cooler's own behaviour, which the owner saw in Phase 3.

| Section | Rows | `test` | `replaced` | `device` |
|---|---|---|---|---|
| Protocol (P) | 10 | 9 | 0 | 1 |
| Display (D) | 41 | 34 | 7 | 0 |
| Music (M) | 21 | 21 | 0 | 0 |
| Sensors (S) | 11 | 11 | 0 | 0 |
| Settings (T) | 6 | 6 | 0 | 0 |
| **Total** | **89** | **81** | **7** | **1** |

- The 81 `test` rows link 239 tests, each a function in the file it links. `crates/odc-testkit/tests/parity_checklist.rs` fails for a row without a valid kind, a link whose file or function isn't there, or a pointer at a row that isn't a test row; it fails on each kind of damage tried, and runs in `cargo test --workspace`.
- The 7 `replaced` rows (D3, D5, D6, D7, D9, D28, D31) point at test rows (D36, D38, D39, D43) or, for D31, at the spec: DeepCreative's behaviour, not copied.
- The one `device` row (P8) is the cooler's own behaviour: the last picture stays for about 10 s, then it goes blank.

## The full run, against the installed package

`cargo run -p odc-e2e -- --dev-interface --require <every capability this PC can run real, weston included>`, 2026-11-17, about 20 minutes, from commit `9487428`. 229 checks passed and 0 failed.

| Area | Checks | Notes |
|---|---|---|
| Dev interface | 2 | The packaged service ran with `--dev` for the run, and the drop-in was removed and the service ran without it again afterwards |
| Device | 3 | Start-up frames, reconnect (gone after 0.1 s, back after 0.2 s, `03 08` first again), reset (a new conversation on the same connection); the service took the cooler back after about 300 ms each time |
| Display | 23 | One example of each of the 23 modes, straight on the cooler |
| GPU rank | 6 | The dedicated GPU, ranked twice alike, real; the other kinds through fakes |
| Sensors | 10 | CPU temperature, usage and power (through the group, idle and under load), Memory Load, the fan inputs against `sensors`, and the real GPU's three readings real; the other GPU kinds through fakes |
| Service | 145 | Every mode in each unit it shows, each scenario type, the start check, the access check, the timing run and the durations; 551 s of examples and scenarios |
| Music | 3 | Sound only in a Music mode; the capture of a test tone; the delay |
| App | 37 | 31 scenarios against the fake service and a fake systemd, and 6 against the real service, in a headless weston, through AT-SPI |

Real: the cooler and its reconnect switch, the CPU temperature, usage and power sources, the memory source, one dedicated GPU, the fan inputs, `sensors`, the GPU load tool, the installed service, PipeWire and weston. Fakes: the GPU kinds this PC doesn't have.

The package test ran on commit `b743362`: 32 of 32 passed in 10 minutes 6 seconds, container real.

## Security scores

`systemd-analyze security` on the installed units, 2026-11-17:

| Unit | Score | Ceiling a test pins |
|---|---|---|
| `opendeepcool.service` | 0.4 SAFE | 0.4 |
| `opendeepcool-gpu-rank.service` | 0.4 SAFE | 0.4 |
| `opendeepcool-sensors-detect.service` | 1.7 OK | its test's ceiling |
| `opendeepcool-sensors-detect-modules.service` | 1.3 OK | its test's ceiling |
| `opendeepcool-audio.service` (user) | 0.6 SAFE | 0.6 |

The two detection units run as root, which is why they sit higher; the module helper may only load two drivers, and the display service never waits for the detection.

## namcap

`namcap` on the PKGBUILD: no output. On the built package (release 2): **0 errors, 12 warnings**, all explained, and none from a binary check:

| Warning | Count | Why it stays |
|---|---|---|
| Unused shared library `ld-linux-x86-64.so.2` | 6 (one per program) | Rust programs list the dynamic loader as needed; nothing links against it by symbol. It is glibc's, which the package depends on |
| Dependency included, but may not be needed: `dbus`, `hwdata`, `lm_sensors`, `pipewire-audio`, `polkit`, `systemd` | 6 | Programs and data the package uses without linking a library: the system bus, the PCI names the app shows, `sensors-detect`, `pw-record`, polkit's checks and `pkexec`, and systemd's units and tools. Each is named in a `depends=` comment in the PKGBUILD |

## Reproducibility

The package test builds the AUR way twice and requires `opendeepcool-0.1.0-1-x86_64.pkg.tar.zst` alike byte for byte (176.7 s each). The GitHub way's build differs from the AUR way's only in `.BUILDINFO` and `.MTREE`, which record how and where it was built.

## Measurements

The baseline (`--dev-interface --only baseline`, the service in mode 4 for 600 s, with the dev recording on):

| Phase | Date | Mean CPU, one core | Peak RSS | Writes p50 / p99 | Snapshots p50 / p99 |
|---|---|---|---|---|---|
| 7 | 2026-11-04 | 0.21 % | 5.4 MiB | 4.3 / 8.2 ms | 0.8 / 2.1 ms |
| 9 | 2026-11-06 | 0.25 % | 7.8 MiB | | |
| 10 | 2026-11-07 | 0.53 % | 7.5 MiB | 4.32 / 8.24 ms | 0.85 / 2.34 ms |
| **11** | **2026-11-17** | **0.57 %** | **6.1 MiB** | **4.37 / 8.27 ms** | **1.23 / 3.84 ms** |

No target exists (the owner set none). The 3000-frame timing run's mean interval was 100.000 ms against the capture's 99.98–100.06 ms. The music delay, over 200 bursts: median 50.3 ms, 95th percentile 95.1 ms, 99th 100.4 ms, maximum 130.0 ms (target: at most 150 ms for 95 % of frames).

## Changes from the plan

- The plan was renumbered after the owner's packaging decisions of 2026-11-10 (Arch's guidelines, the most secure option, nothing on at install): the Start at boot switch became Task 3, and Task 8, the releases, moved to Phase 12.
- **The dev interface in the parity run** comes from one approved `pkexec` step that stays until the run ends and removes the drop-in itself (owner, 2026-11-17), where the plan said a drop guard removes it: a guard running as the owner can't remove a root-owned file. Added: the `--dev-interface` flag, the step `odc-e2e dev-interface-root` with its refusals, and a leftover report.
- No debug package (`options=('!debug')`, owner, 2026-11-16); no `packaging/arch/README` (the AUR lists none; owner, 2026-11-16); the account files' backups stay as the system wrote them after an uninstall (owner, 2026-11-16).
- The real upgrade ran after a reboot, not only a new login; the settings survived the reboot too.
- `odc-pkg-test --export` was added so the packages installed on the owner's PC are the ones the test validated.
- The check fixes now name the package's way (`--dev-interface`, `odc-dev setup --test-access`), and the service check gained the installed policy's access check.

## Findings worth carrying into the next phase

- **Idle CPU is still about double Phase 9's** (0.25 % to 0.53 % in Phase 10, 0.57 % now), and it was measured apart after the run (2026-11-17, the running service, exact thread run times, 60 s): the sensor thread 0.37 %, the device thread 0.07 %, the D-Bus thread 0.06 %, the engine thread 0.06 %, 0.56 % in all. Phase 10's two candidates are small: the frame task and the rest of the D-Bus work are about 0.06 %, and the fan inputs' speeds about 0.05 % (0.45 ms a second). Outside the service, one second's sensor work costs 0.15–0.18 % of a core, and the AMD GPU driver's reads are the most of it (two values cost 1.4 ms a second, because the driver asks the card's firmware); the other 0.2 % of the sensor thread is unexplained, and the sandbox accounts for only 10–15 % of a read's cost. A syscall trace as root would settle it (`strace` isn't installed). The suite now times the sensor thread's work against a ceiling and prints each thread's CPU in every baseline. The baselines run the dev service with its recording on. The service as shipped (no `--dev`, no drop-in) was measured the same way for 60 s on 2026-11-17, in the owner's own mode rather than the baseline's mode 4: 0.53 % of one core in all (sensor thread 0.35 %, device 0.06 %, D-Bus 0.06 %, engine 0.06 %), so the recording adds little to the 0.57 %.
- **At every upgrade the journal shows "Failed with result 'exit-code'"** for the sound helper at warning level: it ends with a failure by design (owner, 2026-11-10) so that `Restart=on-failure` starts the new program.
- **While a test run holds the cooler, the app shows "cooler not connected"** (the service's state is `released`). A normal user can't cause it, so it needs no design change; it cleared when the run ended.
- **`/usr/local/share/applications/mimeinfo.cache`** was left from the dev copy's teardown: a system hook wrote it into a folder the dev setup had created. The owner had it removed on 2026-11-17, with its empty folder, in one `pkexec` step; nothing under `/usr/local` is left from the dev copy. The owner asked that this never need a manual step again, so `odc-dev teardown` now removes the cache when it is all that is left in the folder it created.
- **The README's crash report no longer asks users to rebuild with debug information** (owner, 2026-11-17: debugging is the developer's job): it asks for the version, the journal's lines and, for a program killed without a panic message, the `coredumpctl list` line, and says not to attach a core dump or `coredumpctl info` (account name, host name and machine ID).
- **Unverified:** NVIDIA hardware has never run it, NVML in the sandbox included; S25's real path (switching between two GPUs) has never run on a PC with two.
- **The owner's review of the settings app** is next, after this phase and before Phase 12's plan (PRD, Phase 12 scope).

## Validation

| Gate | Result |
|---|---|
| `cargo build --workspace`, `cargo test --workspace` (the guard, at each commit), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | Pass; 688 tests passed, 0 failed, 6 ignored at the dev-interface commit |
| `cargo run -p odc-e2e -- --dev-interface --require …` | 229 of 229 passed |
| `cargo run -p odc-e2e -- --only package --require container` | 32 of 32 passed |
| `cargo run -p odc-e2e -- --only sensors --require …`, with the checks added after the run (CPU temperature under load, the sensor thread's cost) | Passed in 5 runs of the light load the owner asked for (2026-11-17: never towards 60 °C): 32–33 → 43–47 °C under the load, an independent 100 ms sampler saw at most 49 °C, 0 °C from `sensors` each time, CPU Usage 5.4 % against `top`'s 5.3–5.5 %; the sensor thread's work 1.3–1.5 ms of CPU a second |
| `cargo run -p odc-e2e -- --dev-interface --only baseline --require dev-service` | Passed (table above) |
| `namcap`, `shellcheck`, `systemd-analyze security` | 0 errors; shellcheck clean; scores above |
| Layer 2 differential tests | Not changed by this phase's code; Phase 5–9 results stand |
| CI (build, tests, `cargo deny`, `makepkg`, namcap, shellcheck, the UI job) | After the push; this report is completed with its result (AGENTS.md, How to work) |
