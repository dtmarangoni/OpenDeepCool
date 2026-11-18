# The cooler's fans and pump (removed from OpenDeepCool, kept for a later version)

> **Removed on 2026-11-18 (owner).** OpenDeepCool v1 covers the cooler's display only. It reads no fan or pump speed, lists no fan input, has no service to find the board's sensor driver, and shows nothing of them in the app. Everything the project learned about measuring and showing them is here, so a later version can bring it back without starting again. **Nothing in this file is built.**
>
> The code and the tests as they were are in the repository's history at commit `1a6a4e9` (Phase 11 complete): `crates/odc-sensors/src/fans.rs`, `crates/odc-sensors-detect/`, `packaging/systemd/opendeepcool-sensors-detect*.service`, the fan parts of `crates/odc-service/`, `crates/odc-ipc/` and `crates/odc-app/`. The decision is in the PRD's Decisions Log (2026-11-18) and the Phase 11a plan.
>
> Which board fan input is the CPU fan or the pump on the owner's PC is a machine fact: see `docs/local/`. It is never part of this file, and the program never relied on it.

Evidence labels are those of the [spec README](README.md).

## What DeepCreative does

- DeepCreative never sends fan or pump speeds to the LP360. It shows them on its own device card, from the board fan inputs the user picks as **CPU Fan I/O** and **Pump Fan I/O**, out of the board's fan list. Evidence: code: LP page settings and the helper's fan list; Phase 1 report.
- **How DeepCreative builds the fan list** (static analysis, 2026-10-23):
  - The helper puts every fan-speed reading that HWiNFO reports for the motherboard's sensor chip into a "Fan list", which maps a name to an RPM. That's the HWiNFO sensor group that also holds the board's +3.3 V, +5 V and +12 V readings. Fan readings from other groups, such as a graphics card's own fan, are left out. Evidence: code: the helper's metric-mapping function (SensorWorker.exe).
  - Each name is HWiNFO's own label for that reading. DeepCreative adds no names and doesn't match any name. Where HWiNFO knows the board, its labels name the board's headers: the HWiNFO sample DeepCool ships labels its board fans "System 2" to "System 4". How HWiNFO picks its labels is closed and **unverified**.
  - Both dropdowns show "N/A" first, which is the default (no input), then the names from the list. Nothing is chosen automatically. Evidence: code: the LP page's I/O settings component and its store defaults; system: the first-start store held no fan inputs.
  - The saved choice is the name. With each snapshot, the device card shows the RPM stored under that name. Evidence: code: the LP page's sensor handler.
  - The main process passes the list's names to the page (IPC `app/get-fan-interface`). The main process is V8 bytecode, so this is **inferred** from the page looking up each chosen name in the same list.
  - The list is keyed by name, so two readings with the same label would share one entry: **inferred**, not observed.
  - Under Wine the list is empty: HWiNFO needs its Windows kernel driver to read the sensor chip (Phase 1 report).
- DeepCreative's first start held no fan inputs; the default is none chosen (settings table: "CPU Fan I/O, Pump Fan I/O", default none, "unverified: under Wine the list is empty"; the RPM shows on DeepCreative's own device card, never on the cooler).

## How OpenDeepCool measured and showed them

Written for OpenDeepCool on 2026-10-23 and later; it worked on any PC.

