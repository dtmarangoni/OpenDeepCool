# What the package installs and creates

The exact list behind the PRD's System Footprint and the README's "Installed on your system" (Phase 11 plan, Task 1). The package follows Arch's package guidelines: the settings app in `/usr/bin`, every program users don't run themselves in `/usr/lib/opendeepcool/`, nothing in `/usr/local`, and nothing in `/etc` from the package itself.

A test (`crates/odc-service/tests/packaging.rs`) checks this file against the repository: every source below exists, every file under `packaging/` is listed, and every unit's `ExecStart=` names an installed program. Task 3 adds the Start at boot switch's program, and Task 4 the pacman hook and the PKGBUILD's own files.

## Installed by the package

pacman installs these and `pacman -R` removes them. `target/release/` is the PKGBUILD's build.

| Path | Kind | Owner | Mode | From |
|---|---|---|---|---|
| `/usr/bin/opendeepcool` | program | root:root | 0755 | `target/release/opendeepcool` |
| `/usr/lib/opendeepcool/` | folder | root:root | 0755 | - |
| `/usr/lib/opendeepcool/opendeepcoold` | program | root:root | 0755 | `target/release/opendeepcoold` |
| `/usr/lib/opendeepcool/opendeepcool-audio` | program | root:root | 0755 | `target/release/opendeepcool-audio` |
| `/usr/lib/opendeepcool/odc-gpu-rank` | program | root:root | 0755 | `target/release/odc-gpu-rank` |
| `/usr/lib/opendeepcool/odc-sensors-detect` | program | root:root | 0755 | `target/release/odc-sensors-detect` |
| `/usr/lib/systemd/system/opendeepcool.service` | file | root:root | 0644 | `packaging/systemd/opendeepcool.service` |
| `/usr/lib/systemd/system/opendeepcool-gpu-rank.service` | file | root:root | 0644 | `packaging/systemd/opendeepcool-gpu-rank.service` |
| `/usr/lib/systemd/system/opendeepcool-sensors-detect.service` | file | root:root | 0644 | `packaging/systemd/opendeepcool-sensors-detect.service` |
| `/usr/lib/systemd/user/opendeepcool-audio.service` | file | root:root | 0644 | `packaging/systemd-user/opendeepcool-audio.service` |
| `/usr/lib/sysusers.d/opendeepcool.conf` | file | root:root | 0644 | `packaging/sysusers/opendeepcool.conf` |
| `/usr/lib/udev/rules.d/70-opendeepcool.rules` | file | root:root | 0644 | `packaging/udev/70-opendeepcool.rules` |
| `/usr/share/dbus-1/system.d/io.github.dtmarangoni.OpenDeepCool1.conf` | file | root:root | 0644 | `packaging/dbus/io.github.dtmarangoni.OpenDeepCool1.conf` |
| `/usr/share/polkit-1/actions/io.github.dtmarangoni.OpenDeepCool1.policy` | file | root:root | 0644 | `packaging/polkit/io.github.dtmarangoni.OpenDeepCool1.policy` |
| `/usr/share/applications/io.github.dtmarangoni.OpenDeepCool.desktop` | file | root:root | 0644 | `packaging/applications/io.github.dtmarangoni.OpenDeepCool.desktop` |
| `/usr/share/icons/hicolor/scalable/apps/io.github.dtmarangoni.OpenDeepCool.svg` | file | root:root | 0644 | `packaging/icons/io.github.dtmarangoni.OpenDeepCool.svg` |
| `/usr/share/licenses/opendeepcool/` | folder | root:root | 0755 | - |
| `/usr/share/licenses/opendeepcool/LICENSE` | file | root:root | 0644 | `LICENSE` |

The package ships no enable links: nothing is enabled or started at install (Arch Wiki, systemd: Arch "does not enable most services on installation").

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
| `/var/lib/opendeepcool-detect/` (root:root, 0700), with `sensors-detect.json` | systemd, for the detection unit's `StateDirectory=opendeepcool-detect`; the detection writes the record | `post_remove`: `rm -rf`, after the next row |
| `/etc/conf.d/lm_sensors`, `/etc/modprobe.d/lm_sensors.conf`, and `lm_sensors.service` enabled | The detection's `sensors-detect --auto`, only on a PC that has no lm-sensors configuration and no fan reading | `post_remove`, only what the root-only record's yes/no fields say the detection created |
| The LP360's hidraw node: group `opendeepcool`, mode 0660 | The udev rule | systemd's `35-systemd-udev-reload.hook` reloads udev and triggers a change once the rule is gone |
| The CPU's package energy counters (`energy_uj` of each powercap zone named `package-*`): group `opendeepcool`, the group's read added | The udev rule, through `chgrp` and `chmod` | `post_remove` gives the group back to root and takes the group's read away, which a udev reload can't undo |

## What outlasts the uninstall

- A sensor driver loaded by lm-sensors stays loaded until the next boot.
- The journal's lines stay until journald rotates them.
- A core dump of an OpenDeepCool program that crashed stays until systemd's cleanup, after two weeks by default.
- pacman's own log and cache.
