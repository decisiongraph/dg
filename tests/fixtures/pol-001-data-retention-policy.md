---
status: active
owner: alice
approver: cto
effective_date: "2025-01-01"
review_date: "2026-01-01"
category: policy
audience:
  - engineering
  - legal
  - compliance
tags:
  - data
  - gdpr
  - privacy
caused_by: "ADR-001"
blocks:
  - "OPP-001"
enables:
  - "ADR-002"
---

# Data Retention Policy

Define how long data is retained and when it must be deleted to comply with GDPR and internal standards.

## Purpose

Define how long data is retained and when it must be deleted to comply with GDPR and internal standards.

## Scope

Applies to all production systems storing personally identifiable information (PII).

## Policy

- PII must be deleted within 30 days of account closure
- Logs containing PII must be rotated every 90 days
- Backups containing PII expire after 1 year
- Anonymized analytics data may be kept indefinitely

## Compliance

### Requirements

All services must implement automated deletion pipelines. Manual deletion is not acceptable for production systems.

### Exceptions

Data required for active legal proceedings may be retained beyond normal limits with legal team approval.

## Roles and Responsibilities

| Role | Responsibility |
|------|---------------|
| Data Owner | Classify data and set retention periods |
| Engineering | Implement automated deletion |
| Legal | Approve retention exceptions |
| Compliance | Audit retention adherence quarterly |
