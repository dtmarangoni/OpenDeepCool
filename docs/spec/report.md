# The bug report: `opendeepcool --report`

A command that prints a plain-text bug report for the user to read, change and post in a GitHub issue (owner, 2026-11-17, after the [Arch portability review](../prp/solution-reviews/arch-portability.review.md), AP-09: "option 2 now"). Arch has no automatic crash reporter, a core dump holds the user's memory, and debugging is the developer's job, so the user is asked for something small and safe: this report.

## What it is, and isn't

- **Plain text on standard output.** The user reads it, removes what they don't want to share, and pastes it into an issue at <https://github.com/dtmarangoni/OpenDeepCool/issues>.
- **Nothing is sent.** It opens no network connection, writes no file and changes nothing. It needs no display, no root and no running service: it works when the service has crashed, which is when it is wanted.
- **Where:** the settings app's program, `opendeepcool --report`, handled before the toolkit starts. The flag is refused together with any other (exit code 2, a usage line). Once it starts it exits 0: what it couldn't read is said in its place.
- **Programs it runs:** `pacman`, `systemctl`, `journalctl` and `coredumpctl`, found on `PATH`, each with fixed arguments and no shell. A program that isn't there, or that fails, leaves one line in its section: `not available: <the reason>`.

## Contents, in this order

| # | Section | What it holds | From |
|---|---|---|---|
| 1 | Header | The title, "Read this before you post it. Nothing was sent. Remove anything you don't want to share.", and the issue tracker's address | fixed |
| 2 | Version | The program's version, and the line of `pacman -Q opendeepcool` | the build; pacman |
| 3 | System | The distribution's `NAME`, `ID` and `ID_LIKE`, the kernel release, and whether systemd is running | `/etc/os-release`; `/proc/sys/kernel/osrelease`; `/run/systemd/system` |
| 4 | Units | For the four system units (`opendeepcool.service`, `opendeepcool-gpu-rank.service`, `opendeepcool-sensors-detect.service`, `opendeepcool-sensors-detect-modules.service`) and the user's `opendeepcool-audio.service`: `LoadState`, `UnitFileState`, `ActiveState`, `SubState`, `Result`, `NRestarts`, `ExecMainStatus` | `systemctl show`, and `systemctl --user show` |
| 5 | The service's journal | The last 100 lines since the boot of the four system units | `journalctl -b -n 100 -o short-iso --no-hostname --no-pager` |
| 6 | The sound helper's journal | The last 40 lines since the boot | the same with `--user` |
| 7 | Crashes | The `coredumpctl list` lines of OpenDeepCool's programs (`opendeepcoold`, `opendeepcool`, `opendeepcool-audio`, `odc-gpu-rank`, `odc-sensors-detect`, `opendeepcool-boot`) and nothing of any other program, with the reminder not to attach a core dump | `coredumpctl list` |

The service's journal already says which source each reading uses, whether the cooler is connected and what the detection did, so the report adds no D-Bus call.

## The journal needs a group

A regular user reads only their own journal; the system journal is open to `systemd-journal`, `adm` and `wheel` (ArchWiki, systemd/Journal, read 2026-11-17). Where the system journal can't be read, section 5 says so, with what to do: add the account to `systemd-journal`, or run `sudo journalctl -u opendeepcool.service -b -n 100 --no-hostname` and paste its output.

## Privacy

- **Scrubbed from everything the report prints**, whatever program printed it: the user's home folder (`~`), the host name (`<host>`), the machine ID and the boot ID (`<machine-id>`, `<boot-id>`), and the login name as a whole word (`<user>`). The host name is also left out of the journal's lines by `--no-hostname`.
- **Never collected:** environment variables, any file but `/etc/os-release` and the kernel release, network addresses, the cooler's serial number, core dumps.
- **Kept on purpose:** the distribution and kernel release, PCI addresses and the driver and chip names that the service's lines carry: they are why a reading is found or not.
- The user reads the report before posting it; the header says so.

## Tests

- **Unit tests with a fake `Runner`** (the report's seam for running programs and reading files): the sections and their order; each scrub, the login name only as a whole word (a user `al` leaves `alsa` alone); a missing program, a failing program and an unreadable system journal each leave their one line and the other sections intact; another program's crash is never listed; the host name in a fake's output doesn't reach the report.
- **The real program** with an empty `PATH`: the report prints every section's title with `not available`, and exits 0; with another flag it exits 2.
- **On the development PC**, a real run is read once, and its text is checked against the machine's private values (`docs/local/`), which none may appear in.
