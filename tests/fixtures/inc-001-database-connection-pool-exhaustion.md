---
status: resolved
severity: sev2
commander: onni
responders:
  - alice
  - bob
started_at: "2025-01-20T14:32:00Z"
resolved_at: "2025-01-20T16:05:00Z"
duration_minutes: 93
affected_systems:
  - api-gateway
  - user-service
  - billing-service
customer_impact: degraded
tags:
  - database
  - performance
caused_by: "ADR-001"
triggers:
  - "ADR-003"
enables:
  - "OPP-001"
related:
  - "POL-001"
---

# Database Connection Pool Exhaustion

Database connection pool exhausted due to leaked connections from a new ORM migration. API latency spiked to >10s, affecting 40% of requests.

## Summary

Database connection pool exhausted due to leaked connections from a new ORM migration. API latency spiked to >10s, affecting 40% of requests.

## Impact

- 40% of API requests experienced >10s latency
- 5% of requests failed with 503 errors
- Billing webhooks delayed by ~2 hours
- No data loss

## Timeline

| Time | Event | Actor |
|------|-------|-------|
| 14:32 | Alerts fire: API p99 >5s | PagerDuty |
| 14:35 | Incident declared, war room opened | @onni |
| 14:42 | Identified: connection pool at 100% | @alice |
| 14:50 | Root cause found: missing connection.close() in new ORM code | @alice |
| 15:10 | Hotfix deployed to staging | @bob |
| 15:25 | Hotfix deployed to production | @bob |
| 15:40 | Connection pool recovering | @onni |
| 16:05 | All metrics nominal, incident resolved | @onni |

## Root Cause

PR #1234 migrated the user-service to a new ORM. The new query builder didn't auto-close connections in error paths, causing connection leaks under load.

## Resolution

- Hotfix: added explicit connection.close() in finally blocks
- Increased connection pool from 20 to 50 as temporary buffer
- Rolled back to old ORM for billing-service (not yet migrated)

## Action Items

| Action | Owner | Due Date | Status |
|--------|-------|----------|--------|
| Add connection leak detection to CI | @alice | 2025-02-01 | done |
| Load test ORM migration paths | @bob | 2025-02-05 | in-progress |
| Add connection pool metrics dashboard | @onni | 2025-01-25 | done |
| Review all ORM migration PRs for leaks | @alice | 2025-02-10 | pending |

## Lessons Learned

- ORMs that don't auto-close connections in error paths are dangerous
- Connection pool exhaustion is hard to detect without per-service pool metrics
- We need load testing for data layer changes, not just API endpoints
