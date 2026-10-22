# AGENTS.md — OpenDeepCool

Instructions for any coding agent (Claude Code, Codex, Cursor, …) working in this repository. Keep this file under 200 lines and put details in the documents it points to.

## What this project is

OpenDeepCool replaces DeepCreative, DeepCool's Windows app, on Omarchy (Arch Linux). It is written in Rust and targets only the owner's DeepCool LP360 AIO cooler on this one PC.

- A background service drives the LP360's 14×14 dot-matrix display from boot.
- A GTK4 app changes the settings and follows the Omarchy theme.

It is a personal project in a public repository, MIT-licensed, and not affiliated with DeepCool.

## Start of every session

1. Read the PRD at `docs/prp/prds/opendeepcool-lp360.prd.md`. Its **Implementation Phases** table is the only record of progress.
2. Find the current phase: the first row whose status isn't `complete` and whose dependencies are all `complete`. Open its plan (Plan column) and its latest report, if there is one.
3. Run `git status` and `git log --oneline -15` to see where the work stopped.
4. Before changing anything, tell the owner in one or two lines which phase and step you're on.

## How to work

- Work only on the current phase, and only on the steps in its plan. Get the owner's OK before adding scope, switching phases or skipping a step.
- After a context compaction or a long pause, re-read the current plan step before continuing.
- If the plan or spec is wrong, incomplete, or contradicts what you see: stop, explain, and update the document with the owner's OK before writing code.
- When a phase changes state, update the PRD table (status, plan, report, PR). In Claude Code, use `/prp-core:prp-prd-update`.
- Write what you learn about the hardware or DeepCreative into the spec (`docs/spec/`, from Phase 3), not only into code or chat.
- When a rule or command changes, update this file in the same commit.

## Rules

1. **Never guess.** Don't invent facts, documentation, protocol bytes, sysfs paths, crate APIs or DeepCreative behaviour.
   - Check against this machine, the spec, primary sources (official docs through Context7 or Firecrawl) or the owner.
   - Mark anything you couldn't check as "unverified".
2. **If in doubt, ask now.** When something is ambiguous, stop and ask the owner right away. Never pick a default silently.
3. **The owner validates the hardware.** Anything that shows only on the LP360's display, or otherwise physically, must be checked by the owner. Give numbered steps, say what they should see, and wait for the answer before marking it done.
4. **One implementation, one commit.**
   - Build each display mode and each combination on its own (for example Load Mode → Combined CPU/GPU): implement it, run its tests, get the owner's check on the cooler, then make **exactly one commit**.
   - Never batch several modes, combinations or unrelated changes into one commit.
   - Use Conventional Commits style (`feat:`, `fix:`, `test:`, `docs:`, `chore:`, `refactor:`).
   - Never commit failing tests or a broken build.
5. **Tests with every step**, where they apply:
   - unit tests for pure logic;
   - integration tests with fakes (fake sysfs trees, fake HID transport);
   - e2e tests on the real device, which run with the owner.
6. **Never put DeepCool material in the repository.**
   - This covers the installer and its extracted files, their code (including decompiled code or bytecode), and raw USB captures of DeepCreative.
   - Keep all of it outside the repository; `.gitignore` excludes it.
   - Describe protocol facts in our own words in the spec.
7. **No undocumented system changes.** Ask before any `sudo` or system-level change. List every file, package, rule, module or service the app installs or creates in the README's "Installed on your system" section and in the PRD's System Footprint.
8. **Out of scope for v1:**
   - fan or pump speed control: never write to hwmon `pwm*` files;
   - hidden display settings (`screenBrightness`, `screenStatus`, `noLoadScreen`, `rgbOptical`, `gyroStatus`) and the device reports DeepCreative doesn't use: don't research or send them;
   - ARGB lighting: OpenRGB handles it;
   - other devices or hardware.
9. **Device safety.**
   - Send the LP360 only messages documented in the spec.
   - Run DeepCreative for research only in its dedicated Wine prefix, with the network blocked.
   - Never accept a firmware update.
10. **No personal or machine data in git.** The repository is public.
    - Committed files, commit messages, PR texts and commit metadata must not contain:
      - the owner's name, email, usernames, local paths or serial numbers;
      - the make, model, BIOS or firmware of the owner's PC parts, or its OS and kernel versions;
      - how the owner's PC logs in or encrypts its disk.
    - Put such facts in `docs/local/` (git-ignored) and refer to them only as "see `docs/local/`".
    - Commit with the repository's configured identity (GitHub no-reply email). Never change it.

## Verified facts (details in the PRD Research Summary)

- **LP360**: USB HID device `3633:000c` ("DeepCool LP-Series") with a 14×14 display. The USB message format and the full list of display modes are in `docs/research/phase1-wine-spike.md`.
- **The owner's PC**: board, sensor chip and driver, fan inputs, CPU and GPU are in `docs/local/machine.md`, which is local only. Read it on this machine, but never copy it into committed files.
- **Device lookup**: find hwmon and DRM devices by name or driver, never by index, because indexes change between boots.
- **Omarchy 4 theming**: theme colours live in `~/.local/state/omarchy/current/theme/colors.toml`, and app templates in `~/.config/omarchy/themed/*.tpl`.

## Repository layout

| Path | Contents |
|---|---|
| `docs/prp/prds/` | PRD: scope, decisions, phase status |
| `docs/prp/plans/` | One plan per phase (from Phase 3), with its HTML companion |
| `docs/prp/reports/` | Implementation reports |
| `docs/spec/` | Functionality and protocol spec (from Phase 3) |
| `docs/research/` | Spike and research reports (e.g. `phase1-wine-spike.md`) |
| `docs/hardware/` | Our own notes on the LP360, with manual page references |
| `docs/local/` | Facts about the owner's PC: **local only, git-ignored** |
| `docs/reference/vendor/` | Vendor manuals: **local only, git-ignored**; see `docs/reference/README.md` |
| `research/` | Research workspace (Wine prefix, extracted DeepCreative, captures, screenshots, research tools): **local only, git-ignored** |
| Rust workspace, packaging | Created in Phases 5–12; add them here when they exist |

The PRP tools keep their files under `~/.prp/`, through a symlink to `docs/prp/`, so plans and reports land in the repository.

## Commands

- **Toolchain**: `mise install` sets up the Rust version pinned in `mise.toml`, with clippy and rustfmt. Research-only tools, such as Node for `research/hook/*.mjs`, go in the git-ignored `mise.local.toml`.
- **Build, test, lint, format**: none yet; the Rust workspace starts in Phase 5. Once it exists, write the exact commands here and run them before every commit. Planned: `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
