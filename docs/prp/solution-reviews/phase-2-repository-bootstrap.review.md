# Phase 2 review: repository bootstrap

**Review ID:** `phase-2-repository-bootstrap`
**Reviewed:** 2026-10-26, at commit `3ec34b7`
**Phase status in the PRD:** complete (2026-10-22)
**Inputs:** PRD (Phase 2; How Agents Work on This Project; Decisions Log: Agent instructions, PRP documents, DeepCool material); `AGENTS.md`; `CLAUDE.md`; `README.md`; `LICENSE`; `.gitignore`; `.githooks/pre-commit`, `commit-msg`, `sensitive-values.sh`; `mise.toml`
**Overview:** [project-overview.review.md](project-overview.review.md)

Phase 2 has no plan file. This review covers what it set up: the agent rules, the privacy and vendor-material guard, and the repository conventions every later phase relies on.

## Verdict

A solid base. One rulebook (`AGENTS.md`) imported by `CLAUDE.md`, a guard that backs up the two rules that matter most in a public repository (no vendor material, no personal or machine data), and a sensitive-value list kept outside git: these are good practice, and rare in a personal project.

The weaknesses are the usual ones of client-side controls: they can be bypassed or simply not installed. Two channels sit outside the guard (tags and other outward-facing text), and a few records have drifted. All findings are Low–Medium.

## What the phase set up

- **`AGENTS.md`:** the start-of-session checklist, the working method and 12 rules, kept well under its 200-line limit.
- **`CLAUDE.md`:** imports `AGENTS.md` and the git-ignored `docs/local/machine.md`, and adds Claude-only notes (the PRP commands, REA, Context7).
- **`.githooks/pre-commit`:** rejects
  - a commit identity other than the GitHub no-reply address;
  - local-only folders (`docs/local/`, `docs/reference/vendor/`, `research/`, `captures/`);
  - vendor file types;
  - files over 2 MiB;
  - DeepCreative code markers;
  - any sensitive value from `docs/local/machine.md`, matched without regard to case.
- **`.githooks/commit-msg`:** the same sensitive-value check on the message.
- **`.gitignore`:** vendor documents, local facts, research, DeepCool file types, build output, local agent settings, the PRP store's `project.json` (which holds an absolute path).
- **`mise.toml`:** a pinned Rust with clippy and rustfmt.

## Strengths

- **The sensitive list is data, kept local, and shared by both hooks** through `sensitive-values.sh`. The guard therefore never publishes the very values it protects.
- **The guard checks staged content** (`git show ":path"`), not the working tree, for the privacy checks.
- **Defence in depth for vendor material:** path, file type, size and code-marker checks.
- **`project.json` ignored**, because it holds an absolute local path. That's a detail that's easy to miss.
- **One rulebook for every agent**, plus an explicit note in the PRD that instruction files are context, not enforcement.

## Analysis

### The guard is a client-side control

It runs only where `git config core.hooksPath .githooks` was set (README, Development), and `git commit --no-verify` skips it. Within its own logic:

- **Identity check.** It reads `git config user.name` and `user.email`. Git records other values when `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME` or `GIT_COMMITTER_EMAIL` is set in the environment, or when the `author.*` or `committer.*` configuration keys are set: both override `user.*` (git(1)). The guard sees neither. `git var GIT_AUTHOR_IDENT` and `git var GIT_COMMITTER_IDENT` report the identities git will actually record.
- **Binary files.** The sensitive-value scan uses `grep -I`, which treats binary files as non-matching. A raster image (`.png`, `.jpg`) isn't a blocked type, so a screenshot or an image with text metadata would pass unscanned. Screenshots of DeepCreative or of the desktop are exactly what research produces.
- **Tags.** Annotated tag messages and release notes aren't commits, so neither hook sees them. The same holds for the AUR repository (Phase 11), a separate git repository without the guard, and for GitHub release text.

Nothing checks the repository on the server side. The sensitive list must stay local, but generic checks don't need it: email addresses other than the no-reply address, `/home/` paths, hidraw or hwmon numbers in prose, and serial-like strings. Those can run in CI on every push.

### Agents see the machine facts in every session

`CLAUDE.md` imports `docs/local/machine.md`, so every session has the owner's hardware, login and disk facts in context. That helps testing, but rule 10 only covers git: "committed files, commit messages, PR texts and commit metadata". Other outward-facing channels aren't covered: web searches and MCP tools that send queries to third parties, published pages, issues, AUR comments, release notes. One line in rule 10 would close it.

### Branching after CI exists

