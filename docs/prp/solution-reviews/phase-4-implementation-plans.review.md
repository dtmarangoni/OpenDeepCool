# Phase 4 review: implementation plans

**Review ID:** `phase-4-implementation-plans`
**Reviewed:** 2026-10-26, at commit `3ec34b7`
**Phase status in the PRD:** complete (2026-10-25)
**Inputs:** the [Phase 4 report](../reports/phase-4-implementation-plans.report.md); the plans of Phases 5–11; the PRD (Implementation Phases, Parallelism Notes, Testing Strategy, Success Metrics)
**Overview:** [project-overview.review.md](project-overview.review.md)

Phase 4's output is the set of plans for Phases 5–11. Each plan has its own review; this one looks at the set as a whole: how the plans fit together, what they share, and what none of them owns.

## Verdict

The plans are unusually complete:

- every task names its files, tests and validation command;
- every plan maps its tests to the four layers of the Testing Strategy and to its share of the test matrix;
- the spikes settled the architectural hinges with evidence, before any code: the rootless container, AT-SPI in headless weston, `pw-record` with polkit from a user unit.

The weaknesses sit between plans:

- the PRD's dependency and parallelism columns don't match the plans' real dependencies;
- the cost of the real-hardware runs is unbudgeted;
- the D-Bus contract and the CI environment evolve across phases without one owner;
- no plan carries a threat model or explicit security tests.

## Strengths

