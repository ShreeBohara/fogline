# Decision records

One page per decision that would be expensive to reverse: what was decided, what else was considered, and the evidence.

| # | Decision | Status |
|---|---|---|
| [0001](0001-repository-name-versions-conventions.md) | Repository, name, versions and conventions | Accepted |

Still to be written in Phase 1 (section numbers refer to the [Phase 1 spec](../specs/phase-1.md)):

| # | Decision | Decided by |
|---|---|---|
| 0002 | Engine language | The gate in section 10 |
| 0003 | Map renderer | The spike in section 9 |
| 0004 | Distribution through npm | Milestone M2, section 5.8 |
| 0005 | Scan semantics and bundle v0 | Milestone M3, sections 5.2 to 5.5 |

## Template

```markdown
# ADR-NNNN: Title

Status: proposed | accepted | superseded by ADR-NNNN · YYYY-MM-DD

## Context

What forced a decision, and the facts that shaped it.

## Decision

What was decided, stated so someone can act on it.

## Options considered

| Option | Why not |
|---|---|

## Evidence

Measurements, with the command that reproduces each one.

## Consequences

What becomes easier, what becomes harder, and what would make us revisit this.
```
