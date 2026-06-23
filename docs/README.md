# wifi docs

AI-maintained documentation for the **`wifi`** crate. Lean by design: one source of truth per
fact. Code is truth for behavior; git history is the task log; these docs hold the *why*, the
conventions, and the decisions the code can't tell you.

## Read order
1. [AI_CONTEXT.md](AI_CONTEXT.md) — start here. Compressed context + the workflow for a change.
2. [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md) — what the crate is, at the product level.
3. [ARCHITECTURE.md](ARCHITECTURE.md) — layering, the backend trait, data flow.
4. [AREAS/](AREAS/README.md) — one doc per real module; read the one you're touching.
5. [ROADMAP.md](ROADMAP.md) — phased plan, what's built vs. planned.
6. [DECISIONS/](DECISIONS/README.md) — durable architectural decisions (ADRs).

## Maintenance rules
- Don't duplicate. If a fact lives in one doc, others link to it.
- Don't create a per-task changelog directory — git already is one.
- Update an area doc / ADR only when lasting structure, conventions, or decisions change.
- The operating manual is [../AGENTS.md](../AGENTS.md).
