# Phase 10 review: the settings app

**Review ID:** `phase-10-settings-app`
**Reviewed:** 2026-10-26, at commit `3ec34b7`
**Phase status in the PRD:** pending (planned)
**Inputs:** [Phase 10 plan](../plans/phase-10-settings-app.plan.md); [`docs/design/settings-app.md`](../../design/settings-app.md) and its mockups; `docs/spec/settings.md` (What the settings app must offer); the Phase 5, 7, 9 and 11 plans; PRD (Theme, Settings app ↔ service, Phase 10, Testing Strategy)
**Overview:** [project-overview.review.md](project-overview.review.md)

## Verdict

The app's architecture is clean:

- a pure view model maps the service's published state to a plain description of the screen;
- a pure theme module turns Omarchy's files into one CSS string;
- a thin GTK layer renders it, so every state in the design is a unit test without a display;
- the service stays the single authority: the app never writes settings, never shows a change the service refused, and asks the service, not polkit directly, whether it may change anything.

The preview mirrors the frames the service actually writes, so it's exact for the spectrum and the Dancer without a second engine. The UI tests drive the real binary through AT-SPI in headless weston, a spike-proven, unattended approach.

The main issue is privacy (P10-01, Medium). The new `Frame` signal is broadcast on the system bus, where the default policy lets every process receive signals, and `WatchFrames` is open to every caller. In Music modes, every local user, remote ones included, can follow a spectrum derived from the active user's sound.

Two coherence gaps with the CI and packaging plans need settling: the Ubuntu CI job can't build the GTK crate (P10-03), and the weston-based tests sit inside `cargo test --workspace`, which the PKGBUILD's `check()` runs on users' PCs (P10-04).

## What the phase builds

- `odc-app`: the `opendeepcool` binary with the view model, the theme, the GTK layer, the `odc-ipc` client on a zbus thread, and a `glib` channel into the main loop.
- Six additions to the service contract: `WatchFrames`/`UnwatchFrames` and the `Frame(ay)` signal; RPM in `FanInputs`; IDs and a rule flag in `Gpus`; `GpuInUse`; the two "input present" flags; `CanChangeSettings()`.
- `odc-testkit::weston` and a fake service on a private `dbus-daemon`; `odc-ui-e2e`, an AT-SPI driver on `atspi` 0.30 (zbus 5); the `--only app` area.
- The desktop entry, the icon, and CI in an `archlinux` container.

## Strengths

- **A pure view model and theme.** Every row of the design's States table, the tile set and the theme fallbacks are unit-tested without a display.
- **One authority.** Instant apply shows a new state only after the service's `PropertiesChanged`, and `CanChangeSettings` reuses the service's own `Authorizer`.
- **No polling and no idle cost.** Frames flow only while a window is mapped, watchers are dropped when they leave the bus, and the preview redraws only on a changed frame.
- **Accessibility as a test surface.** Fixed accessible names, contrast ratios computed for the default palettes, focus never applying a tile, and keyboard shortcuts.
- **The design's open questions are checked first** (Task 1), each with a fallback that changes a widget, never the behaviour.
- **Test-only behaviour through flags** (`--service-address`, `--omarchy-state`), with any other argument an error. That's consistent with the project's rule.

## Analysis

### Frames on the system bus

The plan's rationale: "the frame is what anyone near the PC sees". Three things weaken it:

- **Broadcast signals reach everyone.** The system bus's default policy allows `receive_type="signal"` for every user (`/usr/share/dbus-1/system.conf`, read 2026-10-26). A process that adds a match rule receives every `Frame` signal while anyone watches, without calling `WatchFrames`.
- **"Near the PC" isn't "on the bus".** Remote users (SSH) and other local accounts are on the same system bus.
- **In Music modes, a frame is derived from the active user's sound.** That's 14 bands at 10 Hz, not enough to reconstruct speech, but enough to tell when someone is on a call or playing music, and when.

Recommendations:

