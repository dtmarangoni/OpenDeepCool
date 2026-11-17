# Arch portability review: does OpenDeepCool work on Arch Linux and Arch-based distributions?

**Review ID:** `arch-portability`
**Reviewed:** 2026-11-17, at commit `ee93c98` (Phase 11 implemented, awaiting the owner's sign-off)
**Trigger:** the owner, 2026-11-17: "It should be build for Arch or any Arch based distro in general. I'm using Omarchy, but another person using Arch or Arch based distro should be able to use OpenDeepCool as well. Verify the entire solution from documents, files, code, etc... everything, and make sure the solution have as target Arch and will work fine in Arch or any Arch based distro."
**Scope:** every document in the repository, the code of every crate, the packaging, the units and rules, CI, and the tests' environment.
**Overview:** [project-overview.review.md](project-overview.review.md)

## How it was checked

- **Searches:** every mention of Omarchy (331 lines in 42 files outside `research/` and `docs/local/`), of Hyprland, Sway, GNOME, Plasma and other desktops, and of PipeWire and PulseAudio, over the code, the packaging and the documents.
- **Reading:** the PKGBUILD, the install script, the pacman hook, the four system units and the user unit, the udev rule, the polkit and D-Bus files, the app's theme code and the boot switch's error handling, the spec's sensor rules, the design's theme section, the README's install steps and the CI workflow.
- **Arch's own sources, all read 2026-11-17:** ArchWiki (Security, polkit, PipeWire, Arch-based distributions, Core dump, Bug reporting guidelines, Debugging/Getting traces, Arch package guidelines, PKGBUILD, pkgstats); the package metadata of `gtk4` and `pipewire-audio` on archlinux.org; the stock `linux` kernel's `config.x86_64` in Arch's package repository (header "7.3.4-arch1"); `pacman -Qo` for the owner of `pw-record`.

## Verdict

The solution is already general in every layer that touches the system. The product code has no desktop-specific call: nothing speaks to Hyprland, Sway, GNOME or Plasma. The packaging has no mention of Omarchy. CI builds and tests in the official `archlinux:base-devel` image, in all three jobs. Omarchy appears in the product only as an optional source of colours: the theme module reads `~/.local/state/omarchy`, and the app falls back to its own theme when the folder isn't there, which the UI tests cover.

What isn't general is how the documents describe it, and five places where behaviour on other Arch setups is unsupported or has never been tried: the sound helper's sandbox on `linux-hardened`, Music on a PulseAudio system, the systemd requirement, the Rust version the build asks for, and the Start at boot switch where no polkit agent runs. None breaks the display service, which is the core promise. Findings AP-02 and AP-03 touch Music, 11 of the 23 modes.

## Findings

| ID | Severity | Finding |
|---|---|---|
| AP-01 | Medium | The documents frame the product as made for Omarchy |
| AP-02 | Medium | The sound helper's sandbox needs unprivileged user namespaces, which `linux-hardened` turns off; never tried with them off |
| AP-03 | Medium | Music works only with PipeWire, and the package forces PipeWire onto PulseAudio systems |
| AP-04 | Medium | The design needs systemd; Arch-based distributions without it can't run it, and no document says so |
| AP-05 | Low | `rust-version = "1.99"` was the pinned development toolchain, the newest one, not the oldest that builds (now 1.92) |
| AP-06 | Low | Where no polkit agent runs, the Start at boot switch fails with a message that doesn't say what to do |
| AP-07 | Low | The default theme is always dark, whatever the desktop prefers |
| AP-08 | Note | Hardware is tested for real on one PC, and CI runs only on Arch |
| AP-09 | Note | Arch has no automatic crash reporter, and a core dump holds the user's memory |

### AP-01 (Medium): the documents frame the product as made for Omarchy

- **Evidence:**
  - README, first line: "…on **Omarchy** (Arch Linux)".
  - AGENTS.md, "What this project is": "replaces DeepCreative… on Omarchy (Arch Linux)… on any PC running Arch Linux, with Omarchy first".
  - The PRD's title is "…display app for Omarchy"; its Users & Context says "a single Omarchy desktop user", its Success state says "an app that matches the Omarchy theme", and its Non-goals say "Other… distributions… OpenDeepCool supports only the LP360, on Arch Linux: Omarchy comes first".
  - The PRD's Technical Approach says the app links "the system GTK4, which Omarchy already ships".
- **Why it matters:** the text decides what a contributor treats as the target. Arch-based distributions are excluded outright by the PRD's non-goal.
- **Recommendation:** the target is Arch Linux and the Arch-based distributions that use Arch's packages and systemd; Omarchy is one of them, whose theme the app follows when it finds it. Reword those places, add one rule to AGENTS.md, and record the decision in the PRD's Decisions Log. The Omarchy-theme feature itself stays.
- **Outcome (owner, 2026-11-17):** accepted and applied: the wording, AGENTS.md rule 14 and the PRD's Decisions Log row.

### AP-02 (Medium): the sound helper's sandbox needs unprivileged user namespaces

- **Evidence:**
  - `packaging/systemd-user/opendeepcool-audio.service` sets `PrivateUsers=yes`, `ProtectSystem=strict`, `PrivateTmp=yes`, `PrivateDevices=yes`, `PrivateNetwork=yes` and more. Its own comment says that in a user unit these settings need unprivileged user namespaces (systemd.exec(5), read 2026-10-27).
  - ArchWiki, Security: unprivileged user namespaces are on by default in every officially supported kernel **except `linux-hardened`**, which turns them off (`kernel.unprivileged_userns_clone`).
- **What follows:** on `linux-hardened`, or after any `sysctl kernel.unprivileged_userns_clone=0`, the user unit can't set up its sandbox, so the helper doesn't run and Music shows "No sound: spectrum is silent". The display service, a system unit started by root, isn't affected. 
- **Reproduced (2026-11-17, one `pkexec` step the owner approved, the setting off for 9 s and put back, a safety timer set):** with `kernel.unprivileged_userns_clone=0` the user unit fails at once and keeps restarting: `status=217/USER`, "Failed to set up user namespacing for unprivileged user: Operation not permitted" (the journal), `Result=exit-code`. A transient system unit with `PrivateUsers=yes`, `ProtectSystem=strict`, `PrivateTmp=yes`, `PrivateDevices=yes` and `PrivateNetwork=yes` started and ended well in the same window, so the display service, which the root manager starts, isn't affected. After the setting went back to 1 the helper started again.
- **Recommendation:** don't weaken the unit for everyone: the owner chose the strongest sandbox (2026-11-10). Say in the README what the symptom is and the fix, with its cost: `sysctl kernel.unprivileged_userns_clone=1` lets every user create user namespaces, which `linux-hardened` turns off on purpose (ArchWiki, Security).
- **Outcome (owner, 2026-11-17):** reproduce it (done); the README documents the symptom and the fix.

### AP-03 (Medium): Music works only with PipeWire, and the package forces PipeWire on PulseAudio systems

- **Evidence:**
  - The helper runs `pw-record` (`crates/odc-audio/src/lib.rs`), which `pacman -Qo` shows is owned by `pipewire-audio`; the PKGBUILD has `'pipewire-audio'` in `depends`.
  - ArchWiki, PipeWire: PipeWire replaces PulseAudio through `pipewire-pulse`, so the two are alternatives, and Arch keeps both in its repositories. `pipewire-audio` itself declares no conflicts (archlinux.org metadata), so on a PulseAudio system pacman installs it beside PulseAudio.
- **What follows:** on a PulseAudio system the install adds PipeWire's audio stack that the user didn't choose, and Music still has no sound. The app says so ("No sound"), and the helper logs "pw-record can't start". Unverified: whether the extra PipeWire daemon, socket-activated by `pw-record`, disturbs a running PulseAudio.
- **Recommendation (the owner decides):** (a) keep the hard dependency, say in the README that Music needs a PipeWire sound server, and have the app's note name the cause; or (b) move `pipewire-audio` to `optdepends`, which pacman prints at install, and keep the helper's existing "pw-record not found" state. (a) is the simplest and matches how most current installs run; (b) is the more careful for PulseAudio users, and Arch's guidelines keep optional features out of `depends`. Music is half the modes, so I'd choose (a) with the note.
- **Outcome (owner, 2026-11-17):** (a): keep `pipewire-audio` as a dependency. The README says Music needs PipeWire.

### AP-04 (Medium): the design needs systemd

- **Evidence:** the display service and its helpers are systemd units, with `sysusers.d`, `StateDirectory=`, the polkit actions that need logind's active session, `systemctl --global enable`, and a pacman hook that calls Arch's `systemd-hook`. ArchWiki, Arch-based distributions, lists Artix among the derivatives; Artix is known for choosing another init system (OpenRC, runit, s6 or dinit), but that is from general knowledge: the page's text I read says nothing of its init, so it is unverified here.
- **What follows:** an Arch-based distribution without systemd can't run it. Manjaro, EndeavourOS, CachyOS and Garuda are listed by the wiki as derivatives, and as far as I know they keep systemd and Arch's packages, so they can (unverified).
- **Recommendation:** state the requirement: Arch Linux and Arch-based distributions that use systemd. Say in the README and the PRD that those without it are out of scope. Supporting them would be a different init layer, which I don't recommend for v1.
- **Outcome (owner, 2026-11-17):** out of scope for now. The README and the PRD say that systemd is necessary.

### AP-05 (Low): the Rust version asked for is the newest one

- **Evidence:** `Cargo.toml` has `rust-version = "1.99"`, which is also `mise.toml`'s pinned toolchain. The PKGBUILD builds with the system `cargo`, as Arch's Rust guidelines say.
- **What follows:** on a distribution whose `rust` package is older than the current one (derivatives that hold packages back), the build stops at once with Cargo's version error. That is a clear failure, but a lower floor would allow the build.
- **Recommendation:** measure the oldest toolchain that builds the packaged crates (the code uses `let` chains, stable since 1.88; the rest is unmeasured) and set `rust-version` to it, keeping the pin in `mise.toml` for development. Unverified: how far any derivative lags Arch's Rust.
- **Measured (2026-11-17):** the highest `rust-version` that any package in the shipped crates' dependency graph declares is 1.92 (`pango-sys`; 123 packages, 13 declare none), so the lock file can't build with an older compiler whatever our own floor says. With Rust 1.92.0 (installed apart from the owner's toolchains) the six shipped crates check, and the 636 tests of the package's `check()` pass.
- **Outcome (owner, 2026-11-17):** measure and lower it. Done: `rust-version = "1.92"`; `mise.toml` keeps the newer toolchain for development, and `crates/odc-e2e/tests/arch_tools.rs` still keeps the floor at or below Arch's `rust`. A CI job that builds with 1.92 would keep the floor true; it can't be tried locally, so it is left for the owner to ask for.

### AP-06 (Low): no polkit agent, no explanation

- **Evidence:** ArchWiki, polkit: a graphical session needs an authentication agent; Cinnamon, Deepin, Hyprland, GNOME, KDE, LXDE, LXQt, MATE and Xfce have one, and other setups must install one. The app maps `pkexec`'s code 127 to "Not changed: the password wasn't accepted, or no password dialog could open in this session." (`crates/odc-app/src/boot.rs`). The README and the install script do give the terminal commands.
- **Recommendation:** add to that note, and to the README's "Turn it on and off", the terminal commands and a pointer to the wiki's list of agents. Settings changes need no agent: their actions are open to the active local session.
- **Outcome:** the README gets the sentence now (a recommendation, not a question to the owner). The app's note is UI text, so it goes on the list for the owner's UI review.

### AP-07 (Low): the default theme is always dark

- **Evidence:** the design (settings-app.md, Theme) says the default theme is dark outside Omarchy in v1; the owner decided that other desktops' themes wait until after v1 (PRD, Non-goals, 2026-10-23). The app never sets window opacity or blur, so it is opaque everywhere.
- **What follows:** it works on every desktop but ignores a light preference. Arch's `gtk4` is 4.24.1, past the 4.20 that the design's colour-scheme property needs, so following the desktop's preference is possible.
- **Recommendation:** ask the owner whether the default theme should follow the desktop's light or dark preference in v1. It would take the existing light palette, which the design already has.
- **Outcome (owner, 2026-11-17):** the default theme follows the system's dark or light preference, and the app gets an option to change between dark and light. Both are done in the owner's UI review, after Phase 11 and before Phase 12.

### AP-08 (Note): coverage

- Real hardware has run on one PC. Intel and AMD CPU temperature, AMD CPU power through the same `intel-rapl` zones, Intel and AMD and NVIDIA GPUs, and board fan chips other than the owner's are tested through fakes built from the kernel's documentation; a driver without a rule leaves its metric unavailable, with the reason, and the README says this. Each is unverified on real hardware.
- CI runs on Arch's own image only. No Arch-based distribution is tested; I don't recommend more, since they share Arch's packages.

### AP-09 (Note): crash information

See the owner's question of 2026-11-17 and the options below.

- **What Arch says (all read 2026-11-17):** core dumps are stored by `systemd-coredump` for at least two weeks and should be shared "only with trusted parties as they may contain sensitive data (such as passwords or cryptographic keys)" (Core dump). A bug report carries the package version, the relevant journal lines, how to reproduce it and, if it crashed, a stack trace; the reports are human-written (Bug reporting guidelines). For a package Arch doesn't build, the way to symbols is a rebuild from source (Debugging/Getting traces). **I found no automatic crash reporter in any of these pages.**
- **The one precedent for data sent from Arch machines is `pkgstats`:** a separate package; once installed, a weekly timer sends the package list, the architecture and the mirror, anonymously, to archlinux.de, and the user takes part by installing it (ArchWiki, pkgstats; pkgstats.archlinux.de).
- **Other systems:** Ubuntu's error tracker lets users send an error report "with the click of a button", and says it may hold "a unique identifier for your computer" and "personal information such as the state of programs that were running" (Canonical's data privacy page, search excerpts). Fedora's ABRT detects problems and lets users create reports (Fedora wiki). Both rely on servers the distribution runs.
- **What an automatic report would need here:** an endpoint to receive it (a server the owner runs, or a hosted service), a privacy policy, a consent switch, a scrubber for personal data, and a way to send, which the display service can't do: it has no network by design. An install-time question isn't how Arch packages work: pacman's scriptlets print messages, and I found nothing that lets them ask; an AUR helper or `--noconfirm` leaves nobody to answer (my inference, unverified). The decision of 2026-11-10, that nothing is on at install and the settings app has the switch, fits: the consent would be a switch in the app, off until the user turns it on.
- **Options:**
  1. **Manual, what the README now asks** (version, journal lines, `coredumpctl list` line). Costs nothing; most Rust crashes are panics whose message names the source line.
  2. **`opendeepcool --report`**: prints a ready bug report (version, unit states, which readings were found, the last journal lines, no host name or machine ID), which the user reads and pastes into a GitHub issue. Small code, no server, no consent problem. A Phase 12 candidate.
  3. **Opt-in automatic sending**, off by default, with a switch in the app and a separate sender, in the way `pkgstats` is separate. It needs the endpoint and policy above. Worth considering only once there are users to justify it.
- **Recommendation:** 1 now, 2 in Phase 12, 3 only if wanted later.
- **Outcome (owner, 2026-11-17):** option 2 now: `opendeepcool --report` is built in Phase 11 (plan Task 11; [report.md](../../spec/report.md)). Option 3 isn't built.

## What was checked and is fine

| Area | Result |
|---|---|
| Desktop independence | No product code calls any desktop or window manager. GTK4 is the only toolkit, and works on Wayland and X11. The app never sets opacity or blur. |
| Omarchy in the product | Only the theme module reads Omarchy's state folder; without it the app uses its own theme (UI tests start it with no Omarchy folder). Nothing in `packaging/` mentions Omarchy. |
| Icons and desktop files | Arch's `gtk4` 4.24.1 depends on `adwaita-icon-theme`, `adwaita-fonts`, `gtk-update-icon-cache`, `desktop-file-utils` and `xdg-desktop-portal-gtk`, so the app's symbolic icons and its launcher entry work on a bare Arch. |
| Kernel drivers | Arch's stock `linux` config (`config.x86_64`, "7.3.4-arch1") builds `coretemp`, `k10temp`, `nct6683`, `nct6775`, `it87`, `amdgpu`, `i915`, `xe` and `intel_rapl` as modules, and `hidraw`, `usb_hid` and powercap in the kernel. Every driver the sensor rules name is there. |
| Sensors | The rules cover Intel and AMD CPUs, AMD, Intel and NVIDIA GPUs, and any board fan input chosen by the user; detection uses Arch's `lm_sensors`. |
| Packaging | One PKGBUILD for the AUR and for a GitHub clone, `arch=('x86_64')`, `RUSTUP_TOOLCHAIN=stable` and `cargo`, files in `/usr/bin`, `/usr/lib/<name>`, `/usr/share` and `/usr/lib/{systemd,sysusers.d,udev}`, nothing in `/usr/local`. It enables nothing at install, as Arch's packages don't. |
| CI | All three jobs run in `archlinux:base-devel`. |
| Dependencies | `pw-record` is in `pipewire-audio`, so that dependency is the right one. `hwdata`, `lm_sensors`, `polkit`, `systemd` and `hicolor-icon-theme` are Arch packages. |

## Not verified

- The exact failure of the sound helper without unprivileged user namespaces (AP-02).
- Whether a PipeWire daemon started by `pw-record` disturbs a PulseAudio system (AP-03).
- How far any Arch-based distribution lags Arch's Rust (AP-05); the oldest Rust that builds (AP-05).
- AMD CPU power through `intel-rapl` and the other sensor rules on real hardware (AP-08).
- Artix's init choice, from general knowledge (AP-04).

## Questions for the owner, answered 2026-11-17

1. **Support boundary (AP-04):** systemd only. Other init systems are out of scope for now; the notes say systemd is necessary.
2. **Music and PipeWire (AP-03):** keep `pipewire-audio` as a dependency.
3. **`linux-hardened` (AP-02):** yes, reproduce it. Done, see AP-02.
4. **Rust version (AP-05):** yes, measure the oldest that builds and lower `rust-version`.
5. **Default theme (AP-07):** follow the system's dark or light preference, with an option in the app to change it; both in the UI review.
6. **Crash information (AP-09):** option 2 now.

## Outcomes

Each finding's Outcome line is above. Carried into: the README ("What it needs", "Reporting a crash"), the PRD (Decisions Log, the UI review's scope), `docs/spec/settings.md` (Theme), the designer's brief, the Phase 11 plan (Task 11, `--report`) and the spec (`docs/spec/report.md`).
