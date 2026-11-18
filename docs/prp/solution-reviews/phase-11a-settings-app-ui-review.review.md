# Phase 11a review: the settings app's UI review

**Review ID:** `phase-11a-settings-app-ui-review`
**Reviewed:** 2026-11-18, at commit `1a6a4e9` (Phase 11 complete), on the plan as first written the same day
**Phase status in the PRD:** pending (planned)
**Inputs:** [Phase 11a plan](../plans/phase-11a-settings-app-ui-review.plan.md); the PRD (Phase 11a and 12, Decisions Log, System Footprint, Testing Strategy); `docs/design/settings-app.md`; `docs/spec/{dbus,display,music,sensors,settings,parity-checklist}.md`; the code the plan names, read at that commit; [the Phase 10 review](phase-10-settings-app.review.md) and [the overview](project-overview.review.md) (entry points)
**Overview:** [project-overview.review.md](project-overview.review.md)

This review was written in the same session as the plan, so five of its findings (P11A-01 to P11A-05) were already folded into the plan while it was being written; their Outcome lines say so. The owner's cross-check is the independent step.

## Verdict

The plan is sound and well ordered. It removes before it builds, keeps every commit green (the app stops using fans in Task 3, before the service stops publishing them in Task 4), and reaches the owner's goals with little new machinery:

- the live tiles reuse `odc-render` as it is, and the only new things are one state key and one signal on a path the service already guards;
- the fans' removal is deletion along a seam that was already small;
- both centring changes are one rule each, in two places that are kept independent on purpose (the program and the oracle).

One problem would have failed the plan's own gate: Task 4 deletes tests that a parity row links, and a Rust test fails when a linked test is missing (P11A-01, High, folded in). The remaining open points are decisions for the owner, not defects: whether auto-moving tiles need a pause control (P11A-06), how many system steps the development PC goes through (P11A-07), and where the per-user theme file stands against rule 7 (P11A-08).

## What the phase builds

- Removal of fans and pump from the product (and of the sensor detection, which exists for their chip), with their facts kept in one spec file.
- The app's header, pages, window size and a Theme card.
- Two additions to the service's contract: the `Readings` state key and the `Levels` signal, both for frame watchers only.
- Live tiles drawn by `odc-render` in the app.
- Two cooler pictures changed: pairs of values centred, and the Dancer one row up.
- Closing steps: the designer's review, a code review, a new build and the owner's review.

## Strengths

- **One drawing code.** A tile is the cooler's picture for its mode by construction, and a test compares it with what the service writes (Task 13's e2e check, Task 14's matrix).
- **The contract grows on a guarded path.** `Readings` and `Levels` go to frame watchers after the same one-second check that Phase 10's P10-01 settled, with no new method, no new cap and no new privilege.
- **The oracle stays independent.** The centring rule is written twice, in `odc-render` and in the oracle, and `cargo test -p odc-oracle` and the frames catch any disagreement; the Dancer's pictures are read by the oracle from the spec itself.
- **The root surface shrinks.** The sensor-detection one-shot was the only part of OpenDeepCool that ran as root at every boot, and the second root program that OV-01 worried about; both go (Task 5), and with them the root-only record and its undo code.
- **Rule 7 is kept without migration code.** The old package's tested removal undoes what the detection made, and no release exists that would need more.
- **History is kept.** The facts about fans go to a spec file before anything is deleted (Task 2), and the completed phases' records stay as written.

## Analysis

### Removal order and gates

The removal is split along compile boundaries, which keeps every commit green. The parity checklist is a second dependency of the tests: `crates/odc-testkit/tests/parity_checklist.rs` checks that every function a row links exists, and row S9 links four fan tests (P11A-01). The entry-points table that AGENTS.md rule 5 uses is a third (P11A-02).

### Live data and cost

The data's size is small: seven readings once a second, and 14 doubles ten times a second while a Music mode is shown. The cost that matters is the app's own: 23 tiles stepping every 100 ms for as long as the window is alive. A window left open on another workspace is the common case under a tiling window manager, and a `glib` timeout runs there while a frame clock does not (**unverified**; P11A-03). Auto-moving content is also what WCAG 2.2 SC 2.2.2 is about (P11A-06).

### The theme choice

The app has never written a file of its own. The per-user file is the smallest design, and it needs the checks the XDG Base Directory Specification asks for (P11A-05) and a stated exception to rule 7 (P11A-08).

### The development PC

The service tests need the dev copy, which `odc-dev setup` refuses to install beside the package, and `--dev-interface` drives the packaged service, which wouldn't have the new code. So the PC changes state three times (package, dev copy, package), each with the owner's approval (P11A-07).

## Findings