- send `Frame` only to the watchers: D-Bus messages, signals included, can carry a destination, and dbus-daemon then delivers them to that connection only. zbus 6's call for this is unverified;
- check `WatchFrames` against a read action (`allow_active` and `allow_inactive` yes, `allow_any` no), so remote sessions get settings but not live frames. The app's view-only mode for remote sessions is unaffected: the preview would then show the static settings picture rather than live frames;
- cap watchers per user and in total, since unicast makes the cost grow with watchers (P10-02).

### CI and packaging coherence

- **The Ubuntu job.** Phase 5's workflow runs `cargo build`, `clippy` and `test --workspace` on `ubuntu-latest`. Once `odc-app` (gtk4-rs) joins the workspace, that job needs GTK 4 development files, and the design needs GTK 4.16 or newer for CSS custom properties. The plan adds an Arch container for "the UI jobs" but doesn't say what happens to the Ubuntu job. Moving all CI into the Arch container, the target platform, settles it (P4-05).
- **`cargo test --workspace` starts weston.** The layer-3 tests of Tasks 4–7 run "in `Weston::headless()`" from `cargo test`. Three places then need weston and AT-SPI:
  - the pre-commit guard, on the owner's PC: fine;
  - CI, in the container: fine;
  - Phase 11's `check()` in the PKGBUILD, on every AUR user's PC: there it fails or must be skipped.

  Keep the pure view-model and theme tests in `cargo test`. Put the display-bound tests behind an explicit switch: a cargo feature (`ui-tests`), or an `odc-e2e` area that CI calls. Then `check()` stays hardware- and display-free.

### Theme input

The theme module reads `colors.toml` and `shell.toml` from the user's own Omarchy state and writes their values into CSS. They're the user's own files, so a mistake only harms that user's own app. Still, validate every colour value strictly before writing it into CSS: `#rrggbb`, `#rrggbbaa`, or a parsed `rgba(…)` whose alpha comes from a number. A malformed or unexpected value then falls back to the default token, logged, instead of producing CSS that GTK half-parses. The gradient case is already handled by taking the first colour.

### Live reload

Decision D8 in the design says the `GFileMonitor` watches "the parent of `current/`". Its Theme section and this plan's Task 7 say `current/`. Omarchy moves a new theme over `current/theme`, so the folder whose entries change is `current/`. Watching its parent would only see `current` itself being replaced. Settle on `current/` in the design, and check in Task 1 whether `current` is itself ever replaced or a symbolic link (unverified). If it is, watch both.

### The contract changes

`FanInputs` and `Gpus` change signature under `…OpenDeepCool1`. That's acceptable before any release, as the plan argues, but it's the last chance: freeze the contract at v1.0.0 and give it one committed description with a conformance test (P4-04).

### Smaller points

- **The service banner** shows `sudo systemctl start opendeepcool.service`. On most desktops `systemctl start` asks for authorization through polkit itself; either form works. Note only.
- **`pci.ids`** needs `hwdata` at run time, which Phase 11 lists as a dependency. Without it, the label falls back to kind, driver and address, as designed.
- **Two zbus versions** (`odc-ui-e2e` uses `atspi` with zbus 5) affect only a test-only binary. Keep it out of the shipped build (P11-21).

## Findings

| ID | Severity | Category | Title |
|---|---|---|---|
| P10-01 | Medium | Privacy | `Frame` is broadcast to every bus client, remote users included; in Music modes it reflects the active user's sound |
| P10-02 | Low | Availability | No cap on frame watchers (needed once frames are sent per watcher) |
| P10-03 | Medium | Coherence, CI | The Ubuntu CI job can't build the GTK crate; the plan moves only the UI jobs |
| P10-04 | Medium | Coherence, packaging | Weston-based tests inside `cargo test --workspace` break the PKGBUILD's `check()` on users' PCs |
| P10-05 | Low | Robustness | Theme values go into CSS without strict validation |
| P10-06 | Low | Coherence | The design says the monitor watches "the parent of `current/`"; the plan says `current/` |
| P10-07 | Note | UX | The service banner's `sudo` command; `systemctl start` with polkit also works |
| P10-08 | Note | Build | Two zbus versions in the lock file, from a test-only crate |
| P10-09 | Note | Performance | `FanInputs` changes once a second while watched: small, and only while the app is open |
| P10-10 | Low | Packaging | `hwdata` is needed at run time for GPU names (listed in Phase 11) |
| P10-11 | Low | Maintainability | Contract signatures change under the same interface version; freeze at v1.0.0 |
| P10-12 | Note | Security | Test flags in the shipped app are harmless: they only redirect the user's own app |

