# Phase 7 report: service, settings and Load (the MVP)

**Plan:** [phase-7-service-settings-load-mode.plan.md](../plans/phase-7-service-settings-load-mode.plan.md)
**PRD phase:** 7 — Service, settings & Load (MVP)
**Branch:** `phase-7-service-settings-load-mode`
**Status:** complete, 2026-11-05: every task is committed and validated on the owner's PC, the owner signed the phase off, CI passed on the branch on its first run, and `main` was fast-forwarded to the branch (no pull request).

## Outcome

A sandboxed system service shows Load on the LP360 from boot, the same for every user. It keeps its settings in one file it alone writes, and takes changes over the system bus, each checked with polkit. The dev copy runs on the owner's PC. Its end-to-end checks drive the real service through every Load mode and each scenario type a run can drive, unattended, and compare every frame it writes with the expected ones.

| Built | What it is |
|---|---|
| `crates/odc-render` | Pure: glyphs, value rules, the static layout and the scrolling strip, the movement procedure, the mode table and the Load pictures (modes 1–6) |
| `crates/odc-ipc` | The D-Bus contract of `docs/spec/dbus.md`: names, state keys, errors and client proxies |
| `crates/odc-service` | Settings v1, the engine (one frame per 100 ms tick on absolute deadlines), the sensor and device threads, the D-Bus side (polkit for every call, watchers told one by one, the dev interface with its test session, override, release, durations), the first-readings journal line, and the `opendeepcoold` binary |
| `crates/odc-device` | The dev recording: each report with its tick and monotonic write time, gated to the start and the test sessions, rotated, and keeping the cooler's return after a released session |
| `packaging/` | `opendeepcool.service` with its sandbox, the D-Bus policy, the polkit actions, and the sysusers line that makes the locked account |
| `tools/dev` | `odc-dev setup` builds the dev binaries as the user and installs the dev copy with one `pkexec` step; `check` verifies the running service; `teardown` removes all of it from the manifest |
| `crates/odc-testkit` | The `dev-service` capability; the recording line parser |
| `crates/odc-e2e` | The direct checks' test session and hand-back check; the areas `load`, `service` and `baseline`; the attended tour |
| `tools/oracle`, `docs/spec/frames/` | The dash cases and every Load scenario, drawn from the spec's rules |

## Acceptance

| Criterion | Result | Evidence |
|---|---|---|
| AC1 Load as the spec draws it | Met | All 72 Load cases (modes 1–6 × 4 rotations × 3 value sets) match the frames in unit tests, `100%` over its whole cycle. Layer 2, local: 77 captured DeepCreative steps, 2,090 frames, 0 differing pixels; the steps where the rules differ on purpose are listed in mode 1's commit (fractional readings, 100 or more, one value below 10, which OpenDeepCool centres). Each mode's commit ran `--only load` on the cooler. Every metric has its own filler, so a wrong metric shows |
| AC2 Dashes | Met | `unavailable.txt` cases match; negative readings draw dashes (S19), on the real service too |
| AC3 Timing, resets and movement | Met | The 2,499 Load scenarios replay through the engine in all four rotations. On the real service: 3,000 frames, mean interval 100.000 ms, every one within 50–150 ms; mode, rotation, unit and Refresh resets and already-set choices pass as scenarios S14–S16, S21, S24 and S26 |
| AC4 Start-up and wake-up | Met | The start check: the instance's recording is `03 08`, then the saved mode's picture of its journalled first readings, 7.98 ms later. A fresh install starts in mode 1, rotation 0, Celsius. Wake-up: the device side real in Phase 5; logind's signal tested on a private bus (Task 7) |
| AC5 Settings | Met | One versioned TOML file, written atomically by the service only; an invalid file is moved aside and the defaults saved (Task 5 tests) |
| AC6 Authorization | Met | polkit for every call, active session only; watchers told one by one, re-checked every second, 8 per user and 32 in all; refused modes with their reasons (S23 on the real service); the dev interface only with `--dev`, for group `opendeepcool`, in three checks; arguments compared only with the service's own lists (Task 7 tests on a private bus) |
| AC7 Availability | Met | Persistently unavailable metrics and missing devices take their modes out of `AvailableModes` with a reason (Task 6 tests); S23's refusal of an unavailable mode on the real service |
| AC8 Sandboxed system service | Met | The locked `opendeepcool` account, enabled at boot; all eight optional settings kept (none dropped); `systemd-analyze security` rates the dev copy and the product unit 0.5 SAFE (below); a panic aborts (the panic test), the watchdog is pinged, restarts back off and never stop |
| AC9 Real cooler, unattended | Met | `--only service`: 39 of 39 checks, twice. One example of each Load mode, a rotated, a scrolling and a dash case, and the first variant of each scenario type, frame by frame; S17, S22 and S25 in process, each with its reason. The start check passed. `--only device` and `--only load` pass while the service runs and see it take the cooler back. The service reads its GPU's kind from the dev copy's helper. Without a dev service, the same plans run through the engine |
| AC10 Records | Met | README, AGENTS.md and the PRD (footprint names, baseline) match what was built; the dev manifest lists all 23 entries and `odc-dev check` passes |

