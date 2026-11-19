# What the package installs and creates

The exact list behind the PRD's System Footprint and the README's "Installed on your system" (Phase 11 plan, Task 1). The package follows Arch's package guidelines: the settings app in `/usr/bin`, every program users don't run themselves in `/usr/lib/opendeepcool/`, nothing in `/usr/local`, and nothing in `/etc` from the package itself.

A test (`crates/odc-service/tests/packaging.rs`) checks this file against the repository: every source below exists, every file under `packaging/` is listed, and every unit's `ExecStart=` names an installed program. It also checks that the PKGBUILD's `package()` installs exactly the first table, and that its `build()` builds exactly the crates whose programs that table lists.

## Installed by the package

pacman installs these and `pacman -R` removes them. `target/release/` is the PKGBUILD's build.

| Path | Kind | Owner | Mode | From |
|---|---|---|---|---|
| `/usr/bin/opendeepcool` | program | root:root | 0755 | `target/release/opendeepcool` |
| `/usr/lib/opendeepcool/` | folder | root:root | 0755 | - |
| `/usr/lib/opendeepcool/opendeepcoold` | program | root:root | 0755 | `target/release/opendeepcoold` |
| `/usr/lib/opendeepcool/opendeepcool-audio` | program | root:root | 0755 | `target/release/opendeepcool-audio` |
| `/usr/lib/opendeepcool/odc-gpu-rank` | program | root:root | 0755 | `target/release/odc-gpu-rank` |
| `/usr/lib/opendeepcool/opendeepcool-boot` | program | root:root | 0755 | `target/release/opendeepcool-boot` |
| `/usr/lib/systemd/system/opendeepcool.service` | file | root:root | 0644 | `packaging/systemd/opendeepcool.service` |
| `/usr/lib/systemd/system/opendeepcool-gpu-rank.service` | file | root:root | 0644 | `packaging/systemd/opendeepcool-gpu-rank.service` |
| `/usr/lib/systemd/user/opendeepcool-audio.service` | file | root:root | 0644 | `packaging/systemd-user/opendeepcool-audio.service` |
| `/usr/lib/sysusers.d/opendeepcool.conf` | file | root:root | 0644 | `packaging/sysusers/opendeepcool.conf` |
| `/usr/lib/udev/rules.d/70-opendeepcool.rules` | file | root:root | 0644 | `packaging/udev/70-opendeepcool.rules` |
| `/usr/share/dbus-1/system.d/io.github.dtmarangoni.OpenDeepCool1.conf` | file | root:root | 0644 | `packaging/dbus/io.github.dtmarangoni.OpenDeepCool1.conf` |
| `/usr/share/polkit-1/actions/io.github.dtmarangoni.OpenDeepCool1.policy` | file | root:root | 0644 | `packaging/polkit/io.github.dtmarangoni.OpenDeepCool1.policy` |
| `/usr/share/applications/io.github.dtmarangoni.OpenDeepCool.desktop` | file | root:root | 0644 | `packaging/applications/io.github.dtmarangoni.OpenDeepCool.desktop` |
| `/usr/share/icons/hicolor/scalable/apps/io.github.dtmarangoni.OpenDeepCool.svg` | file | root:root | 0644 | `packaging/icons/io.github.dtmarangoni.OpenDeepCool.svg` |
| `/usr/share/libalpm/hooks/opendeepcool-restart.hook` | file | root:root | 0644 | `packaging/arch/hooks/opendeepcool-restart.hook` |
| `/usr/share/licenses/opendeepcool/` | folder | root:root | 0755 | - |
| `/usr/share/licenses/opendeepcool/LICENSE` | file | root:root | 0644 | `LICENSE` |

The package ships no enable links: nothing is enabled or started at install (Arch Wiki, systemd: Arch "does not enable most services on installation").

## The PKGBUILD's own files

They build the package and aren't installed. From `v1.0.0` they are also the AUR repository (Phase 12).

| File | What it is |
|---|---|
| `packaging/arch/PKGBUILD` | The build: the signed release tag through git, by Arch's package and Rust package guidelines |
| `packaging/arch/opendeepcool.install` | pacman's scriptlets: the install message, and on removal the disable, the stop and the clean-up in the tables below |
| `packaging/arch/.SRCINFO` | The PKGBUILD's metadata, as `makepkg --printsrcinfo` writes it |
| `packaging/arch/LICENSE` | These files' own license, the 0BSD text Arch publishes for package sources (AUR submission guidelines). The program stays MIT |

## Created by systemd's pacman hooks at install