### P10-01: Frames to everyone

- **Evidence:** plan Task 2 (`WatchFrames` "open to every caller as reads are"; `Frame(ay)` emitted "only while watchers exist"); Risks ("visible to any local process … acceptable"); the system bus default policy (`<allow receive_type="signal"/>`).
- **Recommendation:** owner decision (overview Q4). Signals addressed to watchers only, a read action for `WatchFrames` that excludes remote sessions, and a cap on watchers. Test it: a second connection with a match rule but no watch receives nothing.
- **Carried from the overview cross-check (2026-10-26):** settle this here.
  - The cooler gets its frames over USB, never D-Bus; the `Frame` signal exists only for the app's preview.
  - The owner's rule (OV-05) is that only the active user needs access to the cooler, and a PC has one user at a time. That points to frames sent only to watchers in the active local session.
  - Still to decide: whether remote or inactive sessions keep the read-only view at all.
- **Outcome (2026-10-26):** decided by the owner: no remote or inactive user can use the service, only the current logged-in user. Applied:
  - every call, reads included, needs polkit (`use` or `change-settings`, `allow_active` only); the service has no readable properties;
  - `Frame` and `StateChanged` go to each authorized watcher alone (zbus 6.1 `set_destination`), re-checked every second; dbus-broker has no eavesdropping;
  - the app's view-only state is replaced by a not-available state, and `CanChangeSettings()` is dropped.
  Where: PRD (Settings app ↔ service, Decisions Log), `settings.md`, checklist T7, the Phase 7, 9 and 10 plans, and the design (by the `ui-ux-designer` agent).
- **Also from the cross-check:** the mockup generator (`docs/design/mockups-src/app.js`, `numberFor`) still uses DeepCreative's rounding (up, and °F rounded up then floored). It should use the nearest rule (`display.md` Values) the next time the mockups are regenerated. **Done 2026-10-26:** `numberFor` rounds to the nearest, °F once from the exact °C, and the mockups were regenerated with the new not-available state.

### P10-02: Watcher cap

- **Recommendation:** for example 8 watchers per Unix user and 32 in total, with a clear error beyond that.
- **Outcome (2026-10-26):** accepted with P10-01: 8 per Unix user and 32 in all, frame and state watchers together, `TooManyWatchers` beyond (Phase 7 Task 7).

### P10-03: The Ubuntu job

- **Recommendation:** all CI in the `archlinux` container (P4-05). If an Ubuntu job stays, exclude `odc-app` and the UI crates from it explicitly.

### P10-04: Display-bound tests and `check()`

- **Recommendation:** a cargo feature or an `odc-e2e` area for the weston tests; `cargo test --workspace` without it stays display-free; CI enables it. Phase 11's `check()` runs without it and passes on a PC with no weston.

### P10-05: Theme values

- **Recommendation:** parse colours into a typed value; write only re-serialised values into CSS; one test per malformed value (missing `#`, a CSS keyword, `;}` injected text).

### P10-06: Monitor location

- **Recommendation:** fix the design's D8 wording to `current/`, and check in Task 1 how Omarchy replaces `current`.

### P10-11: Contract freeze

- **Recommendation:** see P4-04.

## Questions for the owner

1. Frames only to watchers, and not to remote sessions (P10-01; overview Q4)? **Answered 2026-10-26:** yes, and nothing at all for remote or inactive sessions.
2. All CI in an Arch container (P10-03; P4-05)?

## Sources

- `/usr/share/dbus-1/system.conf` (dbus package), read 2026-10-26: the default policy has `<allow send_type="signal"/>` and `<allow receive_type="signal"/>`.
- The design document and the plan's own evidence (docs.gtk.org, the AT-SPI spike, crate versions).
