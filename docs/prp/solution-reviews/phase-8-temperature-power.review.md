# Phase 8 review: Temperature and Power

**Review ID:** `phase-8-temperature-power`
**Reviewed:** 2026-10-26, at commit `3ec34b7`
**Phase status in the PRD:** pending (planned)
**Inputs:** [Phase 8 plan](../plans/phase-8-temperature-power.plan.md); `docs/spec/display.md` (Values, Glyphs, layouts, Movement, Test scenarios), `sensors.md` (CPU power, integrated AMD power), `settings.md`; the Phase 6 and 7 plans; PRD (Phase 8)
**Overview:** [project-overview.review.md](project-overview.review.md)

## Verdict

The leanest plan of the set, and rightly so. Because `display.md` defines one strip and one movement for every mode, Phase 8 is data: three glyphs, two value rules, six mode mappings. It adds no process, no setting and no new D-Bus member. Two design choices are especially good:

- **°F in integer arithmetic**, with the cap applied before the multiplication. That's exact at every boundary and can't overflow.
- **The refusal derived from `odc_render::draws(mode)`**, so no second list of "implemented modes" can drift.

The differential test that compares scrolling captures as an ordered cycle is a real improvement over set comparison, and it was proven against 68 captured steps before planning (50 under the nearest rule, recounted 2026-10-26).

The findings are about inheritance:

- integrated AMD power is hidden only when the GPU is ranked "integrated", so a failed ranking shows CPU-inclusive power as Gpu power (P8-06, Medium);
- the full real-cooler cross at the end of the phase is long (P8-02);
- Power modes depend on the udev trigger that the package doesn't run in time (P8-08, from P11-01).

## What the phase adds

- `odc-render`: the `°C`, `°F` and `W` glyphs; the temperature and power value rules with the 9999 cap; pictures for modes 7–12; `draws(mode)`.
- The oracle: dash cases and scenario files for modes 7–12, in both units.
- `odc-testkit::diff::cycle_match`; `odc-e2e --modes`.
- Six commits, one per mode, each with its own matrix, scenario, differential and real-cooler run.

## Strengths

- **One picture, one movement for every mode.** No separate scrolling renderer to keep in step with the static one.
- **Exact °F:** ⌊(9c + 160) / 5⌋ on whole numbers, with a property test over every *c* from 0 to 9999, and the S7 boundaries pinned (37 → `98°F`, 37.01 → `100°F`).
  - *Since then (2026-10-26):* every measurement rounds to the nearest whole number (owner). °F is now rounded once from the exact millidegrees, (9m + 162 500) div 5000, and the plan pins 37 000 m°C → `99°F`. See P8-01.
- **One cap function** used by every metric, so the cooler, D-Bus and the journal can't show different numbers.
- **`--modes`.** Each mode's commit runs its own real-cooler checks, which keeps rule 4 workable.
- **Ordered cycle comparison** of DeepCreative's scrolling captures instead of a set comparison, which would miss frames in the wrong order.

## Analysis

### Value rules and exactness

*Since then (2026-10-26):* the analysis below was written for DeepCreative's "round up". The owner has since chosen the nearest whole number for every measurement, in exact integer arithmetic (`display.md` Values; P8-01 outcome).

- **Temperatures:** millidegrees ÷ 1000 is exact in doubles when the quotient is integral, as the plan notes, and `ceil` of a non-integral quotient is correct. The plan pins it with S7 (37 000 m°C → `98°F`). That's sound.
- **Power from µW** (amdgpu `power1_average`) is exact the same way.
- **Power from energy over time** (RAPL, and i915/xe `energy1_input`) is genuinely continuous. `ceil` of a noisy value is the natural reading of "round up", and DeepCreative applied it to HWiNFO's floats too. Accept it, and say in `sensors.md` that the rule applies to a measured, continuous value.
- The off-by-one problem of rounding up computed ratios is a Load issue (CPU usage, P7-19), not a Phase 8 one. Phase 8 reuses the same rule machinery, so the fix there (integer-carrying readings, P6-05) must not regress the temperature path.

### Integrated AMD power, fail-safe

Phase 6 makes Gpu power `Unavailable(IntegratedGpuPower)` "on a GPU ranked `Integrated`", because amdgpu's reading on an APU includes the CPU (`sensors.md`). Any ranking failure leaves the APU `Unknown`, and its power is then drawn as Gpu power, which breaks rule 11:

- the helper's first run fails on a fresh install (P6-14);
- the query fails;
- a new GPU appears after the user has chosen one, so the helper never reruns.

The safe default is the inverse: show amdgpu power only for a GPU ranked `Dedicated`, and report `Unavailable` with the reason "this GPU's kind is unknown" otherwise. Mode 11 is where it shows, and the change belongs in Phase 6's rule with a test in this phase.

### Availability of the Power modes after install

Modes 10 and 12 need the package energy counter to be group-readable, which the udev rule does when it's applied. In Phase 11's install order, the udev rule is loaded and triggered only by pacman's post-transaction hook, after `post_install` has already started the service (P11-01). If the service starts before the trigger, CPU Power is `PermissionDenied`. The plan counts that as persistent, so the app disables modes 10 and 12 with "permission denied" until the service restarts, even though the permission arrives seconds later. Fixing the install order (P11-01) and re-trying `PermissionDenied` periodically (P6-04's rescan) both remove it.

### The real-cooler cross