| Item | Created by | Removed by |
|---|---|---|
| The locked system account and group `opendeepcool` | `20-systemd-sysusers.hook`, from `/usr/lib/sysusers.d/opendeepcool.conf` | `post_remove`: `userdel opendeepcool`, then the group if it's left |

## Created when the user turns OpenDeepCool on

With the settings app's Start at boot switch, or `systemctl enable --now opendeepcool.service` and `systemctl --global enable opendeepcool-audio.service`.

| Item | Created by | Removed by |
|---|---|---|
| `/etc/systemd/system/multi-user.target.wants/opendeepcool.service` → `/usr/lib/systemd/system/opendeepcool.service` | `systemctl enable` | `pre_remove`: `systemctl disable`, which also removes the folder if it made it (checked in a scratch root, 2026-11-10) |
| `/etc/systemd/user/default.target.wants/opendeepcool-audio.service` → `/usr/lib/systemd/user/opendeepcool-audio.service` | `systemctl --global enable` | `pre_remove`: `systemctl --global disable`, likewise |

## Created or changed at run time

| Item | Created by | Removed by |
|---|---|---|
| `/var/lib/opendeepcool/` (opendeepcool:opendeepcool, 0700), with `settings.toml` and `gpu-rank.json` | systemd, for the units' `StateDirectory=opendeepcool`; the service and the GPU-rank helper write the files | `post_remove`: `rm -rf` |
| The LP360's hidraw node: group `opendeepcool`, mode 0660 | The udev rule | systemd's `35-systemd-udev-reload.hook` reloads udev and triggers a change once the rule is gone |
| The CPU's package energy counters (`energy_uj` of each powercap zone named `package-*`): group `opendeepcool`, the group's read added | The udev rule, through `chgrp` and `chmod` | `post_remove` gives the group back to root and takes the group's read away, which a udev reload can't undo |

## Made by an administrator under OpenDeepCool's names

pacman never deletes a file it didn't install. These are read in front of, or beside, the package's own files, so one left behind would change the next install; the removal deletes them too (owner, 2026-11-11). Every name is OpenDeepCool's own. Each lives in `/etc`, or in `/run` for the current boot only. In a chroot, whose `/run` may be the host's (`arch-chroot` binds it), only `/etc` is cleaned.

| Item | Made by | Removed by |
|---|---|---|
| `/etc/systemd/system/<unit>`, `<unit>.d/` and `/etc/systemd/system.control/<unit>.d/`, for each of the two system units; the same under `/run/systemd/` | `systemctl edit` (`--full` makes a copy), `systemctl set-property`, `systemctl mask`; `--runtime` for `/run` | `pre_remove`, before the links, since `systemctl disable` skips a masked unit |
| `/etc/systemd/user/opendeepcool-audio.service` and `opendeepcool-audio.service.d/`; the same under `/run/systemd/user/` | `systemctl --global edit`, `systemctl --global mask` | `pre_remove` |
| Links to any of the three units made by hand | `systemctl add-wants`, `systemctl enable --runtime` | `pre_remove`: `systemctl disable` and `systemctl --runtime disable` |
| `/etc/sysusers.d/opendeepcool.conf`, `/etc/udev/rules.d/70-opendeepcool.rules`, `/etc/polkit-1/actions/io.github.dtmarangoni.OpenDeepCool1.policy`; the same three under `/run` | A file that overrides the package's, or masks it as a link to `/dev/null` | `pre_remove` |
| `/etc/dbus-1/system.d/io.github.dtmarangoni.OpenDeepCool1.conf`, `/etc/pacman.d/hooks/opendeepcool-restart.hook` (D-Bus and pacman read no folder in `/run`) | Likewise | `pre_remove` |

The development copy keeps its files in `/etc` under these names, so `odc-dev teardown` removes it before the package is installed (Phase 11 plan, Task 7).

## What outlasts the uninstall

- The journal's lines stay until journald rotates them.
- A core dump of an OpenDeepCool program that crashed stays until systemd's cleanup, after two weeks by default.
- pacman's own log and cache.
- shadow-utils' backups of the account files, `/etc/passwd-`, `/etc/group-`, `/etc/shadow-` and `/etc/gshadow-`: `userdel` saves each file as it was just before the account went, so they keep the `opendeepcool` lines until the next change to any account on the PC (the package test, 2026-11-11). The account files themselves no longer have them.
- Users' home folders, which Arch's package guidelines keep packages out of. OpenDeepCool's programs write nothing there. A user's own `systemctl --user` changes to the sound helper stay theirs, and WirePlumber's remembered volume is kept for `pw-record`, which every `pw-record` capture shares.
- Software an administrator installs in `/usr/local`, such as the development copy, which `odc-dev teardown` removes.
