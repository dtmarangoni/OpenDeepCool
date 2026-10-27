# Phase 1 review: Wine reference spike

**Review ID:** `phase-1-wine-reference-spike`
**Reviewed:** 2026-10-26, at commit `3ec34b7`
**Phase status in the PRD:** complete (2026-10-22)
**Inputs:** PRD (Phase 1, Technical Approach → Reverse-engineering environment, Technical Risks); [`docs/research/phase1-wine-spike.md`](../../research/phase1-wine-spike.md); the [Phase 3 plan](../plans/phase-3-functionality-protocol-spec.plan.md) (which reused the rig); `AGENTS.md` rules 6, 7 and 9
**Overview:** [project-overview.review.md](project-overview.review.md)

Phase 1 has no plan file; its report is the record. This review looks at what the phase left behind for later phases: the research rig, its isolation, and how its findings are used now.

## Verdict

The spike did what a spike should: it settled the riskiest question (can DeepCreative drive the real cooler under Wine, and can every byte be recorded?) quickly, with a clear PROVEN verdict and the network blocked. Its findings were later superseded by the Phase 3 spec, which is correct.

What remains are two research-hygiene points, both Low: the rig's isolation was narrower than it could be, and one agent instruction still points at this report for the mode list.

## What the phase produced

- A research rig, local only:
  - a dedicated Wine prefix;
  - `bwrap --unshare-net`, which leaves loopback only;
  - an `LD_PRELOAD` logger on the LP360's hidraw node;
  - Chrome DevTools and Node inspector access to the app, bridged out of the sandbox through `socat`.
- The first protocol facts, later confirmed and extended by the Phase 3 spec.
- The full list of DeepCreative's LP modes, including Creative Mode (later removed from OpenDeepCool, owner, 2026-10-25).

## Strengths

- **Network blocked during every run** (rule 9: never accept a firmware update). The PRD's risk of self-update or firmware download was mitigated by construction, not by watching prompts.
- **Logger scoped to the LP360's node only.** Other HID devices, such as the keyboard, were never logged. That's good data minimisation.
- **No installation.** DeepCreative ran from the extracted payload, so no Windows services or drivers were registered in the prefix.
- **Captures and vendor files kept out of git** (rule 6), with a guard to back it up (Phase 2).

## Analysis

### Isolation of the research rig

`bwrap --unshare-net` isolates the network only. The local launch script runs `bwrap --dev-bind / / --unshare-net`, which binds the whole host file system and every device node into the sandbox (checked 2026-10-26). So everything else about the host stays reachable:

- The prefix maps drive `Z:` to `/` (Wine's default; `dosdevices/z:` checked 2026-10-26), so DeepCreative's processes could read and write the owner's home directory.
- Every device node the owner's account can open was reachable, not only the LP360's.
- The DevTools and inspector bridges (`socat TCP-LISTEN:…,bind=127.0.0.1`, per `docs/local/`) expose code execution inside the app to any local process for as long as they listen.

DeepCreative is vendor software from a known vendor, and the PC has one user, so the realistic risk is low. But it's closed-source code run with the owner's full file access, plus a debugging port with no authentication. If the rig runs again (the PRD keeps it until the end of v1):

- bind only what's needed: the prefix read-write, the app read-only, and a private `/tmp`, with the home directory replaced by an empty `tmpfs`;
- keep the DevTools and inspector endpoints on Unix sockets with mode 0600, and connect to them with clients that speak Unix sockets, instead of a TCP bridge;
- stop the bridges at the end of each session; a check in the launch script can refuse to start while an old bridge still listens.

### How the report is used today

`AGENTS.md` "Verified facts" says: "The USB message format and the full list of display modes are in `docs/research/phase1-wine-spike.md`." Since Phase 3, the authority is `docs/spec/` (protocol, display, music), and the report's list still includes Creative Mode (modes 100 and 100 + n), which OpenDeepCool will never build. An agent following AGENTS.md literally could treat Creative as in scope. The spec should be the pointer, with the report kept as history.

### Residual risks carried forward

- The meaning of the start-up command `03 08` is still unverified; OpenDeepCool sends it exactly as DeepCreative does (protocol spec). That's the right call, and it's carried by later phases (see P5-05 for the resume case).
- What DeepCreative sends after a reconnect is unverified, because the rig's network namespace probably hid hotplug events (protocol spec). OpenDeepCool's own rule replaces it.

## Findings

| ID | Severity | Category | Title |
|---|---|---|---|
| P1-01 | Low | Security (research) | The rig isolates only the network; DeepCreative had the owner's file system, and the debug bridges listen on TCP |
| P1-02 | Low | Coherence | `AGENTS.md` still points at the Phase 1 report for the mode list, which includes Creative Mode |
| P1-03 | Note | Process | The rig and every DeepCreative file are deleted at the end of v1 (Phase 11 Task 8); the oracle generator must be rescued before that (see P3-01) |

### P1-01: The rig isolates only the network

- **Evidence:** Phase 1 report, Setup ("Isolation: `bwrap --unshare-net`: loopback only"); the local launch script (`--dev-bind / /`); the prefix's `Z:` mapping; `docs/local/` (the `socat` TCP bridges for DevTools and the inspector).
- **Why it matters:** closed-source code ran with read and write access to the owner's files, and the inspector gives any local process code execution in the app, which holds the cooler's device node.
- **Recommendation:** if the rig is used again, apply the narrower `bwrap` binds and Unix-socket endpoints described above, and check that no bridge is left listening after a session.

### P1-02: AGENTS.md points at the report for the mode list

- **Evidence:** `AGENTS.md`, Verified facts, first bullet; Phase 1 report, Display options (Creative Mode rows); PRD, What We're NOT Building (Creative Mode, owner, 2026-10-25).
- **Why it matters:** the report is history; the spec is the contract. A stale pointer invites building something the owner removed.
- **Recommendation:** point AGENTS.md at `docs/spec/protocol.md`, `display.md` and `music.md` for messages and modes. Optionally add a one-line note at the top of the report: "Superseded by `docs/spec/`; Creative Mode is not part of OpenDeepCool."

### P1-03: Deleting the rig also deletes the oracle tools

- **Evidence:** Phase 11 Task 8 deletes `research/`, including "the hooks that only work with DeepCreative in `research/hook/`". The oracle generators (`odc_rules.py`, `expected-frames.py`) live in the same folder.
- **Recommendation:** see P3-01: commit the spec-only generator early, not at the end.

## Questions for the owner

- None specific to Phase 1; P1-01 applies only if the rig runs again.

## Sources

- Phase 1 report; `docs/local/` (bridge commands; not reproduced here).
- `bwrap --help`, read 2026-10-26: `--unshare-net` "Create new network namespace"; binds (`--bind`, `--tmpfs`) are separate options.
- The local launch script and the research prefix's `dosdevices/`, read 2026-10-26 (git-ignored; not reproduced here).
