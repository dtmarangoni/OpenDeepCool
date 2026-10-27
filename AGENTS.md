# AGENTS.md — OpenDeepCool

Instructions for any coding agent (Claude Code, Codex, Cursor, …) working in this repository. Keep this file under 200 lines and put details in the documents it points to.

## What this project is

OpenDeepCool replaces DeepCreative, DeepCool's Windows app, on Omarchy (Arch Linux). It is written in Rust and supports only the DeepCool LP360 AIO cooler, on any PC running Arch Linux, with Omarchy first (owner, 2026-10-23). Real-hardware tests run on the owner's PC.

- A system service drives the LP360's 14×14 dot-matrix display from boot, the same for every user.
- A GTK4 app changes the settings. It follows the current Omarchy theme live, and uses its own default theme on other desktops.

It covers only the DeepCool cooler: its display, and its fans and pump (owner, 2026-10-23). The CPU, GPU and RAM are read only for the values the display shows.

It is a personal project in a public repository, MIT-licensed, and not affiliated with DeepCool.

## Start of every session

1. Read the PRD at `docs/prp/prds/opendeepcool-lp360.prd.md`. Its **Implementation Phases** table is the only record of progress.
2. Find the current phase: the first row whose status isn't `complete` and whose dependencies are all `complete`. Open its plan (Plan column), its solution review in `docs/prp/solution-reviews/`, and its latest report, if they exist.
3. Run `git status` and `git log --oneline -15` to see where the work stopped.
4. Before changing anything, tell the owner in one or two lines which phase and step you're on.

## How to work

- Work only on the current phase, and only on the steps in its plan. Get the owner's OK before adding scope, switching phases or skipping a step.
- **Solution review, between plan and implementation:** once a phase's plan exists, and before implementing it, review it for architecture, best practices, security, bottlenecks and coherence with the PRD and other plans.
  - Write the review to `docs/prp/solution-reviews/phase-N-<slug>.review.md`, and update `project-overview.review.md` there when the architecture changes.
  - A review only reports; it never edits the PRD, a plan, the spec or code.
  - Cross its findings with the PRD and the plan together with the owner, and put the agreed changes into those documents before writing code ([how](docs/prp/solution-reviews/README.md)).
  - The owner's decisions, recorded as Outcome lines in the reviews, win over the PRD, the plans and the spec: where one disagrees with an outcome, correct it to match. A later outcome supersedes an earlier one (owner, 2026-10-27).
- After a context compaction or a long pause, re-read the current plan step before continuing.
- Before a phase's first task, compare its plan with what the earlier phases actually built (names, types, files, commands). Where they differ, update the plan with the owner's OK before writing code (owner, 2026-10-27).
- From Phase 5 on, a change to a plan regenerates its HTML companion in the same commit (owner, 2026-10-27). During the solution-review cross-check, the companions are regenerated once, when it ends.
- If the plan or spec is wrong, incomplete, or contradicts what you see: stop, explain, and update the document with the owner's OK before writing code.
- When a phase changes state, update the PRD table (status, plan, report, PR). In Claude Code, use `/prp-core:prp-prd-update`.
- When a phase is done and signed off, push its branch and wait until its CI run passes, then fast-forward `main` to it and push; no pull requests (owner, 2026-10-23 and 2026-10-27). Its PRD row then becomes `complete`, with `-` in the PR column. At sign-off, CI's pinned actions are checked for newer releases and their hashes updated (owner, 2026-10-27).
- Write what you learn about the hardware or DeepCreative into the spec (`docs/spec/`, from Phase 3), not only into code or chat.
- When a rule or command changes, update this file in the same commit.

## Rules

1. **Never guess.** Don't invent facts, documentation, protocol bytes, sysfs paths, crate APIs or DeepCreative behaviour.
   - Check against this machine, the spec, primary sources (official docs through Context7 or Firecrawl) or the owner.
   - Mark anything you couldn't check as "unverified".
2. **If in doubt, ask now.** When something is ambiguous, stop and ask the owner right away. Never pick a default silently.
3. **Testing needs no person** (owner, 2026-10-24).
   - Phases 1–3 needed the owner's eyes to learn how the device works, and the spec records it. From Phase 5 on, every test (unit, integration, differential and end to end on the real LP360) runs unattended. Nobody watches the cooler or acts during a test.
   - The proof that bytes become the right picture is the byte-for-byte match with DeepCreative's captured messages, which the owner checked on the cooler in Phase 3. Where OpenDeepCool draws differently on purpose (owner decisions, such as no caps), the expected frames follow the spec, drawn with the same glyphs and encoding.
   - Ask the owner for decisions and for system changes (rule 7), never to look at the cooler. If a behaviour can only be judged by looking at the cooler, the spec has a gap: stop and raise it (rule 2).
   - The one exception is optional: `odc-e2e --attended` shows one case of each kind and of each edge case for the owner to watch and confirm, when the owner wants to (owner, 2026-10-24). It never replaces or gates the unattended tests.
