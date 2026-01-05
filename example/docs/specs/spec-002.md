---
title: Zero-downtime PostgreSQL Migration to Bare Metal
status: review
priority: must
author: eve
date: 2025-07-01
tags: [infrastructure, database, migration]
implements: [OPP-002]
depends_on: [ADR-002]
---

## Story

As a platform engineer, I want to migrate our PostgreSQL database from AWS RDS to bare-metal servers using logical replication, so that we achieve zero downtime during the infrastructure migration.

## Scenarios

```gherkin
Feature: Zero-downtime database migration

  Scenario: Logical replication catches up
    Given the bare-metal PostgreSQL replica is initialized from a base backup
    And logical replication is streaming from AWS RDS
    When the replication lag drops below 100ms
    Then the replica is marked as "ready for cutover"

  Scenario: DNS cutover to bare-metal
    Given the bare-metal replica replication lag is under 100ms
    When the operator triggers the cutover command
    Then the RDS primary is set to read-only
    And the bare-metal replica is promoted to primary
    And the DNS CNAME is updated to point to the bare-metal IP
    And application connections failover within 30 seconds

  Scenario: Rollback to RDS on failure
    Given the cutover to bare-metal has been completed
    When a critical error is detected within the rollback window
    Then reverse replication from bare-metal to RDS is activated
    And the DNS CNAME is reverted to the RDS endpoint
    And no committed transactions are lost

  Scenario: Connection pool health after migration
    Given the application uses PgBouncer for connection pooling
    When the database endpoint changes to bare-metal
    Then PgBouncer reconnects all idle connections within 10 seconds
    And active transactions complete on the old connection before recycling
```

## Acceptance Criteria

- Logical replication lag stays under 500ms during normal operation
- Total application downtime during cutover is under 30 seconds
- Rollback procedure tested and documented, executable within 5 minutes
- PgBouncer configuration updated for bare-metal connection parameters
- Monitoring dashboards show replication lag, connection count, and query latency for both environments