`systemd-analyze security`, 2026-11-04 (the dev copy; the product unit, without the dev drop-in, has the same marks but `UMask=`):

```
✗ RootDirectory=/RootImage=          Service runs within the host's root directory                      0.1
✗ PrivateDevices=                    Service potentially has access to hardware devices                 0.2
✗ RestrictAddressFamilies=~AF_UNIX   Service may allocate local sockets                                 0.1
✗ ProcSubset=                        Service has full access to non-process /proc files (/proc subset=) 0.1
✗ PrivateUsers=                      Service has access to other users                                  0.2
✗ DeviceAllow=                       Service has a device ACL with some special devices: char-rtc:r char-hidraw:rw  0.1
✗ UMask=                             Files created by service are group-readable by default             0.1
→ Overall exposure level for opendeepcool.service: 0.5 SAFE
```

Why each stays: the service reads `/sys` and `/proc` of the host, so no root image; `PrivateDevices=` would hide the cooler's hidraw node, which `DeviceAllow=char-hidraw rw` lets through (`char-rtc:r` comes with `ProtectClock=`); the system bus needs `AF_UNIX`; `ProcSubset=pid` would hide `/proc/stat` and `/proc/meminfo`, which CPU Usage and Memory Load read; `PrivateUsers=` isn't in the plan's list, and Phase 11 checks the sandbox again; the `UMask=` mark is the dev drop-in's 0027, which lets the tests read the recording.

## Changes from the plan

Each was agreed with the owner, and the plan, its companion and the PRD say so where marked.