| ID | Severity | Finding | Outcome |
|---|---|---|---|
| P11A-01 | High | Task 4 would fail its own gate: the parity checklist's row S9 links tests the task deletes | Applied in the plan |
| P11A-02 | Medium | Task 5 left the entry-points table's "detection record" row in place | Applied in the plan |
| P11A-03 | Medium | A timer keeps 23 tiles moving in a window the user isn't looking at | Applied in the plan |
| P11A-04 | Medium | Removing the detection might leave a driver unloaded that it used to load | Applied in the plan (a check) |
| P11A-05 | Low | The per-user file's location, symlink and mode rules weren't stated | Applied in the plan |
| P11A-06 | Medium | Auto-moving tiles need a way to pause or stop them (WCAG 2.2.2, Level A) | Open: question 1 |
| P11A-07 | Medium | The development PC changes state three times; each is an approval | Open: question 2 |
| P11A-08 | Medium | The per-user theme file is saved data that uninstall can't remove | Open: question 3 |
| P11A-09 | Low | The selected tile and the preview show the same mode in different phase | Open: for the designer |
| P11A-10 | Low | Task 12 stops for the owner's approval of two redrawn pictures, in the middle of the phase | Open: question 4 |
| P11A-11 | Note | `Levels` carries the sound's levels at full precision | No action |
| P11A-12 | Note | The designer reviews mockups, not the built window | No action |
| P11A-13 | Note | The new number, 11a, needs the README's index to follow | Applied in this folder's README |

### P11A-01: A parity row links the tests Task 4 deletes (High)

- **Evidence:** `docs/spec/parity-checklist.md:125` (row S9, whose OpenDeepCool column links `crates/odc-sensors/tests/fans.rs` and `fan_speeds_reach_only_frame_watchers_and_follow_the_readings` in `crates/odc-service/tests/dbus.rs`); `crates/odc-testkit/tests/parity_checklist.rs:74-110` (every linked function must exist).
- **Failure:** Task 4 deletes both; `cargo test --workspace` then fails in the same commit, and rule 4 forbids committing failing tests.
- **Fix:** delete row S9 and change T1's words in Task 4, after Task 2 has copied S9's text into the kept file.
- **Outcome (2026-11-18, planner):** applied to Tasks 4 and 6 of the plan, before this review was written.

### P11A-02: The entry-points table still lists the detection record (Medium)