- **Uniform plan shape:** Outcome, Recommendation with evidence, Implementation Context, Scope and Not building, Tasks with tests and validation, Acceptance, Validation gates, Risks, Agent Notes. An agent can execute any plan top to bottom.
- **Spikes before plans** for the risky integrations, each recorded with what it proved and what it didn't.
- **Every owner decision is recorded with its date** and carried into the spec, the PRD and the plans the same day (the report's decision list).
- **Rules are applied consistently:** one commit per mode, unattended tests, flags rather than environment variables, made-up fixture names, nothing from `docs/local/` in committed files.

## Analysis

### Dependencies and parallelism

| Phase | PRD: Parallel / Depends | What the plans actually need |
|---|---|---|
| 5 | with 6 / 4 | — |
| 6 | with 5 / 4 | Phase 5's workspace and lints (Task 1), the hidraw fake-tree builder it moves into `odc-testkit` (Task 7), `packaging/udev/70-opendeepcool.rules` and `scripts/dev/*` (Task 8), the hardware probe (Task 9), CI and the guard (Task 10) |
| 8 | with 9, 10 / 7 | Phase 7 |
| 9 | with 8, 10 / 7 | Phases 7 **and 8**: `draws`, `cycle_match`, `--modes`, the temperature rules for modes 18–19 (its own Related Plans) |
| 10 | with 8, 9 / 7 | Phases 7, **8 and 9**: the Music setters, `AudioConnected`, `odc_render::music` for the tiles, every mode drawable for the real-mode test (its own Related Plans) |

Phase 5 and 6 worktrees would edit the same files (the workspace manifest, `odc-testkit`, the udev rule, the dev scripts, the CI workflow), and Phases 9 and 10 would start before their inputs exist. The plans are right and the PRD table is stale. The Phase 4 report repeats the PRD ("Phase 6 can run alongside it").

### The cost of real-hardware runs

The overview estimates the whole real-cooler cross at days per run by Phase 9, against "a few hours" in the Phase 7 plan. No plan states a budget, a count of runs or a time limit. Phase 8's `--modes` flag limits each commit's run, but the switch pairs grow with every drawable mode. A plan that doesn't know its own run time can't plan its commits: rule 4 asks for the run before every mode's commit.

### Shared contracts without an owner

- **D-Bus.** Phase 7 defines it and writes the member list into an `odc-ipc` doc comment. Phase 9 adds members. Phase 10 changes two property signatures, `FanInputs` from `a(ssus)` to `a(ssusd)` and `Gpus` from `a(sss)` to `a(sssqqb)`, all under `…OpenDeepCool1`. `settings.md` and the design document describe parts of it. There's no single description, and no test that the service serves what the description says.
- **CI.** Phase 5 creates an Ubuntu job running the whole workspace. Phase 10 adds GTK crates and an Arch container for the UI jobs only. Phase 11 adds `makepkg` and `namcap` in an Arch container. Nobody reconciles the Ubuntu job with the GTK crates, or the PKGBUILD's `check()` with weston-based tests.
- **The hardware probe and `odc-e2e`** grow in every phase (`device`, `sensors`, `gpu-rank`, `service`, `music`, `app`, `package`). That's fine as designed, with one area per phase.

### Security as a plan concern

Security appears in the plans as design choices: the sandbox, polkit, group-only power access, the `Report` type. It doesn't appear as tests or as a threat model. Each plan tests what should work; none tests what should be refused, beyond polkit denials and invalid D-Bus arguments in Phase 7. Missing examples:

- a dev-interface call by a user outside the group, and one without an interface field;
- path-like strings in fan-input arguments;
- non-finite audio samples;
- a forged or symlinked detection record;
- a malformed dev manifest.

### Plan hygiene

- **Line-number references drift.** Phase 7 already switched to section names; Phases 5 and 6 haven't:
  - the Phase 5 plan cites `display.md:143-156` for rotation, which now lies in the Rotation section near the end of the file;
  - it cites `AGENTS.md:109-112` for Commands, a section that has since moved;
  - the Phase 6 plan cites `settings.md:66-69` for the app's reason texts, lines that now hold the end of the GPU choice and the start of the fan inputs.
- **Every plan has an HTML companion**, a second copy to keep in step. Any change from the cross-check must update both.
- **Only Phase 8** has a task to check the previous phase's real names (`picture`, `Marquee`, …) before starting. Phases 9, 10 and 11 depend on just as many names.

## Findings

| ID | Severity | Category | Title |
|---|---|---|---|
| P4-01 | Medium | Coherence | The PRD's Parallel and Depends columns contradict the plans (6 needs 5; 9 needs 8; 10 needs 8 and 9) |
| P4-02 | Medium–High | Process, bottleneck | No plan budgets its real-hardware run; the whole cross is estimated at days |
| P4-03 | Low | Maintainability | Line-number references have drifted |
| P4-04 | Low–Medium | Coherence | The D-Bus contract has no single committed description or conformance test |
| P4-05 | Medium | Coherence, CI | The CI environment evolves across phases without being reconciled |
| P4-06 | Low | Maintainability | Plans and their HTML companions must be kept in step |
| P4-07 | Low | Process | Only Phase 8 checks the previous phase's real names before starting |
| P4-08 | Medium | Security process | No threat model and no "must be refused" tests in the plans |

### P4-01: Dependencies and parallelism

- **Evidence:** PRD Implementation Phases and Parallelism Notes; the Phase 4 report; the Related Plans of Phases 9 and 10; Phase 6's Mandatory reading and Existing patterns.
- **Recommendation:** correct the PRD table at the cross-check (owner's document):
  - Phase 6 depends on 5, or at least on Phase 5 Tasks 1, 7, 8, 9 and 10;
  - Phase 9 depends on 7 and 8;
  - Phase 10 depends on 7, 8 and 9;
  - parallel work only where files don't overlap. One option: Phase 10's pure view model and theme modules (Task 3) alongside Phase 9.
- **Outcome (2026-10-27):** accepted, without the overlap: one phase at a time, 5 to 11 (owner). The PRD table's Parallel and Depends columns and its Parallelism Notes now match the plans; the notes' "all four" is gone, since only three phases were left in that group after Creative Mode was removed. Decisions Log: Phase order. This also settles P5-12 and OV-09.

### P4-02: Real-hardware run budget

- **Evidence:** Phase 7 Task 9 ("It takes a few hours"); Phase 8 Risks ("takes hours"); Phase 9 Task 9 (506 ordered pairs); Phase 11 Task 7 ("takes hours"); the overview's estimate.
- **Recommendation:** each plan states its real-run count and expected duration, and Phase 7 measures the time per scenario on the first real run. Then the owner chooses a tiering (overview Q1).
- **Outcome (2026-10-26):** decided through OV-04. On the real cooler, one example of each mode and of each test type; the whole cross runs in-process. AGENTS.md rule 5, the PRD's Testing Strategy, and the Phase 5, 7, 8, 9 and 11 plans follow it.

### P4-03: Line-number references

- **Evidence:** the examples above.
- **Recommendation:** cite sections (`display.md` Rotation) as the Phase 7 plan does, and fix the stale ones at the cross-check.
- **Outcome (2026-10-27):** accepted and applied: all 35 line-number references in the Phase 5 and 6 plans now name sections. Several had drifted onto other text, and `protocol.md:5-91` stopped before the Standby section that Phase 5 implements.

### P4-04: The D-Bus contract

- **Evidence:** Phase 7 Task 4; Phase 9 Task 3; Phase 10 Task 2; `settings.md` What the settings app must offer; `settings-app.md` "What the Phase 10 plan must add to the D-Bus contract".
- **Recommendation:** one committed description, either `docs/spec/dbus.md` or an introspection XML under `docs/spec/`. A layer-3 test compares it with the service's introspection. The contract is frozen at v1.0.0; any incompatible change after that gets `…OpenDeepCool2`, as Phase 7 already intends.
- **Outcome (2026-10-27):** accepted, with a correction: since the overview round the service has no readable D-Bus properties, so the shapes Phase 10 changes are state keys. The contract is one document, `docs/spec/dbus.md`, created in Phase 7 Task 4: the interfaces' XML, the state keys with their types (the XML can't show them inside an `a{sv}`), the errors and each member's polkit action. Phase 7 Task 7 tests the service against it, Phases 9 and 10 update it in the same commit as their code, and it's frozen at `v1.0.0`. Phase 9 Task 3 still listed its keys as "Properties"; corrected. PRD: Settings app ↔ service, Decisions Log. This also settles P7-17, P10-11 and OV-20.

### P4-05: CI environment

- **Evidence:** Phase 5 Task 10 (`ubuntu-latest`, `cargo test --workspace`); Phase 10 Task 10 (UI jobs in `archlinux`); Phase 11 Task 2 (`check()` runs the hardware-free tests).
- **Recommendation:** run all CI in an `archlinux` container from Phase 5. It's the only supported platform, it has a current `systemd` for `udevadm verify`, and GTK is new enough. Mark the UI tests that need weston as their own target, outside `cargo test --workspace` and `check()` (P10-04, P11-08).
- **Outcome (2026-10-27):** accepted. Every CI job runs in the official `archlinux:base-devel` container from Phase 5 (Docker Hub, read 2026-10-27), which also removes Phase 5's `udevadm verify` risk. The weston tests sit behind `odc-app`'s `weston-tests` feature, so `cargo test --workspace`, the guard and the PKGBUILD's `check()` need no display, and CI runs them in a job of their own. Phase 5 Task 10 and Risks; Phase 10 Tasks 3 and 10; Phase 11 Task 2; PRD Testing Strategy and Decisions Log. This also settles P10-03, P10-04, P11-08 and OV-10.

### P4-06: HTML companions

- **Recommendation:** after any plan change from the cross-check, regenerate its companion (`/prp-core:prp-companion`) in the same commit.
- **Outcome (2026-10-27):** accepted from Phase 5 on: a plan change regenerates its HTML companion in the same commit (AGENTS.md How to work; `CLAUDE.md` names the command). During the cross-check, the companions are still regenerated once, when it ends, as the owner decided on 2026-10-26.

### P4-07: Name checks between phases

- **Evidence:** Phase 8 Risks ("Before Task 1, check this plan against the Phase 7 code").
- **Recommendation:** add the same first step to Phases 9, 10 and 11.
- **Outcome (2026-10-27):** accepted as one rule for every phase, since Phases 6 and 7 depend on earlier names too: before a phase's first task, its plan is compared with what the earlier phases built, and fixed with the owner's OK where they differ (AGENTS.md How to work). Phase 8's own risk row stays.

### P4-08: No threat model, no refusal tests

- **Recommendation:** keep the overview's threat model as the reference, and add "must be refused" tests to the plans that own each entry point:
  - Phase 7: a dev-interface call by a user outside the group, and one without an interface field; path-like fan-input strings; an unknown PCI address;
  - Phase 9: NaN and infinite samples; an attach from an inactive session;
  - Phase 6 and Phase 11: a symlinked or forged detection record;
  - Phase 11: a manifest with `..`, `/`, an empty line or a path outside the dev locations.
- **Outcome (2026-10-27):** accepted. The overview round had already added most of these: dev-interface calls from outside the group and without an interface field, unknown and path-like D-Bus arguments (Phase 7), forged detection records (Phases 6 and 11), and refused audio attaches (Phase 9). Added now: NaN and infinite samples count as 0, a rule the owner confirmed (`music.md`, Phase 9 Task 3), and a malformed dev manifest removes nothing (Phase 5 Task 8, Phase 11 Tasks 2 and 3), since root deletes what it lists from Phase 5 on. AGENTS.md rule 5 now asks for refusal tests at every entry point, with the overview's threat model as the reference. This also settles OV-25.

## Questions for the owner

1. Correct the PRD's dependency and parallelism columns (P4-01)?
2. Choose a real-hardware tiering once Phase 7 has measured a scenario's run time (P4-02, overview Q1)?
3. Move CI to an Arch container from Phase 5 (P4-05)?

## Sources

- The plans, the Phase 4 report and the PRD, at the commit above.