4. **One implementation, one commit.**
   - Build each display mode and each combination on its own (for example Load Mode → Combined CPU/GPU): implement it, run all its tests, including the unattended end-to-end run on the cooler, then make **exactly one commit**.
   - Never batch several modes, combinations or unrelated changes into one commit.
   - Use Conventional Commits style (`feat:`, `fix:`, `test:`, `docs:`, `chore:`, `refactor:`).
   - Never commit failing tests or a broken build.
5. **Tests with every step, every combination automated** (details: PRD, Testing Strategy):
   - cover the whole test matrix (every mode × rotation × unit, scrolling included) and every scenario in the in-process layers (unit and integration), never samples; give every metric its own distinct, non-zero value in each case;
   - unit tests compare each case with the expected frames in `docs/spec/frames/`, plus property tests (rotation, scrolling, value rules);
   - differential tests replay DeepCreative's local captures and expect 0 differing pixels;
   - integration tests use fakes (fake sysfs trees, fake HID transport, fake clock and audio);
   - tests also cover what each entry point must refuse: callers and arguments the service must reject, forged or malformed files, impossible readings and samples. The entry-points table in `docs/prp/solution-reviews/project-overview.review.md` ("Entry points after the cross-check") is the reference (owner, 2026-10-27);
   - e2e: an unattended run on the real cooler takes one example of each mode and of each test type (owner, 2026-10-26: the whole cross would take days in real time, and runs in-process instead), and compares every written report with the expected frames (rule 3). It probes the PC it runs on: each device present and usable is tested for real, an absent one gets the same checks against fakes, and one present but unusable fails with its setup fix. `odc-e2e --require <list>` names what must run real on a given PC; every run reports which mode each device used (owner, 2026-10-24). Test options are program flags given at start, like the service's `--dev`, never environment variables (owner, 2026-10-24).
6. **Never put DeepCool material in the repository.**
   - This covers the installer and its extracted files, their code (including decompiled code or bytecode), and raw USB captures of DeepCreative.
   - Keep all of it outside the repository; `.gitignore` excludes it.
   - Describe protocol facts in our own words in the spec.
7. **No undocumented system changes.** Ask before any `sudo` or system-level change. List every file, package, rule, module or service the app installs or creates in the README's "Installed on your system" section and in the PRD's System Footprint.
   - Uninstalling removes everything the app installed or created (owner, 2026-10-24): files, users, groups, udev rules, enabled units, permissions and saved data. An automated install-and-uninstall test proves the system is left as it was.
