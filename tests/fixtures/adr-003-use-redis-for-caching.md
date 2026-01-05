---
status: superseded
author: onni
date: "2025-01-08"
superseded_by: "ADR-005"
caused_by: "INC-001"
---

# Use Redis for Caching

Redis will be our caching layer, building on [ADR-001](./adr-001.md) database choice and the [REST API decision](./adr-002.md).

## Decision

Redis will be our caching layer.

## Consequences

### Positive

Fast in-memory lookups for frequently accessed data.
