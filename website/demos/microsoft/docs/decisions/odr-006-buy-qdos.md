---
status: accepted
author: billg
date: 1980-08-01

tags:
  - acquisition
  - technical
depends_on:
  - ODR-005
---

# Buy QDOS (Tim Paterson's 86-DOS)

## Setting
Microsoft promised IBM an OS, but they don't have one and don't have time to write one from scratch for the 8086 chip. Tim Paterson at Seattle Computer Products has written "QDOS" (Quick and Dirty Operating System), a clone of CP/M for the 8086.

## People
- **Responsible**: Paul Allen
- **Approvers**: Bill Gates
- **Consulted**:
- **Informed**:

## Alternatives

### Option A: Write new OS from scratch
**Pros:**
- Clean code.
- Full ownership.

**Cons:**
- Impossible deadline (months).
- Risk of bugs.

### Option B: Acquire QDOS
**Pros:**
- It exists and works.
- It's local (Seattle).
- It's legally available.

**Cons:**
- It's "Dirty" (simple, clone).
- Need to negotiate purchase without revealing the IBM deal (to keep price low).

## Decision
Chosen: **Option B**

Rationale: Time-to-market is the only thing that matters. Paul Allen drives over and buys it for $50k (initially) then full rights for another $25k.

## Consequences

### Positive
- Allowed Microsoft to meet IBM's deadline.
- Became MS-DOS.

### Negative
- Legacy codebase issues for decades.
- Legal disputes with SCP later (settled).
