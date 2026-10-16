# AI agent workflow

This repository may be developed with any coding agent, including Claude Code,
Codex, or another compatible tool. The agent's identity does not change the
project's architecture, priorities, or accepted decisions. The master plan and
the governing documents are the shared source of truth.

Work may span multiple sessions or days, and a different agent may continue it.
No agent should rely on conversational memory or assume that the repository is
unchanged since the previous session. Before proposing implementation details,
changing logic, or editing code, perform this fresh-start review:

1. Read `AGENTS.md` and this workflow.
2. Inspect `git status --short --branch`, the current diff, recent commits, and
   the relevant files. Preserve existing and uncommitted work; do not reset,
   overwrite, or reformat unrelated changes.
3. Read `docs/MASTER_PLAN.md`, `docs/PROJECT_CHARTER.md`,
   `docs/MODULES_AND_ROADMAP.md`, and the relevant sections of
   `docs/architecture/technical-contract.md`. Read applicable ADRs and linked
   governing documents before making a design choice.
4. Identify the current implementation and tests for the task. Treat code,
   tests, roadmap entries, ADRs, and recent commits as evidence of what was
   actually completed; do not assume a feature is missing because it was
   assigned to another agent or was started on another day.
5. State the intended scope and acceptance criteria against the master plan.
   Continue incomplete work from the repository state. If that state conflicts
   with a governing decision, resolve the conflict through the master plan and
   ADR process instead of silently introducing a competing design.
6. Make the smallest coherent change, add or update the appropriate tests, and
   update the roadmap or governing documentation when the status or a decision
   changes. Do not create a second implementation of logic that already exists.
7. Before handing work off, review the diff and record in the final summary what
   changed, what remains, and which checks were run (or not run). Durable
   project decisions belong in the governing documents, not only in chat.

## Authority and continuity

When sources disagree, follow the authority order documented in
`docs/MASTER_PLAN.md`: the master plan governs project direction, accepted ADRs
record architectural decisions, and the technical contract binds
implementations. The roadmap tracks progress but does not override those
decisions. Propose an ADR when a task requires a new architectural decision;
do not let an agent-specific preference become an undocumented project rule.

Each agent must work from the checked-out branch and current files. A handoff
note is useful only for task-specific context not already captured in code or
documentation; it never replaces a fresh repository scan. The same procedure
applies when resuming after a day, a week, or a change of agent.