`AGENTS.md` merges each phase straight into `main` and says to revisit once GitHub CI exists, and the Phase 5 plan asks the owner at Phase 5 sign-off. A workflow that keeps "no pull requests" and still avoids a red `main`: push the phase branch, wait for green CI, then fast-forward `main`.

### Record drift

- README "Status" still says "Next, Phase 4", although Phase 4 is complete.
- README "Planned" lists DeepCreative's mode names. That's acceptable where it describes DeepCreative, but OpenDeepCool's own names (Load, Temperature, Power, Music) could follow.
- The PRD footer still reads "Status: DRAFT - needs validation".
- `AGENTS.md` "Verified facts" points at the Phase 1 report for the mode list (P1-02).

## Findings

| ID | Severity | Category | Title |
|---|---|---|---|
| P2-01 | Low–Medium | Privacy control | The identity check misses environment overrides, and the guard can be skipped or not installed |
| P2-02 | Low–Medium | Privacy control | No server-side safety net |
| P2-03 | Low | Privacy control | Binary files, raster images included, aren't scanned for sensitive values |
| P2-04 | Low | Privacy control | Tags, release notes and the AUR repository are outside the guard |
| P2-05 | Low | Privacy | Machine facts are in every agent session, but rule 10 covers only git |
| P2-06 | Note | Maintainability | `.bin` is a blocked type; a future test fixture may need it |
| P2-07 | Low | Process | The branch policy after CI exists isn't decided yet |
| P2-08 | Low | Documentation | Record drift (README status, PRD footer, AGENTS.md mode-list pointer) |

### P2-01: Identity check and bypass

- **Evidence:** `.githooks/pre-commit`, identity block (`git config user.name`, `git config user.email`).
- **Recommendation:** check `git var GIT_AUTHOR_IDENT` and `git var GIT_COMMITTER_IDENT` against the no-reply pattern and the sensitive list. Add a `pre-push` hook that re-checks every commit and tag in the pushed range, which also catches commits made with `--no-verify`.

### P2-02: No server-side safety net

- **Evidence:** the guard runs only locally; CI starts in Phase 5 without a privacy job.
- **Recommendation:** add a CI job that fails on generic patterns needing no private list (non-no-reply email addresses, `/home/` paths, raster images outside `docs/design/`), and check the author and committer of every pushed commit. Keep the private list local.

### P2-03: Binary files aren't scanned

- **Evidence:** `grep -I` in the sensitive-value scan; `blocked_ext` has no image types.
- **Recommendation:** block raster images by default, with an explicit allow-list for a design folder if the project ever needs them. Mockups are HTML today. Alternatively, strip and scan image metadata.

### P2-04: Tags, release notes and the AUR repository

- **Evidence:** the hooks run on commits only; Phase 11 Task 6 creates a separate AUR git repository and a `v1.0.0` tag with release notes.
- **Recommendation:** extend rule 10 to tag messages, release notes and the AUR repository. Generate the AUR files from the guarded main repository (Phase 11 already copies them) and run the same value check before every AUR push.

### P2-05: Rule 10 covers only git

- **Evidence:** `CLAUDE.md` imports `docs/local/machine.md`; AGENTS.md rule 10 names committed files, messages, PR texts and commit metadata.
- **Recommendation:** add to rule 10: "Never send machine facts to an external service (web search, MCP servers, published pages, issues, the AUR) unless the owner asks."

### P2-06: `.bin` is blocked

- **Evidence:** `blocked_ext` includes `bin`.
- **Note:** sensible for vendor firmware. If a test fixture ever needs a binary, give it another extension or an explicit allow-list entry, rather than weakening the rule.

### P2-07: Branch policy after CI

- **Evidence:** AGENTS.md How to work ("Revisit once GitHub CI exists"); Phase 5 plan, Risks.
- **Recommendation:** decide at Phase 5 sign-off. "Fast-forward `main` only after the branch's CI run is green" keeps the no-PR workflow.

### P2-08: Record drift

- **Evidence:** README Status; PRD footer; AGENTS.md Verified facts.
- **Recommendation:** an errata commit before Phase 5, with the owner's OK (the PRD is the owner's document).

## Questions for the owner

1. Extend rule 10 to tags, release notes, the AUR and external services (P2-04, P2-05)?
2. Branch policy once CI exists (P2-07)?

## Sources

- The hook scripts and `.gitignore` in the repository.
- git(1), Environment Variables, read 2026-10-26: `GIT_AUTHOR_NAME` "Overrides the user.name and author.name configuration settings"; `GIT_COMMITTER_EMAIL` "Overrides the user.email and committer.email configuration settings".
- git-var(1): the `GIT_AUTHOR_IDENT` and `GIT_COMMITTER_IDENT` variables.
- `grep --help`: `-I, --ignore-binary`.
