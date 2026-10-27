# Solution reviews

Architecture, best-practice and security reviews of OpenDeepCool's planned solution: one review of the whole solution, and one per PRD phase. They look for bottlenecks, security flaws, coherence problems between documents, and issues that can already be foreseen before any code exists.

## Where the review sits in the workflow

The solution review is a step of its own, **after a phase's plan and before its implementation**:

1. **PRD:** scope and decisions ([PRD](../prds/opendeepcool-lp360.prd.md)).
2. **Plan:** `/prp-core:prp-plan` writes `docs/prp/plans/phase-N-….plan.md`.
3. **Solution review:** the phase's plan is reviewed into `phase-N-….review.md` here. The whole solution's review (`project-overview.review.md`) is updated when a plan changes the architecture.
4. **Cross-check with the owner:** each finding is crossed with the PRD and the plan. The owner accepts, rejects or defers it, and the agreed changes go into the PRD, the plan or the spec.
5. **Implementation:** `/prp-core:prp-implement`, then its report.

A review only reports. It never changes the PRD, a plan, the spec or code, and this folder holds only `.md` files. It isn't a source of truth: a recommendation becomes a requirement only when the owner accepts it and the PRD, the plan or the spec says so.

## Rules for writing a review

- **Evidence, as in the spec:** cite the document and section, or the primary source and the date it was read. Mark anything not checked as **unverified** (AGENTS.md rule 1).
- **No machine data:** no hardware models, OS or kernel versions, user names, local paths or other facts about the owner's PC (AGENTS.md rule 10). Refer to `docs/local/` instead. The commit guard checks these files like any other.
- **Finding IDs:**
  - `OV-nn` in the overview, which lists the phase findings behind each one;
  - `Pn-nn` in the review of Phase *n*.

  IDs are never reused, even when a finding is withdrawn.
- **Severity:**

  | Severity | Meaning |
  |---|---|
  | Critical | Exploitable as designed, or a core promise fails with no workaround |
  | High | A privilege boundary can be crossed, or a core promise fails in a likely case. Fix in the plan before its phase is implemented |
  | Medium | A realistic failure, privacy, performance or maintainability risk. Fix in the phase's plan |
  | Low | Hardening, hygiene or a rare edge case |
  | Note | An observation; no action needed |

- **Each review ends with its questions for the owner:** the findings whose fix touches an owner rule or decision.

## Reviews

All reviewed on 2026-10-26, at commit `3ec34b7` (Phase 4 complete, Phases 5–11 planned).