`--modes` bounds each commit's run, but each new mode adds its switch pairs with every earlier mode, at every cycle position, in four rotations. By mode 12, "all 132 ordered pairs among modes 1–12 have run", followed by "one full `--only service` run with no `--modes`". By the overview's estimate, the Phase 8 share alone is about 17,000 real runs, roughly 22 h, and the full run at the end includes Phase 7's share too. The plan's Risks row says "takes hours". Measure it in Phase 7 first (P7-08) and let the owner choose the tiering (overview Q1).

### Differential tests

- **Mode 9 in °C can't be compared** (OpenDeepCool ends `°C` at column 14 in two-value modes), so it rests on layer 1's 12 cases and the scenarios. That's acceptable, and the plan says so.
- **`cycle_match` fails on a dropped frame in a capture.** The plan's stance, "investigate, never loosen to a set comparison", is right.

## Findings

| ID | Severity | Category | Title |
|---|---|---|---|
| P8-01 | Low | Correctness | Power from energy over time is continuous; document that "round up" applies to a measured value |
| P8-02 | Medium | Process | The real-cooler cross at the end of the phase is long (estimate: about a day for this phase's share) |
| P8-03 | Note | Design | `draws(mode)` as the single source of "drawable": keep it, and use it in availability too |
| P8-04 | Note | Testing | `cycle_match` fails on a dropped captured frame; investigate rather than loosen (as planned) |
| P8-05 | Note | Testing | Mode 9 °C has no differential coverage; layer 1 and the scenarios carry it |
| P8-06 | Medium | Correctness (rule 11) | Integrated AMD power is hidden only when ranked `Integrated`; an `Unknown` APU shows CPU-inclusive power |
| P8-07 | Note | Spec | Readings below 0 °C draw dashes (owner, 2026-10-24); fine for a liquid-cooled PC |
| P8-08 | Medium | Availability | After install, Power modes are marked unavailable until a restart if the service starts before the udev trigger (P11-01) |

### P8-01: Continuous power

- **Recommendation:** one sentence in `sensors.md` (Updates and rounding): power from energy is a measured value and rounds up as read. Keep the S7 boundary tests for µW-based power.
- **Outcome (2026-10-26):** superseded by OV-08 and the owner's nearest rule. Power from an energy counter is now computed exactly, to the nearest whole number, as (2 · ΔµJ + Δµs) div (2 · Δµs) (`display.md` Values). The Phase 8 plan tests 65,000,000 µJ over 1,000,000 µs → `65W`.

### P8-02: The real-cooler cross

- **Evidence:** plan Task 8 (all 132 ordered pairs; then the whole cross); Risks ("takes hours").
- **Recommendation:** measure (P7-08), then the owner's tiering. Report the actual duration in the Phase 8 report, as the plan already intends.
- **Outcome (2026-10-26):** decided through OV-04. Each mode's commit runs that mode's examples (`--modes`), and the final run takes one example of each mode 1–12 and test type; the 132 switch pairs run in-process.

### P8-03: `draws(mode)`

- **Recommendation:** `AvailableModes` (Phase 7) should also start from `draws`, so a mode that isn't drawable is never offered as available. Today the two are computed separately.
- **Outcome (2026-10-27):** accepted, from Phase 7: the cross-check found that Phase 7's `AvailableModes` counts every mode whose readings work, so it would list Music mode 13 while refusing it. `draws(mode)` now comes with Phase 7's `odc-render`, and both the refusal and `AvailableModes` use it, with a test; Phase 8 only extends it.

### P8-06: Fail-safe for integrated AMD power

- **Evidence:** Phase 6 Task 8 (amdgpu power unavailable "on a GPU ranked `Integrated`"); `sensors.md` (integrated AMD power includes the CPU); P6-14 and P6-07 (ranking failures).
- **Recommendation:** amdgpu Gpu power only when the GPU is ranked `Dedicated`; otherwise `Unavailable` with a reason that names the unknown kind. Add a fixture test: an APU with `Unknown` kind → mode 11 unavailable.
- **Outcome (2026-10-27):** accepted (owner): amdgpu power is shown only on a GPU ranked dedicated; an unknown kind gives a new reason, `UnknownGpuKind`, which counts as lasting. Phase 6 Tasks 2 and 8, Phase 7 availability, Phase 8 mode 11 with a fixture test; `sensors.md`; PRD Decisions Log (AMD GPU power). Nothing changes on the owner's PC, whose card ranks dedicated. This settles the last part of OV-11.

### P8-08: Power modes right after install

- **Recommendation:** fix the install order (P11-01), and let `PermissionDenied` be re-tried by the periodic rescan (P6-04) instead of counting as persistent for the life of the process.
- **Outcome (2026-10-26):** partly applied. The install order is fixed through OV-02: the service starts after systemd's udev hook has applied the rule. The re-try waits for OV-11.
- **Outcome (2026-10-27):** the re-try settled with P6-04: the registry reads the power counter again at every snapshot and looks for its source again after a failed read, and `AvailableModes` follows the latest reading, so modes 10 and 12 return as soon as the permission is in place. No further change.

### P8-04, P8-05 and P8-07: notes

- **Outcome (2026-10-27):** no action; the plan already handles each as the finding says.

## Questions for the owner

- Real-hardware tiering (overview Q1) applies here too. Nothing else specific to this phase.

## Sources

- The plan's own evidence; `display.md` and `sensors.md` at the commit above.
