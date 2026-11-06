@AGENTS.md

## Machine-specific facts (local only)

Imported from the owner's git-ignored file (absent on other machines). Use it for implementation work, but never copy its contents into committed files, commit messages, PR texts or other outward-facing text, and keep personal identifiers out of web searches and other outside queries (AGENTS.md rule 10):

@docs/local/machine.md

## Claude Code

- **PRP workflow**:
  - `/prp-core:prp-plan` plans the current phase from the PRD.
  - **Solution review** (no command of its own): after `/prp-core:prp-plan` and before `/prp-core:prp-implement`, review the plan into `docs/prp/solution-reviews/`, then cross its findings with the owner ([README](docs/prp/solution-reviews/README.md); AGENTS.md, How to work).
  - `/prp-core:prp-implement` carries out a plan and writes its report.
  - `/prp-core:prp-prd-update` records progress in the PRD table.
  - `/prp-core:prp-commit` makes commits.
  - `/prp-core:prp-spike` runs a spike, a throwaway proof of an idea. The Wine reference spike was Phase 1 and is complete.
  - `/prp-core:prp-review` reviews changes.
- **Reverse engineering of DeepCreative**: use the `reverse-engineer-anything` skill (REA). Do static analysis first.
- **Settings app design**: the `ui-ux-designer` agent (`.claude/agents/ui-ux-designer.md`) designs every screen and mockup of the settings app. Run it before planning Phase 10; never copy DeepCreative's interface.
- **Library and API docs**: look up a crate in the Context7 MCP server (or Firecrawl) before writing code against it. Don't rely on memory.