- **Source:** hwmon. There was no rule that named an input: the user picked any `fanN_input` from any hwmon device except a graphics card's. The kernel's hwmon documentation says "There is no standard that ensures, for example, that … the second fan is on the CPU", so no rule could choose. Unit: RPM, whole numbers.
- **The settings app listed** every hwmon fan input from every sensor chip, with the chip's name, the driver's label (`fanN_label`) when it has one, and the live RPM.
- **A graphics card's own fans were left out**, as DeepCreative does (owner, 2026-10-23): a pump or radiator fan never connects to a GPU. The rule: skip an hwmon device whose `device` is a PCI display controller, PCI base class `0x03` (kernel `pci_ids.h`: `PCI_BASE_CLASS_DISPLAY`). System, 2026-10-23: a graphics card's fan device has PCI class `0x030000`, while the board chip's is a platform device.
- **The user picked** one input for the cooler's fans (DeepCreative's CPU Fan I/O: the radiator fans sit on the board's CPU fan header) and one for its pump. None was chosen by default, as in DeepCreative.
- **The choice was saved** as the chip's name, its device path relative to the sysfs root, and the input number, never as an hwmon index, which changes between boots. If that input was missing, the app showed it as not found, never another input's RPM. One that was there but whose speed couldn't be read showed "No reading".
- **The speed was capped** at 9,999 RPM like every value in OpenDeepCool: a faster reading showed `9,999 RPM` (owner, 2026-10-25). The speeds never reached the cooler's display, and rotation, scrolling and the other display rules don't touch them.
- **Which input was the CPU fan and which the pump** on the owner's board was confirmed on 2026-10-23 by a load test, the in-kernel driver's register layout, and the board's BIOS hardware monitor, which all agreed. It served only for checks on the owner's PC (`docs/local/`).
- **The chip's driver:** the cooler's fans and pump plug into the board's headers, so the board's sensor chip measures them, and its driver often isn't loaded by the kernel by itself. See [the next section](#finding-the-board-chips-driver).
- **Concept in the sensor snapshot:** the snapshot carried the two speeds beside the seven metrics (`Snapshot.cooler_fans` and `.pump`, each "not chosen" or the chosen input's reading), and the sensor thread read each listed input's speed once a second for the app's pickers. Reading all ten inputs of one chip cost about 0.03 ms (Phase 11 timing check).

## Finding the board chip's driver

The detection design, as built in Phases 6 and 11 and removed on 2026-11-18.

- OpenDeepCool didn't name the driver; lm-sensors found it.
  - At boot, when the PC had no lm-sensors configuration, neither `/etc/conf.d/lm_sensors` nor `/etc/modprobe.d/lm_sensors.conf`, a root one-shot unit ran `sensors-detect --auto`, at most once per motherboard.
  - `sensors-detect` probed the board's sensor chips and wrote `/etc/conf.d/lm_sensors`, plus `/etc/modprobe.d/lm_sensors.conf` when a driver needed options. Then it enabled and started `lm_sensors.service`, which loads the listed drivers at every boot.
  - An existing configuration was never overwritten: `sensors-detect` would overwrite either file, so detection ran only when both were absent (the script, read 2026-10-24).
  - It didn't run when a fan input outside a graphics card already existed: the chip that measures the cooler's fans and pump was then already readable (owner, 2026-10-27; solution review P6-13).
  - It had 120 s to finish, and the display service didn't wait for it. The cooler's fan and pump speeds appeared once the driver was loaded (owner, 2026-10-27; solution review P6-02, P6-03).
  - "Once per motherboard" was judged from the board's DMI vendor, name and version. A board that reports placeholder strings there could pass for the previous one; that mattered only if the lm-sensors configuration was removed in between, and was accepted (solution review P6-11).
  - Its record of what it created, and for which board, lived in a root-only directory of its own, `/var/lib/opendeepcool-detect/`, never in the display service's (owner, 2026-10-26). The record held yes/no facts, never paths: whether this run created each of the two files, and whether `lm_sensors.service` was enabled before.
  - lm-sensors warns that its SMBus probing "has been reported to cause trouble on some systems"; by default it skips that probing when a Super I/O chip already provides voltages, fans and temperatures, except on ASUS, Tyan and Supermicro boards. When it finds no sensors, it writes nothing; the once-per-motherboard record stopped it from running again at every boot.
  - Evidence: system: the `sensors-detect` script of lm_sensors 3.6.2, read 2026-10-23 (its chip table maps each chip it knows to its driver); Arch Wiki, lm_sensors. System, 2026-10-29: one detection run on the owner's PC wrote `/etc/conf.d/lm_sensors`, enabled and started `lm_sensors.service`, and the board's sensor chip, with its fan inputs, was in `/sys/class/hwmon` right away, without a reboot (details: `docs/local/`).
- **The unit's sandbox** (Phase 11 plan, Task 2; Arch's security package guidelines; lm_sensors 3.6.2's script, read 2026-11-10): root, because loading drivers and probing I/O ports need it, with only `CAP_SYS_MODULE` (modprobe) and `CAP_SYS_RAWIO` (`/dev/port`); `NoNewPrivileges`, `ProtectSystem=strict` with write access to `/etc/conf.d` and `/etc/modprobe.d` only, `ProtectHome`, `PrivateTmp`, `PrivateNetwork`, `IPAddressDeny=any`, `RestrictAddressFamilies=AF_UNIX` (systemctl and udevadm settle talk to local sockets), `MemoryDenyWriteExecute`, `SystemCallFilter=@system-service` without `@privileged @resources` but with `@module`, `DevicePolicy=closed` allowing `/dev/port`, the i2c character devices and `cpuid`. Not `ProtectKernelModules=` (it loads the drivers it finds) and not `ProcSubset=pid` (it reads `/proc/cpuinfo` and `/proc/modules`). `TimeoutStartSec=120`, `Condition`s on the two absent files.
- **A helper unit** loaded `i2c-dev` and `cpuid` just before the detection, under the same conditions: a device group that doesn't exist when a unit starts can't be allowed (`systemd.resource-control(5)`), and `sensors-detect` loads both itself in the same run. They stay loaded until the next boot.
- **The package's undo:** on removal the install script read the record, only as root's own regular file in a root-only (0700) folder, through no link, in the exact layout the detection wrote (version 1, ten lines), and removed only what the record said the detection created (`/etc/conf.d/lm_sensors`, `/etc/modprobe.d/lm_sensors.conf`), disabling `lm_sensors.service` if the detection enabled it (solution review OV-01). A record that wasn't as written left lm-sensors' files as they were. The package test (`odc-pkg-test`) covered it with several cases.
- **The package depended on** `lm_sensors` for `sensors-detect` and `lm_sensors.service`.

## The saved settings

Two optional keys of `settings.toml`, both absent by default:

```toml
[cooler_fan_input]   # and [pump_input]
chip = "<hwmon chip name, 1 to 64 bytes>"
device_path = "devices/…"   # relative to the sysfs root, at most 1024 bytes, no empty, "." or ".." component
input = 1                   # the fan input's number, from 1
```

The file was checked for form only: a saved input missing from the PC for now was kept for when it came back (owner, 2026-10-26; solution review OV-14).

## The service's contract

Removed from `docs/spec/dbus.md` (details as they stood):

- **Methods** `SetCoolerFanInput(ssu)` and `SetPumpInput(ssu)`, polkit action `change-settings`: the fan input listed in `FanInputs` with that chip, device path and number, or all three empty for none; errors `NotAuthorized`, `InvalidArgument`, `Failed`. Arguments were compared only with what the service found itself: a fan input had to be listed in `FanInputs`, a string over its limit (64 bytes; 1024 for a device path) was refused before any lookup, and no file path was ever built from a caller's string (owner, 2026-10-26).
- **State keys:**

  | Key | Type | Meaning |
  |---|---|---|
  | `FanInputs` | `a(ssusd)` | Each fan input: chip, device path relative to the sysfs root (`devices/…`), input number, the driver's label (empty when it gives none), and its speed in whole RPM capped at 9999, NaN when it can't be read |
  | `CoolerFanInput` | `(ssu)` | The input chosen for the cooler's fans: chip, device path and number, all empty when none is chosen |
  | `PumpInput` | `(ssu)` | The same, for the pump |
  | `CoolerFanInputPresent` | `b` | Whether the chosen input for the cooler's fans is there; true when none is chosen |
  | `PumpInputPresent` | `b` | The same, for the pump |
  | `CoolerFanRpm` | `d` | The cooler's fans, in whole RPM capped at 9999; NaN when no input is chosen or it can't be read |
  | `PumpRpm` | `d` | The same, for the pump |

- **Watchers:** `FanInputs`, whose speeds changed every second, went only to watchers that also watched frames (`WatchFrames`), about once a second, in `StateChanged`.
- **`OverrideReadings`** (the dev copy's test interface): the fans kept their real readings.
- **Entry points:** the two setters were in the entry-points table's row "The service's D-Bus methods", and the detection record had a row of its own (root only, 0700; a record or directory that isn't root's alone, a symbolic link, or an unknown or path-valued field: nothing run, written or removed).

## The settings app's screens

As the Phase 10 design drew them, so a later version can redraw them:

- **Preview column:** two rows under the status, "Cooler fans" and "Pump". Each value was one of: `1,180 RPM`; "Choose input", a link-style button that opened the Configuration page; "Input not found"; "No reading" (the chosen input is there but its speed can't be read); "No fan sensors found"; "—" while the service was down. The speeds came from the chosen inputs and never reached the cooler.
- **Configuration page, the Cooler card:** two drop-downs, "Cooler fans input" and "Pump input", with a custom list-item factory. Entries: "None" first (the default), then every fan input except a graphics card's own fans, each `<chip> · <label or fanN>` with its live RPM right-aligned in tabular figures, updating in place while the list was open. A saved input that was missing showed `<chip> · <input> — not found` with a warning border and the hint "The saved input isn't found. Pick another, or it comes back when the input does." With no fan sensors both drop-downs were insensitive and read "No fan sensors found", with the hint "No fan sensors were found. The board's sensor driver may not be loaded; the journal says more." The card's hint: "Pick the inputs your cooler's radiator fans and pump are plugged into. Their speeds show in the app only, never on the cooler."
- **Accessible names:** "Cooler fans speed", "Pump speed", "Choose cooler fans input", "Choose pump input".
- **States the design listed:** a saved fan input missing; no fan sensors found for the cooler's fans and pump.

## The parity checklist rows

- **S9:** Cooler fans (DeepCreative's CPU Fan I/O) and pump: the user picks a fan input for each, from every sensor chip except a graphics card's, with none chosen by default; their RPM shows in the settings app, never on the cooler. Evidence: code: LP page settings, helper's fan list (traced 2026-10-23). Owner check: reviewed 2026-10-23, owner decision. Tests: `every_board_input_is_listed_and_no_graphics_card_fan_is`, `the_snapshot_carries_the_chosen_fans_and_none_by_default` and `a_chosen_input_that_is_gone_is_no_device_never_another_inputs_speed` (`crates/odc-sensors/tests/fans.rs`), and `fan_speeds_reach_only_frame_watchers_and_follow_the_readings` (`crates/odc-service/tests/dbus.rs`).
- **T1:** first start: mode 1, rotation 0, Celsius, no fan inputs (the fan part).

## What the PRD decided, and when

- **2026-10-22 and 2026-10-23:** a fixed module for the owner's board was first chosen, then replaced: the product had to work on any PC, so the user picks the inputs, as in DeepCreative, and lm-sensors finds the driver (alternatives rejected: a fixed module for one board; out-of-tree DKMS drivers).
- **2026-10-23:** the product's scope was the cooler: its display, and its fans and pump in the app; CPU, GPU and RAM are read only for the display's values.
- **2026-10-27 (solution review P6-02):** the detection is automatic, made safer: it doesn't run when a fan reading already exists, has 120 s, and the display service doesn't wait for it; the install docs say so beforehand. Rejected: detection on request in the app behind an administrator's password.
- **2026-11-18:** all of it removed (owner, in the UI review).

## Speed control (never built)

v1 only read the speeds. They were DeepCool hardware, so controlling them might have come after v1 (owner, 2026-10-23). It would need a driver that can write (the in-kernel driver for the board's sensor chip is read-only), and a bug could slow or stop the pump. Other fans stayed out. The project's safety rule, which stays in `AGENTS.md` rule 8 as one sentence, is that OpenDeepCool writes no fan or power setting and never writes to hwmon `pwm*` files.

## Open points a later version would settle

- How HWiNFO-style labels could be matched on Linux so a board's headers were named (the kernel gives `fanN_label` only where a driver provides one).
- Whether the driver detection should run on request instead of at boot.
