---
status: accepted
author: onni
date: "2025-01-10"
reviewers:
  - alice
  - bob
tags:
  - database
  - infrastructure
enables:
  - "OPP-001"
triggers:
  - "POL-001"
related:
  - "ADR-002"
---

# Use PostgreSQL

We will use PostgreSQL as our primary database for reliable, performant data storage.

## Decision

We will use PostgreSQL as our primary database.

### Rationale

PostgreSQL offers the best combination of reliability, performance, and features for ACID-compliant workloads.

### Alternatives Considered

| Option | Score | Notes |
|--------|-------|-------|
| PostgreSQL | 9 | Best overall fit |
| MySQL | 7 | Good but less feature-rich |
| SQLite | 5 | Not suitable for production |

## Consequences

### Positive

Full ACID compliance gives us reliable transactions.

### Negative

Requires more upfront schema design complexity than NoSQL alternatives.
