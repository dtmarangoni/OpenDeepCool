# LP360 USB protocol

How DeepCreative 1.2.14 talks to the LP360, and what the display does when nothing is sent. Evidence labels are defined in [README.md](README.md).

## Device

| Item | Value | Evidence |
|---|---|---|
| USB ID | `3633:000c`, product string "LP-Series" (shown as "DeepCool LP-Series") | capture: lsusb and sysfs, 2026-10-21 |
| Interface | One HID interface, vendor-defined usage page `0xFF00` | code: sysfs `report_descriptor`, 2026-10-22 |
| Declared reports | 2, 3 and 4: output and feature, 63 bytes each. 5: input and feature, 63 one-bit values. 16: output and input, 63 bytes | code: sysfs `report_descriptor`, 2026-10-22 |
| Used by DeepCreative | **Output report 16 only.** It never reads from the device and never uses a feature report | capture: every capture of Phases 1 and 3 (8 app starts, 106,075 messages, every one a write) |

Find the device by its USB ID, never by its hidraw number: the number changes between boots (see `docs/local/`).

The ID and its product string name the LP series, not the LP360 alone: the LP360's manual also covers the LP240, LP240 WH and LP360 WH ([LP360 notes](../hardware/deepcool-lp360.md)). Whether those models share the ID and the 14×14 display is unverified. They're out of scope (PRD), and OpenDeepCool drives any device with this ID as an LP360 (owner, 2026-10-27; solution review P3-05).

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

Evidence: capture: all sessions. Every one of the 106,075 captured messages (8 app starts, 2026-10-22) has this layout, a correct checksum, the terminator and zero padding.

## Messages

DeepCreative sends exactly two kinds of message (capture: all sessions).

### Start-up command

- **Payload:** `03 08` (N = 2). The whole report starts `10 68 01 05 02 03 08 7b 16`, followed by zeros.
- **When:** once per app start, as the first message, before any frame. Capture: all 8 app starts, 2026-10-22. One logging process shows it twice, because the research rig restarted DeepCreative inside a Wine process that stayed alive; each `03 08` was the new start's first message.
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
- **Interval: one frame per 100 ms tick,** 10 frames per second. Over every continuous run of the capture log, the mean interval is 99.98–100.06 ms (14 runs, 176,083 intervals). Single intervals are 96 or 104 ms (8 ms steps, probably the timer's granularity under Wine, unverified), which is why their median is 104 ms. Capture: whole capture log, re-analysed 2026-10-24.
  - Corrected 2026-10-24: this section first gave the median, 103.9 ms, as the interval.
- **Drawn by the PC:** rotation, scrolling and animation are all computed by DeepCreative; the cooler only shows the frames it receives. Capture: Phase 1 walkthroughs ([Phase 1 report](../research/phase1-wine-spike.md)).
- **Write time:** one 64-byte write took 2.9 ms at the median and 7.9 ms at most, over 41 writes. Measured: OpenDeepCool on the cooler, 2026-10-28 (Phase 5, `odc-e2e`). A write to a stalled device can block for up to 5 s (kernel source: `usbhid_output_report` sends with `USB_CTRL_SET_TIMEOUT`, 5000 ms; read 2026-10-26), so the service writes from a thread of its own (PRD, Service model).

## Session end and the display without frames

- **At exit DeepCreative sends nothing special.** Its last message is an ordinary frame. Capture: clean quit, 2026-10-22 04:28:41.
- **When frames stop, the cooler keeps the last picture for about 10 seconds, then goes blank.** Owner: 2026-10-22. The 10 s is the owner's estimate, not a measurement.
- A sender that wants the picture to stay must keep sending frames well within that time.

## Disconnect and reconnect

Tested on 2026-10-24: DeepCreative ran under Wine, sending frames, while the cooler was disconnected and reconnected in software three times, off for 2 s, 15 s and 30 s. The kernel's USB authorization switch (docs.kernel.org, "Authorizing (or not) your USB devices") removes the device and adds it again, like pulling and replugging the cable, without sending it anything. Capture: reconnect test, 2026-10-24: every USB write, the open and close of the device node, and DeepCreative's own log.

- **When the cooler disappears,** DeepCreative's next frame fails with "no such device". It tries to send a frame 3 times in about 1.8 s, each time on the handle of the vanished device, then drops the device and stops sending. It sends no `03 08` during these tries.
- **The cooler comes back under another hidraw number**, so a reconnect must find it again by its USB ID ([Device](#device)).
- **DeepCreative never sent anything again** in any of the three cycles: it didn't reopen the cooler. This is probably the research rig, not DeepCreative: Wine runs in its own network namespace to block the internet, and device hotplug events reach programs through a netlink socket, so Wine likely never learned that the cooler came back. Unverified.
- **So what DeepCreative sends after a reconnect is unverified,** in particular whether it sends `03 08` again.

**OpenDeepCool** (owner, 2026-10-24):
- it keeps looking for the cooler for as long as it runs, instead of giving up after a few tries;
- after every reconnect, it sends `03 08` before the first frame, exactly as at start-up. A reconnect is a new start of the conversation with the cooler, and `03 08` is the only thing DeepCreative was seen sending first.
- Checked on the cooler, unattended, 2026-10-28 (Phase 5, `odc-e2e`): switched off through its USB `authorized` switch, the cooler made the next write fail within one 100 ms tick, and its USB device stayed listed with its IDs. Switched on again, it was back and writable within 0.2 s, with its udev access applied again, and the first message on the new connection was `03 08`.

## Standby

Not tested with DeepCreative. What a USB device goes through while the PC sleeps (suspend or hibernate) depends on the board: it may keep power, lose it, or be removed and added again on wake-up (**unverified** for the LP360).

- Removed and added again: that's a reconnect ([above](#disconnect-and-reconnect)).
- If its device node survives, no write fails, so nothing shows that the cooler may have lost power.

**OpenDeepCool** (owner, 2026-10-26: when the PC comes back from standby, OpenDeepCool must be back):
- after every wake-up it starts a new conversation with the cooler, as after a reconnect: it closes the device node, finds the cooler again by its USB ID, and sends `03 08` before the next frame. Sending `03 08` to a cooler that kept power is what every DeepCreative start does ([Start-up command](#start-up-command)).
- It learns of the wake-up from logind's `PrepareForSleep` signal with the argument false, sent right after suspend or hibernation (org.freedesktop.login1(5)). A gap in time can't show it: the monotonic clock doesn't count the time the PC sleeps (clock_gettime(2)).
- The monitor turning off or the screen locking isn't standby: the PC keeps running and the cooler keeps showing live values.

## Open

- What `03 08` does, and whether the display works without it: unverified. OpenDeepCool sends it as DeepCreative does, so this doesn't block Phase 5.
- Whether DeepCreative sends `03 08` again after a reconnect: unverified ([Disconnect and reconnect](#disconnect-and-reconnect)). OpenDeepCool does.
- What the LP360 goes through while the PC sleeps: unverified ([Standby](#standby)). OpenDeepCool starts a new conversation after every wake-up either way.
