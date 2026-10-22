# DeepCool LP360 — notes for OpenDeepCool

This file holds facts this project needs, summarised in our own words. The full manual is kept locally in `docs/reference/vendor/` (see [../reference/README.md](../reference/README.md)). Page numbers are the printed page numbers in the LP240/LP360 manual (PDF metadata date 2024-08-15).

**Sources**

- The manual.
- Readings taken on this PC on 2026-10-21/22, marked *observed*.
- Community projects, marked *community, unverified*.

## Identity

| Item | Value | Source |
|---|---|---|
| Product | LP360 (the manual also covers LP240, LP240 WH, LP360 WH) | Manual cover |
| Display | Dot-matrix display on the pump ("Matrix Display" in DeepCool's product page title) | DeepCool product page URL/title |
| USB | HID device `3633:000c`, product string "DeepCool LP-Series" | lsusb and sysfs, *observed* |
| HID reports | Vendor-defined (usage page `0xFF00`). Declares reports 2, 3 and 4 (output and feature, 63 bytes each), 5 (input and feature, 63 one-bit values) and 16 (output and input, 63 bytes). DeepCreative 1.2.14 used only output report 16: the Phase 1 log holds writes only. The rest are kept for after v1 and never sent | sysfs `report_descriptor` of the owner's unit (firmware revision in `docs/local/`), *observed* 2026-10-22 |
| Serial number | Reported over USB and used by DeepCreative as the device ID (value omitted here) | DeepCreative log under Wine, *observed* 2026-10-22 |

## Supported sockets (manual contents page)

- Intel LGA1200/1151/1150/1155 (p. 1)
- Intel LGA1851/1700 (p. 5)
- AMD AM5/AM4 (p. 9)

Optional front radiator mounting is on pp. 13–14, and chassis/radiator mounting on p. 15.

## Connections (pp. 16–17)

| Cable | Goes to |
|---|---|
| Radiator fans (LP360: 3, LP240: 2), chained to one 4-pin plug | A 4-pin CPU fan header ("4-PIN / CPU_FAN / CPU_OPT") |
| Pump power | "AIO_PUMP / W_PUMP" header, or CPU_FAN/CPU_OPT |
| Lighting: 3-pin plug labelled 5V / D / GND | The motherboard's 5 V addressable-RGB header. The manual shows the pinout but doesn't name the header type; 5V/D/GND is the standard 3-pin ARGB layout |
| Display ("Connection with Digital") | The motherboard's internal **9-pin USB 2.0 header** |

Which header these go to on the owner's own board is recorded in `docs/local/` (git-ignored).

## Software (pp. 18–19)

- After installing the cooler, install DeepCool's software to enable the product's functions.
- Operating system: **Windows 10/Windows 11 only**.
- The manual names `www.cryoex.com` as the download site. DeepCool's own download page currently offers **DeepCreative** (1.2.14, 2026-09-28).

## What the manual doesn't say

- The display's resolution, colour or brightness capabilities.
- The USB protocol, or what the display shows without software.
- Pump or fan speed specifications.
- Any Linux support.

## Known from elsewhere (to verify in the protocol spec)

- **Display**: 14×14 pixels. Each frame is sent as one 64-byte HID report: report ID 16, header `104 1 5 29 1`, 28 pixel bytes, a checksum, and terminator 22 (*community, unverified*; source: Nortank12/deepcool-digital-linux).
- **DeepCreative 1.2.14 under Wine** (*observed* 2026-10-22):
  - It detects this LP360 (log: `HID usb device[LP-Series] [sn: …] ok`).
  - Its sensor helper and fan-control helper don't run under Wine, so it shows 0 for all readings.
