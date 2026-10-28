# Phase 5 report: device and protocol core

**Plan:** [phase-5-device-protocol-core.plan.md](../plans/phase-5-device-protocol-core.plan.md)
**PRD phase:** 5 — Device & protocol core
**Branch:** `phase-5-device-protocol-core`
**Status:** complete, 2026-10-28: CI passed on the branch after one fix, the owner signed the phase off, and `main` was fast-forwarded to the branch (no pull request).

## Outcome

Frames reach the LP360 byte-exact, without root, and keep reaching it after a reconnect. Every test layer runs unattended, the real-cooler tests included. Everything is Rust, the tests' tools included (owner, 2026-10-28; AGENTS.md rule 13).

| Built | What it is |
|---|---|
| `crates/odc-protocol` | `Frame` and `Rotation`; `Report`, which only `startup_report()` and `encode_frame()` make; `decode` |
| `crates/odc-device` | `DisplayPort` and `Link` with its `Reason`; `Lp360`, which owns the connection, sends `03 08` first on every connection, rescans after any failure and starts a new conversation on request; the hidraw adapter; `Recording`; the public `FakeConnector` |
| `crates/odc-testkit` | The expected-frame loader, the picture diff, the capture-log reader, sysfs lookups, fake trees and the hardware probe |
| `crates/odc-manifest` | The dev manifest: checked whole, then removed one entry at a time, last first; the package's hook program reuses it in Phase 11 |
| `crates/odc-e2e` | Layer 4: the real cooler where one is usable, fakes where none is attached |
| `tools/oracle` | The expected-frame generator, ported from the research script; it shares no code with the program |
| `tools/dev` | `odc-dev`: setup, check and teardown of the development setup, one `pkexec` step each way |
| `tools/guard` | `odc-guard`: the commit guard, its push-time check and CI's privacy check |
| `packaging/` | The product udev rule and the group file |
| `.github/workflows/ci.yml`, `deny.toml` | CI on Arch, the dependency audit |

## Acceptance

