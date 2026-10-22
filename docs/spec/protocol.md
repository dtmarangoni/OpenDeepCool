# LP360 USB protocol

How DeepCreative 1.2.14 talks to the LP360, and what the display does when nothing is sent. Evidence labels are defined in [README.md](README.md).

## Device

| Item | Value | Evidence |
|---|---|---|
| USB ID | `3633:000c`, product string "LP-Series" (shown as "DeepCool LP-Series") | capture: lsusb and sysfs, 2026-10-21 |
| Interface | One HID interface, vendor-defined usage page `0xFF00` | code: sysfs `report_descriptor`, 2026-10-22 |
| Declared reports | 2, 3 and 4: output and feature, 63 bytes each. 5: input and feature, 63 one-bit values. 16: output and input, 63 bytes | code: sysfs `report_descriptor`, 2026-10-22 |
| Used by DeepCreative | **Output report 16 only.** It never reads from the device and never uses a feature report | capture: all sessions (17,228 messages, every one a write) |

Find the device by its USB ID, never by its hidraw number: the number changes between boots (see `docs/local/`).

The other reports (2–5, and input report 16) are not used by DeepCreative's LP page. They are after v1 in the PRD, and OpenDeepCool never sends them.

## Message layout

Every message is one 64-byte HID output report: report ID 16 followed by 63 bytes.

| Byte | Value | Meaning |
|---|---|---|
| 0 | `0x10` | Report ID 16 |
| 1–3 | `0x68 0x01 0x05` | Fixed header |
| 4 | N | Payload length |
| 5 … 4+N | payload | See [Messages](#messages) |
| 5+N | checksum | Sum of bytes 1 … 4+N, modulo 256 |
| 6+N | `0x16` | Terminator |
| 7+N … 63 | `0x00` | Padding |

Evidence: capture: all sessions. Every one of the 17,228 captured messages has this layout, a correct checksum, the terminator and zero padding.

## Messages

DeepCreative sends exactly two kinds of message (capture: all sessions).

### Start-up command

- **Payload:** `03 08` (N = 2). The whole report starts `10 68 01 05 02 03 08 7b 16`, followed by zeros.
- **When:** once, as the first message of a session, before any frame. Capture: both sessions (2026-10-22).
- **Meaning:** unverified. DeepCreative's strings don't show how it's built.
- **OpenDeepCool:** sends it once at start-up, exactly as DeepCreative does.

### Frame

- **Payload:** `01` followed by 28 pixel bytes (N = 29).
- **Contents:** one complete 14×14 picture, one bit per pixel (on or off).

Pixels are numbered with row 1 at the top and column 1 at the left, as the cooler shows them (owner: 2026-10-22, digits legible the right way up). Rows pair up as 1–2, 3–4, …, 13–14, and each pair has one bit in every pixel byte:

| Row pair | 1–2 | 3–4 | 5–6 | 7–8 | 9–10 | 11–12 | 13–14 |
|---|---|---|---|---|---|---|---|
| Bit | `0x10` | `0x20` | `0x40` | `0x80` | `0x01` | `0x02` | `0x04` |

- **Pixel bytes 1–14** (payload bytes 2–15) hold the **odd** row of each pair (rows 1, 3, …, 13) for **columns 1 → 14**.
- **Pixel bytes 15–28** (payload bytes 16–29) hold the **even** row of each pair (rows 2, 4, …, 14) for **columns 14 → 1**, in reverse order.
- Bit `0x08` is never set (capture: 17,226 frames).

So pixel (row r, column c), with pair k = (r + 1) div 2, is:
- for odd r: bit k of pixel byte c;
- for even r: bit k of pixel byte 29 − c.

Here "bit k" means the k-th entry of the table above.

Evidence: capture: all sessions, with the frames decoded this way showing legible digits; owner: 2026-10-22, the cooler showed the decoded value `3%`. The layout matches the community project Nortank12/deepcool-digital-linux.

## Timing

- **Frames are sent continuously in every mode,** even when the picture doesn't change. Capture: all sessions.
- **Interval:** median 103.9 ms between frames; 5th percentile 88–96 ms, 95th percentile 104.1 ms. That's about 9.6 frames per second. Capture: both sessions.
- **Drawn by the PC:** rotation, scrolling and animation are all computed by DeepCreative; the cooler only shows the frames it receives. Capture: Phase 1 walkthroughs ([Phase 1 report](../research/phase1-wine-spike.md)).

## Session end and the display without frames

- **At exit DeepCreative sends nothing special.** Its last message is an ordinary frame. Capture: clean quit, 2026-10-22 04:28:41.
- **When frames stop, the cooler keeps the last picture for about 10 seconds, then goes blank.** Owner: 2026-10-22. The 10 s is the owner's estimate, not a measurement.
- A sender that wants the picture to stay must keep sending frames well within that time.

## Open

- What `03 08` does, and whether the display works without it: unverified. OpenDeepCool sends it as DeepCreative does, so this doesn't block Phase 5.
