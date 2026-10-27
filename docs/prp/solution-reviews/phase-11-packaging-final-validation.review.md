# Phase 11 review: packaging and final validation

**Review ID:** `phase-11-packaging-final-validation`
**Reviewed:** 2026-10-26, at commit `3ec34b7`
**Phase status in the PRD:** pending (planned)
**Inputs:** [Phase 11 plan](../plans/phase-11-packaging-final-validation.plan.md); PRD (Packaging, System Footprint, Decisions Log: Distribution, Service at install, Uninstall, Boot testing); the Phase 5–10 plans where they put files on the system; the systemd pacman hooks and `systemd-hook` script on an Arch system (read 2026-10-26)
**Overview:** [project-overview.review.md](project-overview.review.md)

## Verdict

The packaging design is ambitious in the right places:

- one PKGBUILD for the AUR and for a GitHub clone;
- an uninstall that removes everything, the account included;
- proof by a rootless throwaway root that is snapshotted before install and compared after uninstall, file by file, account by account, enable link by enable link;
- the final parity run against the installed package, not a dev build;
- DeepCreative's deletion from a list the owner confirms.

Three problems need fixing in the plan:

1. **The install order is wrong** (P11-01, High). pacman runs `post_install` before its post-transaction hooks, and those hooks create the account, load the D-Bus policy and apply the udev rules. Starting the service inside `post_install` fails on a fresh system, and depending on how long the hooks take, it can stay failed until the next boot.
2. **The removal script trusts a record that the service account can rewrite** (P11-02, High), and it deletes whatever the dev manifest lists (P11-03).
3. **Upgrades aren't specified** (P11-05). After an upgrade, the old binary keeps running.

The container test can't catch the first problem, because nothing runs there (P11-09).

## What the phase builds

- `packaging/arch/PKGBUILD`, `opendeepcool.install`, `.SRCINFO`, `packaging/FILES.md`.
- `odc-pkg-test`: `pacstrap -N` and `arch-chroot -N` as a regular user, a snapshot, both install ways, uninstall, and a comparison.
- The release (`v1.0.0`, the AUR repository), the owner's PC moved from the dev copy to the package, the full parity run inside a runtime `--dev` drop-in, and DeepCreative's deletion.

## Strengths

- **Clean uninstall is proven, not promised.** The snapshot covers paths, types, modes, owners, link targets, file hashes, the account databases and `/etc/systemd` links.
- **Rootless throwaway root.** No password prompt, so the test is unattended (rule 3).
- **What the package creates at run time is recorded** (the dev manifest, the detection record), so removal touches only what OpenDeepCool made.
- **Paths follow Arch's guidelines.** Nothing in `/usr/local` or `/etc`; service binaries in `/usr/lib/opendeepcool/`, off `PATH`.
- **The parity run uses the packaged binary**, with `--dev` added only by a runtime drop-in in `/run`.
- **The outward-facing steps are gated:** pushing to the AUR and tagging need the owner's explicit OK, and the AUR repository uses the project's no-reply identity.

## Analysis

### Install order

pacman runs a package's `post_install` before the transaction's post-transaction hooks. The Arch developer blog's 2026 article "On scripts and hooks" shows a transaction log with "Running the post_install function…" followed by ":: Running post-transaction hooks…". The hooks this plan relies on are all `When = PostTransaction`:

| Hook (Arch's systemd package) | Does | Needed by the service for |
|---|---|---|
| `20-systemd-sysusers.hook` | `systemd-sysusers` | `User=opendeepcool` |
| `30-systemd-daemon-reload-system.hook` | `systemctl daemon-reload` | the new unit file |
| `35-systemd-udev-reload.hook` | `udevadm control --reload`, then `udevadm trigger -c change` and `settle` | access to the LP360 node and the power counter |
| `dbus-reload.hook` | `systemctl try-reload-or-restart dbus.service` | owning the bus name under the new policy |

So when `post_install` runs `systemctl enable --now opendeepcool.service`:

- the account doesn't exist yet, so the start fails;
- `Restart=on-failure` with `RestartSec=1` retries;
- with the manager's default start limit (5 starts in 10 s), the unit ends `failed` if the hooks take longer than those retries. `udevadm settle` in the udev hook and the D-Bus reload, which sorts last, make that plausible.

Even when a retry lands after the account exists, the D-Bus policy or the udev trigger may still be missing: the service then can't own its name, and Power modes count as unavailable (P8-08). On the owner's PC it's worse, because `post_install` first deletes the dev account and group (step 1), so the old account disappears before the new one exists.

The PRD's promise, "the cooler shows the default mode at once, with no extra step", then depends on timing, and can fail until the next boot. Two fixes, either is enough:

- **(a) Make the system ready inside `post_install`, then start without blocking.** Skip each step in a chroot or on a system that isn't booted, as the `systemd-hook` script does.

  ```sh
  post_install() {
    # 1. dev-copy removal, with validated paths (P11-03)
    systemd-sysusers /usr/lib/sysusers.d/opendeepcool.conf
    systemctl daemon-reload
    systemctl try-reload-or-restart dbus.service
    udevadm control --reload
    udevadm trigger --action=change --subsystem-match=hidraw --subsystem-match=powercap
    systemctl enable opendeepcool.service
    systemctl --global enable opendeepcool-audio.service
    systemctl start --no-block opendeepcool.service
    # the message
  }
  ```

- **(b) Ship a post-transaction hook** whose file name sorts after the systemd ones (hooks run "in alphabetical order of their file name", alpm-hooks(5)), for example `/usr/share/libalpm/hooks/90-opendeepcool.hook`, triggered on install and upgrade of the unit file. It runs `systemctl try-restart`, or `start --no-block` on install, after the account, policy and rules exist. `post_install` then only enables.

(b) also solves upgrades (P11-05). Either way, `--no-block` keeps `pacman` from waiting on the one-shots: `sensors-detect` and `cuInit` have no start timeout (P6-03), and with `enable --now` a probe that hangs would hang the package transaction (P11-04).

### Root and the records it reads

`post_remove` runs as root and:

- removes `/etc/conf.d/lm_sensors` and the `lm_sensors.service` enable link "only if `sensors-detect.json` records that OpenDeepCool's detection created them";
- `post_install` removes "exactly what" `/var/lib/opendeepcool-dev/manifest` lists.

The detection record sits in `/var/lib/opendeepcool`, which belongs to the service account (P6-01). Unless the record moves to a root-only directory, the removal script must treat it as untrusted:

- read it with `O_NOFOLLOW`;
- accept only fixed booleans for the two known paths and "was enabled before";
- never take paths from it.

The dev manifest is written by root (`pkexec`), so its risk is corruption rather than attack. But this logic runs as root on every AUR user's PC, and a manifest with an empty line, `/`, `..` or a stray path would make it delete the wrong thing. Validate every entry against an allow-list (the dev copy's known `/etc` and `/usr/local` paths, the account, the group, the membership), refuse anything else, and log every action. Shipping development-only logic in the public package is the owner's decision (2026-10-24); hardening it is cheap.

### Upgrades

The plan says only "Upgrades run `post_upgrade` only. They never delete the account or the settings". It doesn't say:

- that the running service must be restarted, or the old binary keeps running until the next boot and may disagree with the new app's D-Bus contract;
- what happens to running audio helpers, which pick up the new binary at their next start;
- how a later version adds or renames a unit or an action.

A `post_upgrade` that runs the same "make ready" steps as (a), then `systemctl try-restart opendeepcool.service`, or the hook of (b), covers it. Also test that an upgrade keeps settings written by the previous version (already planned), including fields an older version didn't know (P7-12).

### What the container test can and can't prove

The rootless root has no running systemd, no D-Bus and no udev: the `systemd-hook` script skips its work in a chroot ("Skipped: Running in chroot"), and so would the fixed `post_install`. The test therefore proves the files, the account, the links and the clean removal, but **not**:

- that the service starts after install;
- the order of P11-01;
- that the D-Bus policy and the udev rule take effect.

Those are proven only on the owner's PC in Task 5, which already has history and isn't a fresh system. Name this gap in the report, and add a static test that the scriptlet runs sysusers, the reloads, the trigger and the enable in that order, before it starts anything.

The snapshot comparison will also meet expected noise from other hooks: icon, desktop-file, MIME and library caches regenerated during install and removal. Rather than ignoring whole files, compare them by content after both runs regenerate them, and list each allowed difference with its reason, as the plan intends.

### Build and source

- **Arch's Rust package guidelines:** `cargo fetch --locked` in `prepare()`, then `cargo build --frozen --release` and `cargo test --frozen`. The plan runs `cargo build --release --locked` in `build()`, which fetches during the build.
- **The minimum Rust version.** The workspace pins `rust-version = "1.99"`, and the package builds with the system's `cargo`. The package fails to build for anyone whose `rust` is older; keep the declared minimum at or below Arch's current `rust` at release.
- **Build only what ships.** A plain `cargo build` at the workspace root builds every member, including `odc-e2e`, `odc-ui-e2e` (with zbus 5 and `atspi`) and `odc-pkg-test`. Use `-p` for the shipped binaries, or set `default-members`.
- **`check()`.** "The hardware-free tests" must also be display-free (P10-04).
- **Source authenticity.** `source=` is the tag's auto-generated GitHub archive with a SHA-256. The checksum proves the bytes didn't change since the PKGBUILD was written; it doesn't prove who made them. A signed tag, built with `source=("git+https://…#tag=v1.0.0?signed")` and `validpgpkeys=()`, or a signed release asset, adds authenticity.
- **The working-tree build.** The GitHub way builds "from the working tree (a `pkgver()` from `git describe`)", while `source=` is the tag's archive, so one PKGBUILD has to switch between two sources. Specify the switch, and make sure a build never includes uncommitted changes without saying so.

### NVIDIA access