| Criterion | Result | Evidence |
|---|---|---|
| AC1 Byte-exact encoding | Met | The spec vectors pass in `cargo test`. All 210,221 captured DeepCreative messages (209,977 + 244; 15 start-ups, 210,206 frames) decode and re-encode to the identical 64 bytes. Flipping one bit of the bit table fails it with 199,813 mismatches |
| AC2 Only documented messages | Met | Two `compile_fail` doctests: raw bytes can't become a `Report`. The real cooler's recordings hold only start-ups and frames |
| AC3 Rotation | Met | Property tests pass. Mode 1's 8 static cases match in all four rotations; counter-clockwise fails in 28 pixels. On the cooler, `42%` upright and turned 90° matched |
| AC4 Reusable oracle | Met | All 22 files load: 284 cases, one per heading. A mismatch prints both pictures. The Rust oracle rebuilds every file byte for byte, as the Python version did, and a pixel changed by hand fails it |
| AC5 Start-up and reconnect | Met | Fakes: every scenario. Real cooler: gone within one tick, back within 0.2 s with `03 08` first; `reset_connection()` starts a new conversation on the same connection |
| AC6 Without root | Met | `odc-e2e` ran as a member of `opendeepcool`, with no `sudo`. Both rules pass `udevadm verify`. `odc-dev check` passed before and after the real reconnect |
| AC7 Unattended | Met | No layer needs a person. CI and the guard run layers 1 and 3; AGENTS.md has the local commands for layers 2 and 4 |
| AC8 Real where present | Met | With `--require lp360,lp360-reconnect`, both ran real on the owner's PC. In `cargo test`, the same checks pass against fakes; a required capability that's absent, or a cooler without access, fails |
| AC9 Records | Met | AGENTS.md (layout, commands, rule 13), the README's Development section, the PRD's dev footprint (the reconnect rule's row; installed and removed by `odc-dev`) |
| AC10 A guard that can't be fooled | Met | Throwaway-repository tests: another identity set six ways, a binary, a listed value, a local file and a listed message refused at commit; a `--no-verify` identity, a `--no-verify` listed value and a tag message refused at push; a clean commit and push accepted. CI's privacy job checks all 105 commits |
| AC11 One language | Met | Every program, test and tool is Rust; the other new files are git's one-line hook starters, CI's YAML and configuration |

## Changes from the plan

- **Rust everywhere** (owner, 2026-10-28): the expected-frame generator, the commit guard, the dev setup and the mockup generator are Rust programs. The plan, the PRD, AGENTS.md (rule 13) and the reviews were updated first, in `48b525b`. The Python generator stays local in `research/`.
- **The frame files' titles** take the settings app's names, for example `# Mode 4: Load · CPU + GPU` (owner, 2026-10-28). Only line 1 of each file changed.
- **Task 0's workspace** listed only `tools/*`: Cargo refuses a `crates/*` member while no `crates/` folder exists. Task 1 added it with the first crates, as the plan's end state has it.
- **`Reason`'s `errno` is an `Option`.** An error made in our own code, such as a short write, has no kernel number. P5-02's outcome asks for the path and the error, which this keeps.
- **The `compile_fail` doctests** came with `Report` in Task 4 instead of Task 6.
- **The capture counts:** 209,977 + 244 messages. The plan's 176,110 is from before the two research sessions of 2026-10-24; Task 5 allowed the current totals.
- **Shared helpers:** `HidrawConnector::find`, `odc_testkit::sysfs` and `odc_testkit::fake_tree` serve the dev tool, the probe and `odc-e2e`. Phase 6 can reuse the fake-tree builder.
- **The fake reconnect** uses `FakeConnector`, one of the two ways the plan named.
- **"A node that can't be opened"** is a folder in the tests: CI runs as root, which file modes don't stop.
- **Teardown also resets the USB switch** to `root:root 0644`: a re-trigger can't take back the reconnect rule's `chgrp` and `chmod`.
- **`cargo-deny`** runs in CI only (owner, 2026-10-28).
- **CI's privacy job** marks the checkout as safe for git before `odc-guard history`: the Arch container runs as root, but the checkout belongs to the runner's user.
- **Not run for real:** `odc-dev teardown`. The plan doesn't ask for it in this phase; its manifest check and its removal order are tested on fakes and in a temporary folder.

## Findings worth carrying into the build

- **Write time:** one 64-byte write took 2.9 ms at the median and 7.9 ms at most (41 writes). Phase 7 sizes its device thread and watchdog from it, keeping the kernel's 5 s bound for a stalled device (`protocol.md` Timing).
- **Reconnect:** switched off, the cooler fails the next write within one tick, and its USB device stays listed; switched on, it's writable within 0.2 s with its udev access applied again. The reconnect rule's access survives a reconnect, which the plan had left unverified (`protocol.md` Disconnect and reconnect).
- **The node opens write-only with close-on-exec,** proved from `/proc/self/fdinfo`.
- **A `cargo run` inside the guard doesn't wait on cargo's lock,** so the hook files stay one-line starters. Cargo commands the guard starts run without git's own variables, so a test that runs git works on its own repository.

## Validation

| Gate | Result |
|---|---|
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` | Clean |
| `cargo test --workspace` | Passes; the differential test is `#[ignore]`d there |
| `udevadm verify --resolve-names=never` on both rules files | 2 of 2 |
| Layer 2: `cargo test -p odc-protocol --test differential_captures -- --ignored` | 210,221 messages, 0 mismatches |
| Layer 4: `cargo run -p odc-e2e -- --only device --require lp360,lp360-reconnect` | test_images, reconnect and reset passed, both capabilities real |
| `cargo run -p odc-dev -- check` | All eight items ok, before and after the real reconnect |
| `cargo run -p odc-guard -- history` | 105 commits checked, no binary file |
| CI on the branch | Green on `e96f8f9`, both jobs: the test job (`cargo deny check`, fmt, clippy, `cargo test --workspace`, the udev rules) and the privacy job (107 commits, no binary file). The first run, on `4405d65`, failed in the privacy job only: git refused the checkout, which the container's root doesn't own ("dubious ownership"). `e96f8f9` marks it safe for git first |

## Commits

On `phase-5-device-protocol-core`, from `48b525b` (the plan follows the owner's Rust decision) to `4405d65` (CI and the Rust guard): one commit for each task, plus the titles change, each after its tests passed. Then `e96f8f9`, the privacy job's fix, and this report.
