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

- Every rule is tested against fake sysfs trees built from its documentation. Some were also checked on real hardware, on the owner's PC; which ones is a machine fact, kept in `docs/local/`, because the drivers a PC uses reveal its parts' makes (AGENTS.md rule 10; owner, 2026-10-27).
- A metric with no matching rule is unavailable, never drawn as a wrong number.
- Driver and label names are allowed here by the AGENTS.md rule 10 exception.

| Metric | Where | Match | File and unit | Notes |
|---|---|---|---|---|
| CPU Temperature | hwmon | `name` is `coretemp`; the channel whose `tempN_label` is `Package id 0` | `tempN_input`, millidegrees C | — |
| CPU Temperature | hwmon | `name` is `k10temp`; the channel labelled `Tdie` if there is one, else `Tctl` | `tempN_input`, millidegrees C | kernel docs: `Tdie` is the real die temperature where it differs from `Tctl`. On CPUs older than family 17h, `Tctl` isn't a physical temperature, so the metric is unavailable there |
| CPU Usage | `/proc/stat` | the aggregate `cpu` line | jiffies; busy = total − (idle + iowait) over the interval, kept as whole numbers ([display.md](display.md#values), exact whole-number arithmetic) | — |
| CPU Power | powercap | every zone directly under the control type `intel-rapl` whose `name` starts with `package-`: `package-N`, or `package-N-die-M` on multi-die parts. Never a sub-zone (`core`, `uncore`, `dram`) and never an `intel-rapl-mmio` zone | `energy_uj`, microjoules; power = the zones' ΔµJ added up ÷ Δµs, kept as whole numbers; each zone wraps at its `max_energy_range_uj`. Root-only, see [CPU power](#cpu-power) | AMD and Hygon: the same zones. Kernel source, read 2026-10-24: `intel_rapl_msr.c` gives AMD families 17h, 19h and 1Ah and Hygon family 18h the RAPL driver's AMD defaults under the control type `intel-rapl`, and `intel_rapl_common.c` names the package zones |
| Gpu temperature | hwmon | `name` is `amdgpu`; the channel whose `tempN_label` is `edge` | `tempN_input`, millidegrees C | — |
| Gpu usage | DRM | the chosen GPU's card, device driver `amdgpu` | `device/gpu_busy_percent`, percent | — |
| Gpu power | hwmon | `name` is `amdgpu`: `power1_average`, else `power1_input` | microwatts | `power1_input` (instantaneous power) from the amdgpu docs. On integrated AMD graphics this reading includes the CPU (amdgpu docs), so Gpu power is unavailable there (rule 11). OpenDeepCool shows it only for a GPU ranked dedicated; on an AMD GPU of unknown kind it's unavailable too, because that GPU might be integrated (owner, 2026-10-27; solution review P8-06) |
| Gpu temperature | hwmon | `name` is `i915` | `temp1_input`, millidegrees C | kernel ABI `sysfs-driver-intel-i915-hwmon`: "GPU package temperature"; only some i915 platforms have this hwmon, and the metric is unavailable on the others |
| Gpu power | hwmon | `name` is `i915` | `energy1_input`, microjoules; power = ΔµJ ÷ Δµs over the interval, kept as whole numbers | same document and platform note |
| Gpu temperature | hwmon | `name` is `xe` | `temp2_input`, millidegrees C | kernel ABI `sysfs-driver-intel-xe-hwmon`: "Package temperature"; same platform note |
| Gpu power | hwmon | `name` is `xe` | `energy1_input`, microjoules ("Card energy"); power = ΔµJ ÷ Δµs over the interval, kept as whole numbers | same document |
| Gpu usage | — | `i915`, `xe` | — | unavailable, with the reason that the GPU's driver doesn't report usage: neither driver documents a busy percentage (kernel ABI documents, read 2026-10-24) |
| Gpu temperature, usage, power | NVML | the chosen GPU's driver is NVIDIA's `nvidia`; the NVML device whose PCI bus ID is the chosen GPU's address | temperature: the GPU sensor, whole °C; usage: the GPU utilisation rate, whole percent; power: the power usage, milliwatts | NVIDIA's driver has no hwmon interface (a pull request to add one exists, NVIDIA open-gpu-kernel-modules #1169, status unverified). Read through `nvml-wrapper`: `temperature(TemperatureSensor::Gpu)`, `utilization_rates().gpu`, `power_usage()`, matched by `pci_info().bus_id` (its documentation, read 2026-10-24) |
| Gpu metrics | — | any other GPU driver, such as `nouveau` | — | unavailable, with the reason that there's no reading for this GPU's driver: none is documented (docs.kernel.org documents no nouveau hwmon readings, read 2026-10-24) |
| Memory Load | `/proc/meminfo` | `MemTotal`, `MemAvailable` | kB; load = (MemTotal − MemAvailable) ÷ MemTotal, kept as whole numbers | — |
| Cooler fans (CPU Fan I/O), pump | hwmon | no rule: the user picks any `fanN_input` from any hwmon device except a graphics card's ([Fan and pump speed](#fan-and-pump-speed)) | RPM | — |

- **The chosen GPU only:** every Gpu row reads the GPU picked by the [GPU choice](#gpu-choice) rules. An hwmon device counts only when its `device` resolves to that GPU's PCI address, and a DRM card only when its `device` is that GPU.
- **Board sensor driver** (the cooler's fans and pump plug into the board's headers, so the board's sensor chip is what measures them): a motherboard's sensor-chip driver often isn't loaded automatically. OpenDeepCool doesn't name the driver; lm-sensors finds it.
  - At boot, when the PC has no lm-sensors configuration, neither `/etc/conf.d/lm_sensors` nor `/etc/modprobe.d/lm_sensors.conf`, a root one-shot unit runs `sensors-detect --auto`, at most once per motherboard.
  - `sensors-detect` probes the board's sensor chips and writes `/etc/conf.d/lm_sensors`, plus `/etc/modprobe.d/lm_sensors.conf` when a driver needs options. Then it enables and starts `lm_sensors.service`, which loads the listed drivers at every boot.
  - An existing configuration is never overwritten: `sensors-detect` would overwrite either file, so detection runs only when both are absent (the script, read 2026-10-24).
  - It doesn't run when a fan input outside a graphics card already exists: the chip that measures the cooler's fans and pump is then already readable (owner, 2026-10-27; solution review P6-13).
  - It has 120 s to finish, and the display service doesn't wait for it. The cooler's fan and pump speeds appear once the driver is loaded (owner, 2026-10-27; solution review P6-02, P6-03).
  - "Once per motherboard" is judged from the board's DMI vendor, name and version. A board that reports placeholder strings there could pass for the previous one; that matters only if the lm-sensors configuration was removed in between, and is accepted (solution review P6-11).
  - Its record of what it created, and for which board, lives in a root-only directory of its own, `/var/lib/opendeepcool-detect/`, never in the display service's (owner, 2026-10-26). The record holds yes/no facts, never paths: whether this run created each of the two files, and whether `lm_sensors.service` was enabled before.
  - lm-sensors warns that its SMBus probing "has been reported to cause trouble on some systems"; by default it skips that probing when a Super I/O chip already provides voltages, fans and temperatures, except on ASUS, Tyan and Supermicro boards. When it finds no sensors, it writes nothing; the once-per-motherboard record stops it from running again at every boot.
  - Evidence: system: the `sensors-detect` script of lm_sensors 3.6.2, read 2026-10-23 (its chip table maps each chip it knows to its driver; the owner's chip: `docs/local/`); Arch Wiki, lm_sensors. System, 2026-10-29: one detection run on the owner's PC wrote `/etc/conf.d/lm_sensors`, enabled and started `lm_sensors.service`, and the board's sensor chip, with its fan inputs, was in `/sys/class/hwmon` right away, without a reboot (details: `docs/local/`).
- **NVML in the display service** (owner, 2026-10-27; solution review P6-06): NVIDIA's closed library is the one piece of code that isn't OpenDeepCool's own inside the service. It's loaded only when the chosen GPU uses NVIDIA's `nvidia` driver, never on other PCs. Its failures name their cause: the library can't be loaded or started, with the loader's or NVML's own message, which tells a missing library from a refused one; the driver was updated without a reboot; or no permission (solution review P6-12). NVIDIA's driver not loaded yet counts as no device, so a driver that loads late is still found. NVML inside the service's sandbox is untested; a system call the sandbox refuses fails instead of stopping the service (owner, 2026-10-27; overview review's cross-check).
- **Sources that appear late** (owner, 2026-10-27; solution review P6-04): a metric without a source is looked for again every 5 s for the first two minutes after start-up or a wake-up, then every 60 s. Only after those two minutes does "no device" count as lasting and disable its modes in the app.
- **Evidence:** system: each rule checked on the owner's PC matched exactly one channel there, 2026-10-22, and its value agreed with `sensors` (which rules: `docs/local/`). File meanings and units: docs.kernel.org (hwmon sysfs interface, coretemp, k10temp, amdgpu thermal, powercap), the kernel ABI documents for the i915 and xe hwmon, and the kernel's RAPL sources (read 2026-10-24). NVML's calls: the `nvml-wrapper` documentation (read 2026-10-24 and 2026-10-27).
- **Unverified:** the CPU usage formula is the common definition of "busy". It's checked against `top` in Phase 6.

## Updates and rounding

- **Refresh:** OpenDeepCool reads every metric once per second, as DeepCreative does. CPU usage and CPU power are averages over that second, computed from two counter readings. Gpu usage is the driver's own percentage, shown as read; the window it covers isn't documented (**unverified**). System: on the owner's PC it rose from 2 % to 99–100 % within 2 s of a GPU load (an `ffmpeg` Vulkan filter) and was back at 2 % about 3 s after it stopped, 2026-10-23.
- **Gpu power** is likewise the driver's own value, shown as read; the window it averages over isn't documented (**unverified**). System, on the owner's PC: `sensors` shows the same reading (a read right after `sensors` matched it 7 times out of 8). At idle it held steady; under a full GPU load, reads a fraction of a second apart differed by up to about a third, 2026-10-23.
- **Rounding:** every reading keeps its source's whole-number units (jiffies, kB, millidegrees, µW, mW, µJ over µs) until it's drawn. [display.md](display.md#values) defines the rounding: every measurement rounds to the nearest whole number, halves up, in integer arithmetic only (owner, 2026-10-26). DeepCreative rounds usage, temperature and power up.
- **Units:** temperatures are read in °C; °F is a display conversion ([display.md](display.md#values)).
- **After standby** (owner, 2026-10-26): when the PC wakes from sleep, every reading averaged over the interval starts a new one, as at start-up: CPU usage, and power from an energy counter (CPU power, and Gpu power on Intel graphics). So no average spans the sleep; until that first second has passed, the cooler shows dashes for them. What the counters do during sleep then doesn't matter (**unverified**).

## GPU choice

OpenDeepCool reads a GPU only for the display's GPU metrics: temperature, usage and power. It doesn't monitor GPUs otherwise (owner, 2026-10-23: OpenDeepCool covers only the cooler).

- **Kinds:** each GPU in the PC is either **dedicated** (a graphics card) or **integrated** (part of the CPU). Each driver's documented signal tells them apart (Phase 6 plan, read 2026-10-24): amdgpu's `AMDGPU_IDS_FLAGS_FUSION`, xe's `DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM`, i915's device-local memory region, and CUDA's `CU_DEVICE_ATTRIBUTE_INTEGRATED` for NVIDIA, asked by a short-lived helper. A driver without a signal, such as nouveau, gives an unknown kind, which ranks after an integrated GPU (owner, 2026-10-27; solution review P6-07).
- **The rank helper** (owner's design, 2026-10-24): a short-lived program, `odc-gpu-rank`, asks each GPU's driver for its kind, writes the result to `gpu-rank.json` in the display service's state directory, and exits.
  - It runs when OpenDeepCool starts, and only when no ranking file exists, so normally once.
  - It's the only OpenDeepCool program with `unsafe` code and the only one that opens a GPU's device nodes. The display service reads only sysfs and procfs, plus NVML on a PC whose chosen GPU is NVIDIA's (above).
- **A failed kind query** (owner, 2026-10-27; solution review P6-07): when the query that tells a GPU's kind fails, the GPU is unknown and ranks after an integrated one, like any unknown GPU. The failure is logged as a warning and named in the GPU list, apart from a driver that simply has no signal.
- **Rules** (owner, 2026-10-23):
  1. With one GPU, of either kind, OpenDeepCool reads it, and the settings app shows no GPU choice.
  2. With more than one, a dedicated GPU comes before an integrated one.
  3. With more than one, the settings app shows a dropdown to switch between them. On the first run it's set to the dedicated GPU.
  4. The user's choice is saved with the other settings and kept across reboots.
- **Details** (owner, 2026-10-23):
  - The choice is saved by the GPU's PCI address, because DRM card numbers can change between boots. That the PCI address stays the same while the hardware stays the same is **unverified**: checking it would take reboots, which aren't tested (owner, 2026-10-25). A GPU whose address changed counts as missing.
  - If the saved GPU is missing at start-up, rules 1–2 pick one, and the saved choice is kept for when it's back.
  - Two GPUs of the same kind are ordered by PCI address.
- **Ranking:** dedicated, then integrated, then unknown, and GPUs of the same kind by PCI address. Without a saved choice, or when the saved GPU is missing, the first in this order is read.
- **When the GPUs change** (owner, 2026-10-24):
  - If the set of GPUs differs from the one the ranking recorded and the user hasn't chosen a GPU, the service deletes the ranking, and the helper ranks again at the next start.
  - Once the user has chosen, the helper never runs again. A new GPU then appears in the list as unknown, ranked last.
  - A ranking file that can't be read, or that doesn't have the expected form, is deleted whether or not the user has chosen; until the next ranking, every GPU is unknown (overview review's cross-check, 2026-10-27).
- **DeepCreative** picks one "current GPU" and reads its group. With integrated graphics it can also take GPU readings from the CPU group (the integrated-graphics labels above). It also has its own GPU list and default-GPU setting (main-process strings `app/get-gpu-list`, `app/set-default-gpu`). How it picks by default wasn't traced, and OpenDeepCool follows the rules above instead. Code: the helper's GPU selection and label list.

## CPU power

- Since a kernel security fix, the RAPL energy counters are readable by root only. Reading them lets any local user mount the "Platypus" power side-channel attack (CVE-2020-8694; Intel advisory INTEL-SA-00389, which names `/sys/class/powercap/intel-rapl*/*/energy_uj`). System: on the owner's PC the counter is root-only (2026-10-22).
- So the display service, which runs as an unprivileged system user (PRD, Service model), can't read CPU power without an install-time change. Making the file readable by **every** user would reopen that attack for all local accounts. The options were:
  - a rule that lets **only** the display service's account read it;
  - a small privileged helper that serves only the power value.

  The owner chose the rule on 2026-10-24: a line in OpenDeepCool's udev rule lets the `opendeepcool` group read the package zones' `energy_uj` (Phase 6 Task 6). The PRD's open question "is CPU power readable without root?" is answered: **no**.
- **The rule's line:**
  - It matches the zones by kernel name, `intel-rapl:*`, and by zone name, `package-*`, never by index. So the sub-zones (`core`, `uncore`, `dram`) and the `intel-rapl-mmio` zones stay root-only.
  - It acts on `add` and `change` events, so a zone created again, after a module reload for example, gets the group's access back.
  - It covers AMD and Hygon CPUs too, whose package zones use the same control type ([Driver rules](#driver-rules)).
  - In the product, only the display service's account is in the group. On the development PC the owner's account is too (PRD, System Footprint).

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
  - it leaves out a graphics card's own fans, as DeepCreative does (owner, 2026-10-23): a pump or radiator fan never connects to a GPU. The rule: skip an hwmon device whose `device` is a PCI display controller, PCI base class `0x03` (kernel `pci_ids.h`: `PCI_BASE_CLASS_DISPLAY`). System, 2026-10-23 (details in `docs/local/`): a graphics card's fan device has PCI class `0x030000`, while the board chip's is a platform device;
  - the user picks one input for the cooler's fans (DeepCreative's CPU Fan I/O: the radiator fans sit on the board's CPU fan header) and one for its pump. None is chosen by default, as in DeepCreative. No rule could choose: the kernel's hwmon documentation says "There is no standard that ensures, for example, that … the second fan is on the CPU";
  - the choice is saved as the chip's name, its device path and the input number, never as an hwmon index, which changes between boots. If that input is missing, the app shows it as not found, never another input's RPM;
  - the settings app shows their RPM, and nothing reaches the display.
- lm-sensors finds and loads the board's sensor driver ([Driver rules](#driver-rules), Board sensor driver).
- Which input is the CPU fan and which is the pump on the owner's board: `docs/local/`. Confirmed on 2026-10-23 by a load test, the driver's register layout and the board's BIOS hardware monitor, which all agree. Evidence: owner check (BIOS). It's only for checks on the owner's PC: the program never relies on it.