`DeviceAllow=` is documented in systemd.resource-control(5), not systemd.exec(5) as the plan says. It accepts `char-<group>` names from `/proc/devices`, and groups "not resolvable" at start "are not added", so group names are safe on PCs without NVIDIA. The exact group names (for example the ones NVIDIA's modules register in `/proc/devices`) are unverified and can't be checked on the owner's PC. Together with NVML in-process (P6-06) and CUDA in the rank helper (P6-07), say plainly in the report and the README that NVIDIA support is untested.

### Removal details

- **Deleting the system account** goes against common practice, because the UID may later be reused. The owner requires it, and removing the state directory takes away the only files the account owned. Keep it, and keep `userdel` after the state directory is gone.
- **Running audio helpers after uninstall.** `pre_remove` disables the user unit globally. Stopping every running helper is "unverified". If they can't be stopped, they lose their binary and keep retrying `AttachAudio` (P9-02) until each user logs out. Make the helper exit when the service has been absent for a while, or when its own executable is gone.
- **A runtime drop-in left by a crashed parity run** keeps `--dev` on until the next boot. The next `odc-e2e` start should check `/run/systemd/system/opendeepcool.service.d/` and clean it, with the owner's OK, since that's a privileged step.

### Parity at 100 %

Some checklist rows aren't OpenDeepCool behaviour:

- **P8:** the cooler blanks by itself;
- **D28, D31:** DeepCreative-only behaviour, deliberately not reproduced;
- **M10:** DeepCreative's swap.

The plan's mapping, "proven by a test or replaced by an owner decision", needs a third category, device behaviour that can't be tested unattended, so "100 %" means something precise (P3-06).

### Duration and outward-facing text

- **The full run** of every area against the installed package "takes hours". By the overview's estimate, it's days with the full scenario cross (overview Q1).
- **Release notes, the tag message and the AUR repository** are outside the commit guard (P2-04). Apply rule 10 to them explicitly.

## Findings

| ID | Severity | Category | Title |
|---|---|---|---|
| P11-01 | High | Correctness | `post_install` starts the service before pacman's hooks create its account, load its policy and apply its udev rules |
| P11-02 | High | Security | `post_remove` acts on a record the service account can rewrite |
| P11-03 | Medium | Safety | The dev-copy removal deletes whatever the manifest lists, as root, on every user's PC |
| P11-04 | Medium | Availability | `enable --now` waits inside `pacman` for one-shots that have no start timeout |
| P11-05 | Medium | Correctness | `post_upgrade` isn't specified: the old service keeps running after an upgrade |
| P11-06 | Low–Medium | Supply chain | The source is checked by checksum only, not authenticated |
| P11-07 | Low | Packaging | Follow the Rust package guidelines (`fetch --locked`, `--frozen`); keep the declared minimum Rust within Arch's |
| P11-08 | Medium | Coherence | `check()` would run display-bound tests (P10-04) |
| P11-09 | Medium | Test gap | The container can't show that the service starts after install, so the install order goes untested |
| P11-10 | Low | Testing | Caches regenerated by hooks will differ in the snapshot comparison |
| P11-11 | Note | Convention | Deleting the system account (owner's requirement; UID reuse risk mitigated by removing its files) |
| P11-12 | Low | Packaging | Declare `nvidia-utils` as an optional dependency, for NVML and CUDA |
| P11-13 | Low | Documentation, testability | `DeviceAllow=` lives in systemd.resource-control(5); NVIDIA group names unverified; NVIDIA untested |
| P11-14 | Low | Privacy | Rule 10 for the tag message, release notes and the AUR repository |
| P11-15 | Medium | Process | The full parity run's duration (overview Q1) |
| P11-16 | Low | Coherence | "Checklist at 100 %" needs a category for device behaviour |
| P11-17 | Low | Packaging | One PKGBUILD with two source modes: specify the switch |
| P11-18 | Low | Test robustness | A crashed parity run leaves the `--dev` drop-in in `/run` until the next boot |
| P11-19 | Medium | Process | Committing the oracle generator only now is late (P3-01) |
| P11-20 | Low–Medium | Robustness | Audio helpers keep running and retrying after uninstall |
| P11-21 | Low | Packaging | A plain workspace build compiles test-only crates for every user |

### P11-01: Install order

- **Evidence:** the hook files above (`When = PostTransaction`); the Arch developer blog's transaction log; plan, `.install` order (dev removal, `daemon-reload`, `enable --now`).
- **Recommendation:** fix (a) or (b) above, with a static order test, and a check in Task 5 that the service is active right after `pacman -U`, without a manual restart.
- **Outcome (2026-10-26):** accepted through OV-02, with fix (b) as the owner's choice. The units are enabled by vendor symlinks; install, upgrade and remove hooks do the rest; there's a static order test; Task 5 checks the service is active right after `pacman -U`.

### P11-02: The record `post_remove` trusts

- **Recommendation:** read the record from a root-only directory (P6-01); accept only fixed booleans; `O_NOFOLLOW`. In the container test, plant a forged record that names another file, and assert nothing else is removed.
- **Outcome (2026-10-26):** accepted through OV-01. The record is in `/var/lib/opendeepcool-detect/`, read only if root's alone, and only its yes/no fields count; Task 3 has the forged-record case.

### P11-03: Manifest-driven deletion

- **Recommendation:** an allow-list of the dev copy's paths and kinds; refuse and report anything else; never `rm -r` a path from the manifest; a test with a hostile manifest in the container.

### P11-04: Waiting inside `pacman`

- **Recommendation:** `--no-block`, plus the one-shot timeouts of P6-03.
- **Outcome (2026-10-26):** partly applied. The install hook starts the service with `--no-block` (OV-02); the one-shot timeouts wait for OV-12 (P6-03).

### P11-05: Upgrades

- **Recommendation:** specify `post_upgrade` (or the hook of fix (b)): make ready, `daemon-reload`, `try-restart` the service. Extend the container's upgrade check to assert the unit's enablement and the settings survive.
- **Outcome (2026-10-26):** partly applied. The upgrade hook restarts a running service on the new binary (OV-02); the rest waits for OV-18.

### P11-06: Source authenticity

- **Recommendation:** sign the release tag and build from it with `?signed` and `validpgpkeys`, or publish a signed release asset. Keep the SHA-256 as well.

### P11-07: Rust packaging

- **Recommendation:** `prepare()` with `cargo fetch --locked --target …`; `build()` and `check()` with `--frozen`; `CARGO_TARGET_DIR=target`; check the declared `rust-version` against Arch's `rust` before release.

### P11-08: `check()`

- **Recommendation:** see P10-04.

### P11-09: The untestable start

- **Recommendation:** state the gap; add the static order test; consider one more owner-approved step on the owner's PC: uninstall completely, then install fresh from the AUR, and confirm the service is active with no manual action. That's not a boot, so the no-boot rule holds.

### P11-10: Snapshot noise

- **Recommendation:** an allowed-difference list with reasons, built from the hooks that ran, comparing cache contents rather than ignoring files.

### P11-12: Optional dependency

- **Recommendation:** `optdepends=('nvidia-utils: GPU readings and GPU kind on NVIDIA cards')`.

### P11-13: NVIDIA access

- **Recommendation:** cite systemd.resource-control(5); use group names; document "untested on NVIDIA" in the README and the report.

### P11-14: Rule 10 beyond commits

- **Recommendation:** see P2-04. Run the sensitive-value check over the release notes and the AUR files before publishing.

### P11-15: The full run's duration

- **Outcome (2026-10-26):** decided through OV-04. Task 7 runs, on the real cooler, one example of each mode and of each test type, and the start check; every case runs in-process. The real-cooler part should take well under an hour (estimate).

### P11-17: Two source modes

- **Recommendation:** document the mechanism (for example a separate local build recipe, or a `git+file://` source that pins the checked-out commit), and make `makepkg` refuse to build a dirty tree unless asked.

### P11-18: A leftover drop-in

- **Recommendation:** check it at the next `odc-e2e` start and in the summary, and remove it with the owner's approval.

### P11-19: The oracle generator

- **Recommendation:** commit it before Phase 7 (P3-01). Task 8 then only removes the DeepCreative-reading parts that remain local.

### P11-20: Helpers after uninstall

- **Recommendation:** in the helper, exit when its executable is gone or the service has been absent for some minutes. In `pre_remove`, stop running helpers with `systemctl --user -M <user>@ stop` for each logged-in user, if Task 2 confirms it works.

### P11-21: Building test crates

- **Recommendation:** `cargo build --frozen --release -p odc-service -p odc-app -p odc-audio -p odc-gpu-rank -p odc-sensors-detect` (the shipped binaries' crates), or `default-members` in the workspace.

## Questions for the owner

1. Fix the install order through the scriptlet (a) or a shipped post-transaction hook (b) (P11-01)?
2. Add one "remove everything, install fresh from the AUR" check on the owner's PC (P11-09)?
3. Sign release tags (P11-06)?

## Sources

- The Arch systemd package's pacman hooks and `systemd-hook` script, read 2026-10-26: `20-systemd-sysusers.hook`, `30-systemd-daemon-reload-system.hook`, `35-systemd-udev-reload.hook` (runs `udevadm control --reload`, then `udevadm trigger -c change` and `udevadm settle`), `dbus-reload.hook`; all `When = PostTransaction`. The script skips in a chroot and when systemd isn't booted.
- Arch Linux developer blog, "On scripts and hooks" (2026), read 2026-10-26: a transaction log with `post_install` before the post-transaction hooks.
- alpm-hooks(5), local man page, read 2026-10-26: "Hooks are run in alphabetical order of their file name, where the ordering ignores the suffix."
- Arch developer manual, Rust package guidelines, read 2026-10-26.
- systemd.resource-control(5), read 2026-10-26: `DeviceAllow=` group names, and groups "not resolvable … are not added".