| Review | Scope | Findings (High / total) | Status |
|---|---|---|---|
| [project-overview.review.md](project-overview.review.md) | The whole solution: architecture, privileges, threat model, bottlenecks, coherence between plans | 3 / 25 | Paused after OV-01 to OV-14 (2026-10-26): 8 accepted and applied, OV-06 partly decided, OV-11 open, OV-07, OV-09, OV-10 and OV-12 not covered; OV-19 decided later the same day; OV-22 decided and OV-23 partly, through the Phase 2 review, OV-07 through the Phase 3 review, OV-09, OV-10, OV-20, OV-23 and OV-25 through the Phase 4 review, OV-17 and part of OV-16 through the Phase 5 review, OV-12, most of OV-11 and part of OV-15 through the Phase 6 review, the rest of OV-11 through the Phase 8 review, OV-21 and part of OV-24 through the Phase 7 review, and the rest of OV-24 through the Phase 9 review (2026-10-27). Resumes after the phase reviews, for what's still open |
| [phase-1-wine-reference-spike.review.md](phase-1-wine-reference-spike.review.md) | The research rig and how its report is used | 0 / 3 | Done (2026-10-27): P1-01 deferred (only if the rig runs again), P1-02 accepted and applied, P1-03 goes with P3-01 |
| [phase-2-repository-bootstrap.review.md](phase-2-repository-bootstrap.review.md) | Agent rules, the commit guard, repository conventions | 0 / 8 | Done (2026-10-27): all accepted (P2-02 smaller, P2-03 broader, P2-05 lighter); P2-06 no action; P2-07 decided now |
| [phase-3-functionality-protocol-spec.review.md](phase-3-functionality-protocol-spec.review.md) | The spec, the expected frames and the parity checklist | 0 / 7 | Done (2026-10-27): P3-01, P3-02, P3-05 and P3-06 accepted (P3-06 wider: a column on all 88 rows), P3-07 no action; P3-03 and P3-04 decided through the overview; P1-03 settled with P3-01 |
| [phase-4-implementation-plans.review.md](phase-4-implementation-plans.review.md) | The plans as a set: dependencies, shared contracts, budgets | 0 / 8 | Done (2026-10-27): all accepted (P4-01 without the overlap, P4-07 as one AGENTS.md rule); P4-02 decided through the overview |
| [phase-5-device-protocol-core.review.md](phase-5-device-protocol-core.review.md) | Frame encoding, the LP360 writer, udev, dev scripts, CI | 0 / 13 | Done (2026-10-27): all accepted (P5-04 documented, without `sg`; P5-06 without Dependabot; P5-08 with a longer path list), P5-13 no action; P5-01 and P5-05 decided through the overview; P5-11 settled with P3-05, P5-12 with P4-01 |
| [phase-6-sensors.review.md](phase-6-sensors.review.md) | Driver rules, GPU ranking, sensor detection, CPU power access | 1 / 14 | Done (2026-10-27): P6-02 automatic, made safer; P6-06 NVML for NVIDIA only; P6-07 unknown, made visible; the rest accepted; P6-08 and P6-10 notes; P6-01 and P6-05 decided through the overview |
| [phase-7-service-settings-load-mode.review.md](phase-7-service-settings-load-mode.review.md) | The system service, D-Bus and polkit, settings, Load, the dev copy | 1 / 19 | Done (2026-10-27): P7-07, P7-09, P7-12, P7-13, P7-15 and P7-16 accepted; P7-11 and P7-18 notes; P7-01 to P7-06, P7-08 and P7-19 decided through the overview; P7-10 with OV-03 and P6-04, P7-14 with P5-02, P7-17 with P4-04 |
| [phase-8-temperature-power.review.md](phase-8-temperature-power.review.md) | Temperature and Power | 0 / 8 | Done (2026-10-27): P8-03 and P8-06 accepted, P8-08's re-try settled with P6-04, P8-04, P8-05 and P8-07 notes; P8-01 and P8-02 decided through the overview |
| [phase-9-music.review.md](phase-9-music.review.md) | The audio helper, the spectrum, emojis and the Dancer | 0 / 14 | Done (2026-10-27): P9-03, P9-04, P9-06, P9-09, P9-10 and P9-11 accepted; P9-08 already in the plan; P9-05 and P9-12 notes; P9-01, P9-02, P9-07 and P9-14 decided through the overview; P9-13 settled with P4-01 |
| [phase-10-settings-app.review.md](phase-10-settings-app.review.md) | The GTK4 app, the frame preview, the theme, UI tests | 0 / 12 | Waiting for the cross-check; P10-01 and P10-02 already decided (owner, 2026-10-26: only the user at the PC); P10-03, P10-04 and P10-11 settled with P4-05 and P4-04 |
| [phase-11-packaging-final-validation.review.md](phase-11-packaging-final-validation.review.md) | The PKGBUILD, install and uninstall, release, the final parity run | 2 / 21 | Waiting for the cross-check; P11-01, P11-02 and P11-15 already decided through the overview, P11-04 and P11-05 partly (P11-04's detection part settled with P6-02 and P6-03); the release stages (alpha, beta, then `v1.0.0`; owner, 2026-10-27) are designed in this round; P11-08, P11-16 and P11-19 settled with P4-05, P3-06 and P3-01 |

## How the cross-check works

**Order** (owner, 2026-10-26): the phase reviews first, Phase 1 to 11, one at a time, because they hold the deep analysis. The overview comes last, for its findings that are still open. A phase finding already decided through the overview keeps its Outcome line and comes up again only if the owner asks.

For each review, with the owner:

1. Go through its findings, High first, then its questions.
2. For each finding, the owner decides: **accepted** (the PRD, plan or spec changes), **rejected** (with the reason), or **deferred** (to a later phase or after v1, recorded in the PRD).
3. Make the agreed changes in the PRD, the plan and the spec, each with the owner's OK, as AGENTS.md requires. The plans' HTML companions are regenerated once, when the whole cross-check ends.
4. Record the outcome in the review: add an "Outcome" line under each finding, with the date and where the change went. Leave the finding's text as it was written.
5. Then implement the phase.
