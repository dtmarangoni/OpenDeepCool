# Sensors: what each metric means and where Linux has it

The readings behind every number DeepCreative 1.2.14 shows: which reading it takes, its unit, how often it updates, and the Linux source OpenDeepCool reads instead. How numbers are rounded and drawn is in [display.md](display.md#values); evidence labels are defined in [README.md](README.md).

Which driver and label provide each reading on the owner's PC, and which board fan input is the CPU fan or the pump, are machine facts: see `docs/local/`.

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
| Gpu temperature | the GPU's main temperature, **not** its hot spot or memory | "GPU Temperature", "GPU Core Temperature", "GPU Global Temperature"; for integrated graphics "CPU GT Cores (Graphics)", "CPU (Tdie)" | °C | the GPU driver's main GPU temperature (hwmon), not a hot-spot or memory temperature |
| Gpu usage | how busy the GPU is | "GPU Total Usage", "GPU Utilization", "GPU Core Load"; for integrated graphics "GPU D3D Usage" | % | the GPU driver's busy-percent file of the DRM device |
| Gpu power | power drawn by the graphics card | "GPU Power", "Total GPU Power", "GPU Power (Total)", "Total Graphics Power (TGP)", "GPU ASIC Power", "Total GPU Power (TGP)" | W | the GPU driver's board power reading (hwmon) |
| Memory Load | share of physical memory in use | "Physical Memory Load" | % | `/proc/meminfo`: (MemTotal − MemAvailable) ÷ MemTotal |

Evidence:
- **Labels:** code: the helper's label list and mapping function (static analysis, 2026-10-22).
- **Meanings:** code: the HWiNFO sample DeepCool ships with the app.
  - A GPU in it reports both "GPU Temperature" (58 °C) and "GPU Hot Spot Temperature" (60 °C), and only the first is accepted.
  - Its Physical Memory Load (65.5 %) equals used ÷ (used + available) (41,246 ÷ 62,932 MB).
- **Linux sources:** system: each one was read on the owner's PC on 2026-10-22, found by name or driver, and agreed with `sensors` and `/proc`.
- **Unverified equivalences:** HWiNFO's definitions and the Linux files are measured by different software. That the Linux GPU busy percentage and board power equal HWiNFO's "GPU Utilization" and "Total Graphics Power" is **inferred** from their names, not compared side by side; Windows isn't available.

## Driver rules

The built-in rules the sensor registry matches (PRD, Sensor architecture). They target the owner's drivers only, and are found by name or driver, never by index. Driver and label names are allowed here by the AGENTS.md rule 10 exception.

| Metric | Where | Match | File and unit |
|---|---|---|---|
| CPU Temperature | hwmon | `name` is `coretemp`; the channel whose `tempN_label` is `Package id 0` | `tempN_input`, millidegrees C |
| CPU Usage | `/proc/stat` | the aggregate `cpu` line | jiffies; busy share = 1 − (idle + iowait) ÷ total, over the interval |
| CPU Power | powercap | the zone whose `name` is `package-0` | `energy_uj`, microjoules; power = difference ÷ time; wraps at `max_energy_range_uj`. Root-only, see [CPU power](#cpu-power) |
| Gpu temperature | hwmon | `name` is `amdgpu`; the channel whose `tempN_label` is `edge` | `tempN_input`, millidegrees C |
| Gpu usage | DRM | the card whose device driver is `amdgpu` | `device/gpu_busy_percent`, percent |
| Gpu power | hwmon | `name` is `amdgpu`; the channel whose `powerN_label` is `PPT` | `powerN_average`, microwatts. This driver has no `powerN_input` here |
| Memory Load | `/proc/meminfo` | `MemTotal`, `MemAvailable` | kB; load = (MemTotal − MemAvailable) ÷ MemTotal |
| CPU fan, pump | hwmon | no rule: the user picks any `fanN_input` from any hwmon device | RPM |

- **Boot:** the board's fan sensors need the kernel module `nct6683`, which isn't autoloaded on the owner's PC. The install loads it at every boot (PRD, System Footprint).
- **Evidence:** system: every rule matched exactly one channel on the owner's PC, 2026-10-22, and its value agreed with `sensors`. File meanings and units: docs.kernel.org, hwmon sysfs interface.
- **Unverified:** the CPU usage formula is the common definition of "busy". It's checked against `top` in Phase 6.

## Updates and rounding

- **Refresh:** OpenDeepCool reads every metric once per second, as DeepCreative does. Usage and power are averages over that second.
- **Rounding:** the reading keeps its fraction until it's drawn. [display.md](display.md#values) defines the rounding per metric (usage and temperature round up, Memory Load rounds to nearest, and so on).
- **Units:** temperatures are read in °C; °F is a display conversion ([display.md](display.md#values)).

## GPU choice

DeepCreative picks one "current GPU" and reads its group. With integrated graphics it can also take GPU readings from the CPU group (the integrated-graphics labels above). Code: the helper's GPU selection and label list. OpenDeepCool v1 reads the owner's only GPU (`docs/local/`). Choosing among several GPUs is outside v1 (PRD, What We're NOT Building).

## CPU power

- Since a kernel security fix, the RAPL energy counters are readable by root only. Reading them lets any local user mount the "Platypus" power side-channel attack (CVE-2020-8694; Intel advisory INTEL-SA-00389, which names `/sys/class/powercap/intel-rapl*/*/energy_uj`). System: on the owner's PC the counter is root-only (2026-10-22).
- So the owner's account can't read CPU power without an install-time change. Making the file readable by **every** user would reopen that attack for all local accounts. Options for Phase 6:
  - a rule that lets **only** the owner's account read it;
  - a small privileged helper that serves only the power value.

  Phase 6 decides, with the owner. The PRD's open question "is CPU power readable without root?" is answered: **no**.

## Fan and pump speed

- DeepCreative never sends fan or pump speeds to the LP360. It shows them on its own device card, from the board fan inputs the user picks as **CPU Fan I/O** and **Pump Fan I/O**, out of the board's fan list. Evidence: code: LP page settings and the helper's fan list; Phase 1 report.
- OpenDeepCool does the same:
  - the user picks one board fan input for each, from the board sensor driver's fan inputs (hwmon);
  - the settings app shows their RPM;
  - nothing reaches the display.
- The board sensor driver isn't loaded at boot on the owner's PC. Loading it at boot is part of the install (PRD, System Footprint).
- Which input is the CPU fan and which is the pump on the owner's board: `docs/local/` (inferred from a load test, 2026-10-22; not yet confirmed in the BIOS).