- **D-Bus test bus** (Task 7; owner, 2026-10-29): a private `dbus-daemon` per test instead of peer-to-peer pairs, because zbus sets a sender only from a bus; the fake polkit stays peer-to-peer. The real sleep watcher runs on the private bus, so there's no `SleepWatcher` fake. `Authorizer` and `Credentials` are async and boxed; `Credentials` also gives the user; `Status` keeps no settings; zbus's `p2p` feature, no new crate.
- **Internal failures** (Task 7; owner, 2026-11-04): a failure inside the service, such as a settings file it can't write, answers OpenDeepCool's own `…Error.Failed` with the reason (`dbus.md`, Errors). Task 7 first sent zbus's generic `org.freedesktop.zbus.Error`; before sign-off the owner chose the named error, so the app tells it apart from a lost connection, and `dbus.md`'s error list is now checked against the crate's.
- **polkit action owner and SIGTERM** (Task 8; owner, 2026-10-29): both actions are owned by `unix-user:opendeepcool`; `signal-hook` stops the service cleanly.
- **D-Bus catch-all deny** (Task 8; owner, 2026-11-04): method calls only, so logind's broadcast `PrepareForSleep` reaches the service whatever dbus-broker does with broadcasts.
- **Sandbox trial** (Task 8; owner, 2026-11-04): the eight optional settings tried together, to bisect only on a failure. None failed.
- **polkit reload** (Task 8, found 2026-11-04): polkitd didn't read the actions file in `/etc/polkit-1/actions`, a folder made after it started, until it reloaded; the dev install and removal now reload polkit.
- **Task 9's gaps** (owner, 2026-11-04): `Durations()` on the dev interface; the recording keeps the cooler's return after a released session; S17 through the start check, S18's standby and S22 in process, S25 real only with two GPUs; a first-readings journal line in product and dev; a made-up start fixture without a dev service, built in code.
- **GPU choice** (owner, 2026-11-04, before sign-off): `ChooseGpu("")` goes back to Automatic. At start, a GPU added or removed, or a chosen GPU that's gone, clears the choice, which is saved before the stale ranking is deleted; a choice that can't be saved holds for the run and keeps the ranking (`sensors.md`, GPU choice). Phase 6's `rank_is_stale` and `review_ranking` no longer take `user_has_chosen`, and `review_ranking` leaves a stale file for the service to delete.
- **S24's GPU part** (Task 9; owner, 2026-11-04): runs on the real service by choosing what's already set, the saved GPU or Automatic. Until Automatic existed it was left out, because a choice couldn't be taken back; the run now also puts the GPU choice back at its end.
- **S25 on a PC with two GPUs** (found 2026-11-04; owner): the run sent S25 to the real service on such a PC, but its real player had no step to switch GPUs, so S25 would have failed there. The player now switches to the other GPU with `ChooseGpu`, called as Refresh is, and chooses the user's GPU, or Automatic, back after the comparison; with one GPU, S25 passes in process, as before. This PC has one GPU, so the real path is **unverified** until a run on a PC with two. Also unverified: whether switching to an NVIDIA GPU, which loads NVML only then (`sensors.md`), still resets on the planned tick.
- **Baseline as an area** (Task 10): `odc-e2e --only baseline`, only when named, so later phases can repeat it.
- **The panic test** (owner, 2026-11-04): its child turns its own core dump off first, so a run no longer raises Omarchy's crash notification; every other crash still dumps.
- **Checked, no change** (owner's request): two rustc SIGSEGVs during builds on 2026-10-29 and 10-30 weren't tied to `panic = "abort"` and didn't come back in three clean release builds.

## Findings worth carrying into the build

- **polkit and new folders:** a policy file in a folder polkitd didn't have at start needs a reload. Phase 11's package installs into `/usr/share/polkit-1/actions`, which exists, but its hook can reload polkit the same way.
- **systemd re-applies `StateDirectoryMode=`** to a folder that exists: the GPU-rank helper creates `/var/lib/opendeepcool` with the default 0755, and the service's start makes it 0750 (dev) or 0700 (product).
- **Frame intervals:** the engine sends on exact deadlines, but a write takes 4–9 ms, so the intervals between write-end times spread about ±4 ms around 100 ms (p50 96.9 ms, p99 104.2 ms); the mean stays 100.000 ms.
- **No way back to an automatic GPU choice:** settled before sign-off (owner, 2026-11-04): Automatic and the GPU-change rule, under Changes from the plan. The settings app's mockups still show the earlier missing-GPU hint and no Automatic entry; the designer updates them before Phase 10's code.
- **The e2e service checks test the installed copy:** after a change to the service, `odc-dev setup` again (one password prompt) before `--only service`.
- **`syn` 2 stays allowed in `deny.toml`:** at sign-off, the newest nvml-wrapper (0.13.0, through thiserror 1 and wrapcenum-derive's darling 0.20) and the newest tracing-attributes (0.1.31, through zbus's tracing) still build with syn 2, and so does bindgen 0.72.1 in the dev tool.

## Validation

| Check | Result |
|---|---|
| `cargo build --workspace`, `cargo test --workspace`, clippy, fmt | Pass: 377 tests, 2 ignored (the local layer 2 runs) |
| Layer 2, local | 77 steps, 2,090 frames, 0 differing pixels (modes 1–6) |
| `odc-dev check` | Pass, 23 manifest entries |
| `busctl call … GetState` | `Mode` 1; the GPU's kind from the ranking |
| `cargo run -p odc-e2e -- --only device --require lp360,lp360-reconnect,dev-service` | 3 of 3, the service taking the cooler back about 300 ms after each session |
| `--only load --require lp360,dev-service` | 6 of 6, likewise |
| `--only service --require lp360,lp360-reconnect,dev-service` | 39 of 39, twice; 136 s plus the 5-minute timing run |
| `--only baseline --require dev-service` | Mode 4, 600 s: mean CPU 0.21 % of one core, peak RSS 5.4 MiB; writes p50 4.3 ms, p99 8.2 ms, max 9.0 ms; snapshots p50 0.8 ms, p99 2.1 ms, max 2.8 ms |
| `systemd-analyze security` | 0.5 SAFE, dev copy and product unit |
| `--attended` | Written, not run: it's the owner's optional tour |
| CI on the branch | Passed on its first run: the dependency audit (`cargo deny check`: advisories, bans, licences and sources; its first run on Phase 7's dependencies), format, lint, the workspace's tests as root (377, 2 ignored, as on the owner's PC), the udev rules and the privacy job. It passed again with `jdx/mise-action` moved to v5.2.6, the newest release at sign-off; `actions/checkout` v7.0.1 was already the newest |

## Commits

From `9cc513b` (the plan follows what Phases 5 and 6 built) to `1916741` (Task 10): 25 commits on the branch, one per task, per Load mode and per owner decision. Then this report (`007fb22`), the owner's three changes before sign-off (`4fbbb2c` `Error.Failed`, `1f11adb` the GPU choice's Automatic, `e305b97` S25 on a PC with two GPUs), the action pin at sign-off (`65765e6`) and the sign-off itself. `git log` from Phase 6's sign-off, `dc7ed7a`, to the sign-off lists them.