- **Evidence:** `project-overview.review.md`, "Entry points after the cross-check" (the row "The detection record"); AGENTS.md rule 5 uses the table for refusal tests.
- **Fix:** Task 5 updates the table in its commit; Tasks 4, 10 and 13 add their own changes (the setters leave, the appearance file and the test flags join, `Readings` and `Levels` join the watchers' row).
- **Outcome (2026-11-18, planner):** applied to Task 5 (Task 4's, 10's and 13's were already in the plan).

### P11A-03: The tiles' tick should stop when nobody can see them (Medium)

- **Evidence:** the design's "no idle CPU use" brief (owner, 2026-10-22); the plan's Task 14. A tiling window manager leaves a settings window open on another workspace for hours.
- **Fix:** drive the tick from the window's frame clock if that stops for a surface the compositor doesn't draw, else from a timeout stopped when the window is hidden, and measure both in Task 1; follow `gtk-enable-animations`.
- **Outcome (2026-11-18, planner):** applied to Tasks 1 (item 6) and 14. Whether the frame clock stops there is **unverified**.

### P11A-04: Drivers the detection used to load (Medium)

- **Evidence:** `docs/spec/sensors.md:64-75` (the detection loads whatever `sensors-detect` finds, which may include a CPU temperature driver as well as the fan chip's; **unverified** for each CPU); the CPU, CPU power and memory rules read drivers the kernel is expected to load by itself.
- **Fix:** Task 1 checks, from the kernel's alias tables and the Arch wiki, that `coretemp`, `k10temp` and the RAPL driver autoload; where one doesn't, the modes that need it are already disabled with their reason, and the README names the driver.
- **Outcome (2026-11-18, planner):** applied to Task 1 (item 7). **Unverified** until then; the failure mode (a disabled mode with a reason) is already specified.

### P11A-05: The appearance file's rules (Low)

- **Evidence:** XDG Base Directory Specification, read 2026-11-18: the default is `$HOME/.config`; a relative path "must be" absolute and is otherwise invalid and skipped; a folder an application creates gets mode `0700`, and an existing one's mode is left alone.
- **Fix:** Task 10 states those rules, opens the file without following a symbolic link, writes atomically with mode `0600`, and tests a relative `XDG_CONFIG_HOME` and a link.
- **Outcome (2026-11-18, planner):** applied to Task 10.

### P11A-06: Auto-moving tiles (Medium)

- **Evidence:** WCAG 2.2, Understanding SC 2.2.2 Pause, Stop, Hide (Level A), read 2026-11-18: for moving, blinking or scrolling information that starts automatically, lasts more than five seconds and is presented in parallel with other content, "there is a mechanism for the user to pause, stop, or hide it unless the movement … is essential"; and for auto-updating information, a mechanism "to pause, stop, or hide it or to control the frequency of the update". The page lists no operating system animation setting among its sufficient techniques, though a platform feature may count if it meets the criterion. The project follows WCAG already (the design's contrast table, SC 2.4.7).
- **Failure:** 23 tiles that move and update from the moment the window opens, beside the settings, have no way to stop. The preview is the cooler's picture and can be called essential; the tiles are a gallery of choices.
- **Recommendation:** the plan already follows `gtk-enable-animations` (the user's own animation setting: the tiles then show still frames and update once a second). Decide whether that is enough or whether the app also needs its own pause control; the smallest own control would be one icon beside Refresh that holds the tiles still.
- **Outcome:** open (question 1).

### P11A-07: The development PC changes state three times (Medium)

- **Evidence:** AGENTS.md, Commands: `odc-dev setup` refuses while the package is installed, and `--dev-interface` adds the dev interface to the packaged service, which has none of this phase's changes; the PRD's rule 7 and the owner's practice (each system step approved with the password).
- **Plan:** remove the old package before Task 4's service runs (this also undoes the detection from its record), stay on the dev copy through Task 14, and install the new package at Task 17.
- **Risk:** a forgotten step leaves the real runs testing the wrong service; a missed approval stalls the task.
- **Recommendation:** ask for each of the three steps once, at the task that needs it, and have each task's report say which service it ran against.
- **Outcome:** open (question 2).

### P11A-08: The per-user theme file and rule 7 (Medium)

- **Evidence:** AGENTS.md rule 7 (uninstalling removes everything the app created, saved data included); the PRD ("No pacman step reaches into users' sessions"; Arch's package guidelines); the file is created in each user's home by the app, only after the user changes the theme.
- **Failure:** `pacman -R` can't remove it, so rule 7 needs a stated exception for it, or a different home for the choice.
- **Recommendation:** keep the per-user file, list it in the README's "Installed on your system" and the PRD's footprint, and say in the uninstall text to delete it by hand; the alternative (the service holds it) costs a contract change and a polkit action, and one user's choice would change another's session.
- **Outcome:** open (question 3).

### P11A-09: The selected tile and the preview (Low)

- **Evidence:** the plan's Task 14: each tile steps its own state machines; the preview shows the service's.
- **Effect:** for the saved mode, the tile and the preview show the same pictures in a different phase (an emoji loop, a scroll position). They are not promised to be in step.
- **Recommendation:** accept it, and have the design say so; drawing the selected tile from the service's frame would need the rotation undone and doesn't help the other 22.
- **Outcome:** open, for the designer.

### P11A-10: An owner gate in the middle of the phase (Low)

- **Evidence:** the plan's Task 12 stops for the owner to approve the two redrawn Dancer pictures (the first Dancer was approved the same way, 2026-10-25).
- **Recommendation:** write the proposed pictures into `music.md` early, with Task 2, so the owner's answer arrives while other tasks run, and Task 12 is not waiting.
- **Outcome:** open (question 4).

### P11A-11 to P11A-13: notes

- **P11A-11:** `Levels` sends the 14 column levels as doubles, a little more detail than `Frame`'s thresholded dots, from the same source and to the same watchers. Quantising to bytes would shave the size, but the Dancer's thresholds (0.1, 0.12, 0.35, 0.6) could then pick a different pose in the app than on the cooler. Keep doubles.
- **P11A-12:** `odc-testkit` has no screenshot support (no `screenshooter` or capture code in `crates/odc-testkit` or `crates/odc-ui-e2e`), so the designer's whole-design review (Task 15) judges the mockups and the design text. The owner's review of the build (Task 17) is the visual check of the real window.
- **P11A-13:** the phase is numbered 11a so that Releases stays Phase 12 and about sixty references in completed phases' records stay valid. This folder's README lists the new review.

## Questions for the owner

1. **Pause control** (P11A-06): is following the system's animation setting enough for the live tiles, or should the app also have its own control that holds them still?
2. **System steps** (P11A-07): may the plan ask, at each of the three points (remove the package before Task 4; `odc-dev setup` after each service change; install the new package at Task 17), for the password then, once each?
3. **The theme file** (P11A-08): is a per-user file, listed in the footprint and removed by hand, acceptable as an exception to rule 7, or should the service hold the theme choice for the whole PC?
4. **The Dancer** (P11A-10): may the proposed pictures for the squat and the jump come to you early, with the fans' facts in Task 2, so the answer is ready when Task 12 starts?

The plan's Risks and Decisions also carry the roles split between the designer and the implementer, the floating window, the `pwm*` sentence in rule 8, and the one commit for five modes; each has a recommendation there, and a "no" changes only what its row says.

## Sources

Read on 2026-11-18:

- XDG Base Directory Specification, specifications.freedesktop.org (`XDG_CONFIG_HOME`, relative paths, directory mode `0700`).
- WCAG 2.2, Understanding SC 2.2.2 Pause, Stop, Hide, w3.org.
- The repository at `1a6a4e9`: the files and lines the plan cites.
