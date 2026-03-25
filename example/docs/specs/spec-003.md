---
status: implemented
priority: should
author: eve
date: 2025-04-10
tags: [monitoring, grafana, observability]
implements: [OPP-003]
title: Unified Grafana Dashboard for All Services
---


## Story

As an on-call engineer responding to an incident, I want a single Grafana dashboard showing health metrics for all services, so that I can identify the failing component within 60 seconds without switching between monitoring tools.

## Scenarios

```gherkin
Feature: Unified service health dashboard

  Scenario: Dashboard loads all service panels
    Given the engineer opens the "Service Overview" Grafana dashboard
    Then panels for API, Worker, and Database are visible
    And each panel shows request rate, error rate, and P99 latency
    And data refreshes every 10 seconds

  Scenario: Alert fires and links to dashboard
    Given the API error rate exceeds 5% for 2 minutes
    When Alertmanager fires the "high-error-rate" alert
    Then a PagerDuty notification is sent within 30 seconds
    And the notification includes a deep link to the relevant Grafana panel

  Scenario: Historical comparison during incident
    Given an incident is in progress
    When the engineer selects "Compare to last week" in Grafana
    Then the dashboard overlays current metrics with the same time range from 7 days ago
    And anomalies are visually highlighted

  Scenario: New service auto-discovered
    Given a new service is deployed with Prometheus annotations
    When Prometheus scrapes the service's /metrics endpoint
    Then the service appears in the "Service Overview" dashboard within 5 minutes
    And default panels (request rate, error rate, latency) are auto-generated
```

## Acceptance Criteria

- Single dashboard URL bookmarkable by all on-call engineers
- Dashboard loads in under 3 seconds with 30 days of data retention
- Alertmanager rules migrated from Datadog monitors with matching thresholds
- PagerDuty integration tested with synthetic alerts before go-live
- Runbook links embedded in each alert annotation