8. **Out of scope for v1:**
   - fan or pump speed control: never write to hwmon `pwm*` files;
   - hidden display settings (`screenBrightness`, `screenStatus`, `noLoadScreen`, `rgbOptical`, `gyroStatus`) and the device reports DeepCreative doesn't use: don't research or send them;
   - Creative Mode (DeepCreative's image editor, modes 100 and 100 + n), in any version (owner, 2026-10-25);
   - ARGB lighting: OpenRGB handles it;
   - other devices (sensors, though, must work on any PC: rule 12);
   - monitoring or controlling any hardware other than the cooler (owner, 2026-10-23): read the CPU, GPU and RAM only for the display's values, never write a CPU, GPU, fan or power setting, and leave that to dedicated tools, as OpenRGB handles lighting;
   - following other desktops' themes: v1 uses its default theme there.
9. **Device safety.**
   - Send the LP360 only messages documented in the spec.
   - Run DeepCreative for research only in its dedicated Wine prefix, with the network blocked.
   - Never accept a firmware update.
10. **No personal or machine data in git or other public text.** The repository is public.
    - Committed files, commit messages, PR texts and commit metadata must not contain:
      - the owner's name, email, usernames, local paths or serial numbers;
      - the make, model, BIOS or firmware of the owner's PC parts, or its OS and kernel versions;
      - how the owner's PC logs in or encrypts its disk.
    - The same holds for all other outward-facing text (owner, 2026-10-27): tag messages, release notes, the AUR repository and its comments, issues and published pages.
    - Queries to outside services, such as web searches and MCP tools, never carry the owner's name, email, usernames, serial numbers or local paths, nor how the PC logs in or encrypts its disk. They may name a part or driver when a task needs it, such as the fan chip to find its driver (owner, 2026-10-27).
    - Put such facts in `docs/local/` (git-ignored) and refer to them only as "see `docs/local/`".
    - Commit with the repository's configured identity (GitHub no-reply email). Never change it.
    - **Exception** (owner, 2026-10-22): the kernel driver and module names and sensor labels that the sensor rules need may appear in code, tests, packaging and the spec. Part models, serials and all other machine facts still may not; test fixtures use made-up chip names.
    - **Drivers tied to the owner's PC** (owner, 2026-10-27): nothing committed or outward-facing says which drivers, vendors or device kinds the owner's PC has, because they reveal its parts' makes. Reports and docs name its devices by role (for example "CPU temperature: real"); which drivers it uses stays in `docs/local/`.
11. **Every metric shows itself.** A CPU reading is shown as the CPU, a GPU reading as the GPU, and so on, everywhere: renderers, settings, the app, tests. Never swap or mix metrics, even where DeepCreative does (owner, 2026-10-22). The spec marks each such DeepCreative quirk and what OpenDeepCool does instead.
12. **Never rely on the owner's PC** (owner, 2026-10-23). The code must work on any PC with an LP360.
    - Find devices, drivers and sensors at runtime, from rules taken from primary documentation.
    - Where the hardware can't tell (which fan input is the pump), the user chooses.
    - Facts about the owner's PC serve only for testing on it.

## Verified facts (details in the PRD Research Summary)

- **LP360**: USB HID device `3633:000c` ("DeepCool LP-Series") with a 14×14 display. Its USB messages are in `docs/spec/protocol.md`, and the display modes in `docs/spec/display.md` (1–12) and `docs/spec/music.md` (13–23). The Phase 1 report (`docs/research/phase1-wine-spike.md`) is history: the spec supersedes it, and its mode list is DeepCreative's, Creative Mode included.
- **The owner's PC**: board, sensor chip and driver, fan inputs, CPU and GPU are in `docs/local/machine.md`, which is local only. Read it on this machine, but never copy it into committed files or other outward-facing text (rule 10), and never make the code depend on it (rule 12).
- **Device lookup**: find hwmon and DRM devices by name or driver, never by index, because indexes change between boots.
- **Omarchy 4 theming**: theme colours live in `~/.local/state/omarchy/current/theme/colors.toml`. App templates live in each user's `~/.config/omarchy/themed/*.tpl` and in Omarchy's own `$OMARCHY_PATH/default/themed/` (its `omarchy-theme-set-templates` script, read 2026-10-23).

## Repository layout

| Path | Contents |
|---|---|
| `docs/prp/prds/` | PRD: scope, decisions, phase status |
| `docs/prp/plans/` | One plan per phase (from Phase 3), with its HTML companion |
| `docs/prp/reports/` | Implementation reports |
| `docs/prp/solution-reviews/` | Solution reviews: the whole solution and each phase's plan, reviewed after planning and before implementation; their findings are crossed with the owner before any code |
| `docs/spec/` | Functionality and protocol spec (from Phase 3) |
| `docs/design/` | The settings app's design, its mockups and the mockup generator (`mockups-src/gen.py`); a UI change updates them before the code |
| `docs/research/` | Spike and research reports (e.g. `phase1-wine-spike.md`) |
| `docs/hardware/` | Our own notes on the LP360, with manual page references |
| `docs/local/` | Facts about the owner's PC: **local only, git-ignored** |
| `docs/reference/vendor/` | Vendor manuals: **local only, git-ignored**; see `docs/reference/README.md` |
| `research/` | Research workspace (Wine prefix, extracted DeepCreative, captures, screenshots, research tools): **local only, git-ignored** |
| Rust workspace, packaging | Created in Phases 5–11; add them here when they exist |

The PRP tools keep their files under `~/.prp/`, through a symlink to `docs/prp/`, so plans and reports land in the repository.

## Commands

- **Toolchain**: `mise install` sets up the Rust version pinned in `mise.toml`, with clippy and rustfmt. Research-only tools, such as Node for `research/hook/*.mjs`, go in the git-ignored `mise.local.toml`.
- **Build, test, lint, format**: none yet; the Rust workspace starts in Phase 5. Once it exists, write the exact commands here and run them before every commit. Planned: `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
