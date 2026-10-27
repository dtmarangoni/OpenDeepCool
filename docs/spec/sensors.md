# Sensors: what each metric means and where Linux has it

The readings behind every number DeepCreative 1.2.14 shows: which reading it takes, its unit, how often it updates, and the Linux source OpenDeepCool reads instead. How numbers are rounded and drawn is in [display.md](display.md#values); evidence labels are defined in [README.md](README.md).

Which driver and label provide each reading on the owner's PC, and which board fan input is the CPU fan or the pump, are machine facts: see `docs/local/`.

**Scope** (owner, 2026-10-23): OpenDeepCool reads only the seven values the display shows ([Metrics](#metrics)) and the speeds of the cooler's own fans and pump ([Fan and pump speed](#fan-and-pump-speed)). It reads nothing else, such as clocks, voltages or other fans, and never writes to any device but the LP360.

## How DeepCreative gets its readings

- A helper program reads the PC's sensors through HWiNFO's library and passes them to a small service. DeepCreative's main process asks that service for a snapshot about **once per second**. Evidence: system: the service's log of 2026-10-22 polls its helper every 1000 ms; code: the service's configuration.
- Each snapshot maps a metric name to a value and unit, for example `CPU Temperature` → 57 °C. The LP page reads only the names in the table below. Evidence: code: main-process strings; capture: injected snapshots drive every mode (display.md).
- The helper fills each metric by looking for HWiNFO readings with particular labels, in a particular sensor group (CPU, GPU, system), and of the matching kind (temperature, power, usage, memory). Evidence: code: the helper's metric-mapping function and its label list (SensorWorker.exe, static analysis 2026-10-22).
- When several readings match one metric, the helper collects them all. Which one DeepCreative then shows is **unverified**: it wasn't traced. OpenDeepCool reads one defined source per metric, so this doesn't change what it draws.

## Metrics

| Metric (DeepCreative's name) | Meaning | HWiNFO labels DeepCreative accepts | Unit | OpenDeepCool's Linux source |
|---|---|---|---|---|
| CPU Temperature | the CPU package temperature | "CPU Package" (Intel); "CPU (Tctl/Tdie)", "CPU (Tdie)" (AMD) | °C | the CPU temperature driver's package reading (hwmon) |
| CPU Usage | total CPU load across all logical CPUs | "Total CPU Usage", "Total CPU Utility" | % | `/proc/stat`: the busy share of the aggregate `cpu` line over the last interval |
| CPU Power | power drawn by the CPU package | "CPU Package Power" | W | the package energy counter (powercap RAPL `energy_uj`), as energy difference ÷ time. **Root-only**: see [CPU power](#cpu-power) |
| Gpu temperature | the GPU's main temperature, **not** its hot spot or memory | "GPU Temperature", "GPU Core Temperature", "GPU Global Temperature"; for integrated graphics "CPU GT Cores (Graphics)", "CPU (Tdie)" | °C | the chosen GPU's main temperature from its driver ([GPU choice](#gpu-choice)), not a hot-spot or memory temperature |
| Gpu usage | how busy the GPU is | "GPU Total Usage", "GPU Utilization", "GPU Core Load"; for integrated graphics "GPU D3D Usage" | % | the chosen GPU's busy percentage from its driver |
| Gpu power | power drawn by the graphics card | "GPU Power", "Total GPU Power", "GPU Power (Total)", "Total Graphics Power (TGP)", "GPU ASIC Power", "Total GPU Power (TGP)" | W | the chosen GPU's board power from its driver |
| Memory Load | share of physical memory in use | "Physical Memory Load" | % | `/proc/meminfo`: (MemTotal − MemAvailable) ÷ MemTotal |

Evidence:
- **Labels:** code: the helper's label list and mapping function (static analysis, 2026-10-22).
- **Meanings:** code: the HWiNFO sample DeepCool ships with the app.
  - A GPU in it reports both "GPU Temperature" (58 °C) and "GPU Hot Spot Temperature" (60 °C), and only the first is accepted.
  - Its Physical Memory Load (65.5 %) equals used ÷ (used + available) (41,246 ÷ 62,932 MB).
- **Linux sources:** system: each one was read on the owner's PC on 2026-10-22, found by name or driver, and agreed with `sensors` and `/proc`.
- **Unverified equivalences:** HWiNFO's definitions and the Linux files are measured by different software. That the Linux GPU busy percentage and board power equal HWiNFO's "GPU Utilization" and "Total Graphics Power" is **inferred** from their names, not compared side by side; Windows isn't available.
- **Quirk, not reproduced (AGENTS.md rule 11):** for integrated graphics, DeepCreative also accepts "CPU (Tdie)" as the GPU temperature, which is the CPU die's temperature. OpenDeepCool never shows a CPU reading as the GPU's: it reads the integrated GPU's own sensor, or shows the metric as unavailable. Evidence: code: the helper's label list and mapping function (static analysis, 2026-10-22 and 2026-10-23).

## Driver rules

The built-in rules the sensor registry matches (PRD, Sensor architecture). They cover the common drivers on any PC (owner, 2026-10-23), each taken from its kernel documentation, and match by name or driver, never by index.

- Rules marked **owner's PC** were checked on real hardware. The others are **not tested on hardware**, only against fake sysfs trees built from the documentation.
- A metric with no matching rule is unavailable, never drawn as a wrong number.
- Driver and label names are allowed here by the AGENTS.md rule 10 exception.

| Metric | Where | Match | File and unit | Checked |
|---|---|---|---|---|
| CPU Temperature | hwmon | `name` is `coretemp`; the channel whose `tempN_label` is `Package id 0` | `tempN_input`, millidegrees C | owner's PC |
| CPU Temperature | hwmon | `name` is `k10temp`; the channel labelled `Tdie` if there is one, else `Tctl` | `tempN_input`, millidegrees C | kernel docs: `Tdie` is the real die temperature where it differs from `Tctl`. On CPUs older than family 17h, `Tctl` isn't a physical temperature, so the metric is unavailable there |
| CPU Usage | `/proc/stat` | the aggregate `cpu` line | jiffies; busy = total − (idle + iowait) over the interval, kept as whole numbers ([display.md](display.md#values), exact whole-number arithmetic) | owner's PC |
| CPU Power | powercap | the zone whose `name` is `package-0` | `energy_uj`, microjoules; power = ΔµJ ÷ Δµs, kept as whole numbers; wraps at `max_energy_range_uj`. Root-only, see [CPU power](#cpu-power) | owner's PC (Intel). AMD: the same zone since kernel 5.8 (5.11 for family 19h) according to Scaphandre's documentation; **unverified** until Phase 6 checks the kernel source |
| Gpu temperature | hwmon | `name` is `amdgpu`; the channel whose `tempN_label` is `edge` | `tempN_input`, millidegrees C | owner's PC |
| Gpu usage | DRM | the chosen GPU's card, device driver `amdgpu` | `device/gpu_busy_percent`, percent | owner's PC |
| Gpu power | hwmon | `name` is `amdgpu`: `power1_average` (labelled `PPT` on the owner's PC), else `power1_input` | microwatts | owner's PC for `power1_average`; `power1_input` (instantaneous power) from the amdgpu docs. On integrated AMD graphics this reading includes the CPU (amdgpu docs), so Gpu power is unavailable there (rule 11) |
| Gpu temperature, usage, power | NVML | NVIDIA GPUs | NVIDIA's library | **unverified**: NVIDIA's driver has no hwmon interface (a pull request to add one exists, NVIDIA open-gpu-kernel-modules #1169, status unverified). Phase 6 chooses the NVML calls from NVIDIA's documentation |
| Gpu metrics | — | other GPU drivers, such as Intel `i915` and `xe`, or `nouveau` | — | no rule yet: Phase 6 researches what their documentation offers; until then unavailable |
| Memory Load | `/proc/meminfo` | `MemTotal`, `MemAvailable` | kB; load = (MemTotal − MemAvailable) ÷ MemTotal, kept as whole numbers | owner's PC |
| Cooler fans (CPU Fan I/O), pump | hwmon | no rule: the user picks any `fanN_input` from any hwmon device except a graphics card's ([Fan and pump speed](#fan-and-pump-speed)) | RPM | — |

- **Board sensor driver** (the cooler's fans and pump plug into the board's headers, so the board's sensor chip is what measures them): a motherboard's sensor-chip driver often isn't loaded automatically (on the owner's PC, `nct6683` isn't). OpenDeepCool doesn't name the driver; lm-sensors finds it.
  - At boot, when the PC has no lm-sensors configuration (`/etc/conf.d/lm_sensors`), a root one-shot unit runs `sensors-detect --auto`, at most once per motherboard.
  - `sensors-detect` probes the board's sensor chips, writes that file, then enables and starts `lm_sensors.service`, which loads the listed drivers at every boot.
  - An existing configuration is never overwritten.
  - It doesn't run when a fan input outside a graphics card already exists: the chip that measures the cooler's fans and pump is then already readable (owner, 2026-10-27; solution review P6-13).
  - It has 120 s to finish, and the display service doesn't wait for it. The cooler's fan and pump speeds appear once the driver is loaded (owner, 2026-10-27; solution review P6-02, P6-03).
  - "Once per motherboard" is judged from the board's DMI vendor, name and version. A board that reports placeholder strings there could pass for the previous one; that matters only if the lm-sensors configuration was removed in between, and is accepted (solution review P6-11).
  - Its record of what it created, and for which board, lives in a root-only directory of its own, `/var/lib/opendeepcool-detect/`, never in the display service's (owner, 2026-10-26). The record holds yes/no facts, never paths.
  - lm-sensors warns that its SMBus probing "has been reported to cause trouble on some systems"; by default it skips that probing when a Super I/O chip already provides voltages, fans and temperatures, except on ASUS, Tyan and Supermicro boards. When it finds no sensors, it writes nothing; the once-per-motherboard record stops it from running again at every boot.
  - Evidence: system: the `sensors-detect` script of lm_sensors 3.6.2, read 2026-10-23 (its chip table maps the owner's sensor chip to `nct6683`); Arch Wiki, lm_sensors.
- **NVML in the display service** (owner, 2026-10-27; solution review P6-06): NVIDIA's closed library is the one piece of code that isn't OpenDeepCool's own inside the service. It's loaded only when the chosen GPU uses NVIDIA's `nvidia` driver, never on other PCs. Its failures name their cause: the library is missing, the driver was updated without a reboot, or no permission (solution review P6-12).
- **Sources that appear late** (owner, 2026-10-27; solution review P6-04): a metric without a source is looked for again every 5 s for the first two minutes after start-up or a wake-up, then every 60 s. Only after those two minutes does "no device" count as lasting and disable its modes in the app.
- **Evidence:** system: every rule marked owner's PC matched exactly one channel on the owner's PC, 2026-10-22, and its value agreed with `sensors`. File meanings and units: docs.kernel.org (hwmon sysfs interface, coretemp, k10temp, amdgpu thermal, powercap).
- **Unverified:** the CPU usage formula is the common definition of "busy". It's checked against `top` in Phase 6.

## Updates and rounding

- **Refresh:** OpenDeepCool reads every metric once per second, as DeepCreative does. CPU usage and CPU power are averages over that second, computed from two counter readings. Gpu usage is the driver's own percentage, shown as read; the window it covers isn't documented (**unverified**). System: on the owner's PC it rose from 2 % to 99–100 % within 2 s of a GPU load (an `ffmpeg` Vulkan filter) and was back at 2 % about 3 s after it stopped, 2026-10-23.
- **Gpu power** is likewise the driver's own value (`power1_average`), shown as read; the window it averages over isn't documented (**unverified**). System: `sensors` reads the same file (a read right after `sensors` matched it 7 times out of 8). At idle it held steady; under a full GPU load, reads a fraction of a second apart differed by up to about a third, so the once-per-second value moves by tens of watts, 2026-10-23.
- **Rounding:** every reading keeps its source's whole-number units (jiffies, kB, millidegrees, µW, mW, µJ over µs) until it's drawn. [display.md](display.md#values) defines the rounding: every measurement rounds to the nearest whole number, halves up, in integer arithmetic only (owner, 2026-10-26). DeepCreative rounds usage, temperature and power up.
- **Units:** temperatures are read in °C; °F is a display conversion ([display.md](display.md#values)).
- **After standby** (owner, 2026-10-26): when the PC wakes from sleep, every reading averaged over the interval starts a new one, as at start-up: CPU usage, and power from an energy counter (CPU power, and Gpu power on Intel graphics). So no average spans the sleep; until that first second has passed, the cooler shows dashes for them. What the counters do during sleep then doesn't matter (**unverified**).

## GPU choice

OpenDeepCool reads a GPU only for the display's GPU metrics: temperature, usage and power. It doesn't monitor GPUs otherwise (owner, 2026-10-23: OpenDeepCool covers only the cooler).

- **Kinds:** each GPU in the PC is either **dedicated** (a graphics card) or **integrated** (part of the CPU). Phase 6 verifies, from primary sources, which signal tells them apart for each driver; until then that is **unverified**. If a driver has no reliable signal, the owner decides.
- **A failed kind query** (owner, 2026-10-27; solution review P6-07): when the query that tells a GPU's kind fails, the GPU is unknown and ranks after an integrated one, like any unknown GPU. The failure is logged as a warning and named in the GPU list, apart from a driver that simply has no signal.
- **Rules** (owner, 2026-10-23):
  1. With one GPU, of either kind, OpenDeepCool reads it, and the settings app shows no GPU choice.
  2. With more than one, a dedicated GPU comes before an integrated one.
  3. With more than one, the settings app shows a dropdown to switch between them. On the first run it's set to the dedicated GPU.
  4. The user's choice is saved with the other settings and kept across reboots.
- **Details** (owner, 2026-10-23):
  - The choice is saved by the GPU's PCI address, because DRM card numbers can change between boots. That the PCI address stays the same while the hardware stays the same is **unverified**; Phase 6 checks it.
  - If the saved GPU is missing at start-up, rules 1–2 pick one, and the saved choice is kept for when it's back.
  - Two GPUs of the same kind are ordered by PCI address.
- **DeepCreative** picks one "current GPU" and reads its group. With integrated graphics it can also take GPU readings from the CPU group (the integrated-graphics labels above). It also has its own GPU list and default-GPU setting (main-process strings `app/get-gpu-list`, `app/set-default-gpu`). How it picks by default wasn't traced, and OpenDeepCool follows the rules above instead. Code: the helper's GPU selection and label list.

## CPU power

- Since a kernel security fix, the RAPL energy counters are readable by root only. Reading them lets any local user mount the "Platypus" power side-channel attack (CVE-2020-8694; Intel advisory INTEL-SA-00389, which names `/sys/class/powercap/intel-rapl*/*/energy_uj`). System: on the owner's PC the counter is root-only (2026-10-22).
- So the display service, which runs as an unprivileged system user (PRD, Service model), can't read CPU power without an install-time change. Making the file readable by **every** user would reopen that attack for all local accounts. Options for Phase 6:
  - a rule that lets **only** the display service's account read it;
  - a small privileged helper that serves only the power value.

  Phase 6 decides, with the owner. The PRD's open question "is CPU power readable without root?" is answered: **no**.

## Fan and pump speed

- DeepCreative never sends fan or pump speeds to the LP360. It shows them on its own device card, from the board fan inputs the user picks as **CPU Fan I/O** and **Pump Fan I/O**, out of the board's fan list. Evidence: code: LP page settings and the helper's fan list; Phase 1 report.
- **How DeepCreative builds the fan list** (static analysis, 2026-10-23):
  - The helper puts every fan-speed reading that HWiNFO reports for the motherboard's sensor chip into a "Fan list", which maps a name to an RPM. That's the HWiNFO sensor group that also holds the board's +3.3 V, +5 V and +12 V readings. Fan readings from other groups, such as a graphics card's own fan, are left out. Evidence: code: the helper's metric-mapping function (SensorWorker.exe).
  - Each name is HWiNFO's own label for that reading. DeepCreative adds no names and doesn't match any name. Where HWiNFO knows the board, its labels name the board's headers: the HWiNFO sample DeepCool ships labels its board fans "System 2" to "System 4". How HWiNFO picks its labels is closed and **unverified**.
  - Both dropdowns show "N/A" first, which is the default (no input), then the names from the list. Nothing is chosen automatically. Evidence: code: the LP page's I/O settings component and its store defaults; system: the first-start store held no fan inputs ([settings.md](settings.md#settings)).
  - The saved choice is the name. With each snapshot, the device card shows the RPM stored under that name. Evidence: code: the LP page's sensor handler.
  - The main process passes the list's names to the page (IPC `app/get-fan-interface`). The main process is V8 bytecode, so this is **inferred** from the page looking up each chosen name in the same list.
  - The list is keyed by name, so two readings with the same label would share one entry: **inferred**, not observed.
  - Under Wine the list is empty: HWiNFO needs its Windows kernel driver to read the sensor chip (Phase 1 report).
- OpenDeepCool does the same, on any PC:
  - the settings app lists every hwmon fan input from every sensor chip, with the chip's name, the driver's label (`fanN_label`) when it has one, and the live RPM;
  - it leaves out a graphics card's own fans, as DeepCreative does (owner, 2026-10-23): a pump or radiator fan never connects to a GPU. The rule: skip an hwmon device whose `device` is a PCI display controller, PCI base class `0x03` (kernel `pci_ids.h`: `PCI_BASE_CLASS_DISPLAY`). System: on the owner's PC the `amdgpu` fan's device has class `0x030000`, while the board chip's is a platform device, 2026-10-23;
  - the user picks one input for the cooler's fans (DeepCreative's CPU Fan I/O: the radiator fans sit on the board's CPU fan header) and one for its pump. None is chosen by default, as in DeepCreative. No rule could choose: the kernel's hwmon documentation says "There is no standard that ensures, for example, that … the second fan is on the CPU";
  - the choice is saved as the chip's name, its device path and the input number, never as an hwmon index, which changes between boots. If that input is missing, the app shows it as not found, never another input's RPM;
  - the settings app shows their RPM, and nothing reaches the display.
- lm-sensors finds and loads the board's sensor driver ([Driver rules](#driver-rules), Board sensor driver).
- Which input is the CPU fan and which is the pump on the owner's board: `docs/local/`. Confirmed on 2026-10-23 by a load test, the driver's register layout and the board's BIOS hardware monitor, which all agree. Evidence: owner check (BIOS). It's only for checks on the owner's PC: the program never relies on it.
