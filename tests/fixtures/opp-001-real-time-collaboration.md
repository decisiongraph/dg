---
status: exploring
owner: onni
date: "2025-01-15"
priority: high
effort: large
impact: xl
stakeholders:
  - alice
  - bob
enabled_by:
  - "ADR-001"
  - "ADR-002"
triggers:
  - "ADR-003"
blocked_by:
  - "POL-001"
related:
  - "INC-001"
---

# Real-time Collaboration

Users cannot edit documents simultaneously. This causes version conflicts and slows teams down.

## Problem

Users cannot edit documents simultaneously. This causes version conflicts and slows teams down.

## Proposed Solution

Implement operational transform (OT) or CRDT-based real-time editing with cursor presence.

## Success Criteria

- Multiple users can edit the same document simultaneously
- Changes merge without conflicts in >99% of cases
- Latency under 200ms for edits

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| CRDT complexity | High | High | Use proven library (yjs/automerge) |
| Network latency | Medium | Medium | Optimistic local-first updates |
| Data loss | Low | Critical | WAL + periodic snapshots |

## Open Questions

- OT vs CRDT? Need ADR.
- WebSocket vs SSE for transport?
