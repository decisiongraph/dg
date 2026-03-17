---
title: Role-based Access Control for Documents
status: proposed
priority: must
author: alice
date: 2025-06-10
tags: [security, access-control, permissions]
implements: [OPP-001]
depends_on: [ADR-001]
---

## Story

As a team administrator, I want to assign read/write/admin roles to team members on a per-document-type basis, so that sensitive decisions and incident reports are only accessible to authorised people while general policies remain visible to everyone.

## Scenarios

```gherkin
Feature: Role-based access control for documents

  Scenario: Viewer cannot edit a restricted document
    Given a document "INC-003" has access level "restricted"
    And Bob has role "viewer" on the incidents collection
    When Bob attempts to edit "INC-003"
    Then Bob sees an "Access denied" error
    And the document is not modified

  Scenario: Editor can update a document they own
    Given a document "ADR-005" has access level "team"
    And Alice has role "editor" on the architecture collection
    When Alice submits a status change to "deprecated"
    Then the change is saved
    And an audit log entry is created with Alice's identity

  Scenario: Admin can grant access to another user
    Given Carol has role "admin" on the policies collection
    And Dave has role "viewer" on the policies collection
    When Carol promotes Dave to "editor"
    Then Dave can edit policy documents
    And Carol receives a confirmation notification

  Scenario: Public documents are readable without login
    Given a document "POL-001" has access level "public"
    When an unauthenticated user visits the document URL
    Then the document body is rendered in full
    And no login prompt is shown

  Scenario: Access change is reflected immediately
    Given Alice has role "viewer" on the opportunities collection
    And Alice has the opportunities list page open
    When an admin revokes Alice's access
    And Alice refreshes the page
    Then Alice sees an empty list with "No documents available"
```

## Acceptance Criteria

- Three roles supported per collection: `viewer` (read-only), `editor` (read/write), `admin` (read/write/manage)
- Access level per document: `public`, `team`, `restricted` — controlled via frontmatter field `access`
- Role assignments stored in `org.kdl` under each user entry
- All write operations append an immutable audit log entry (timestamp, user, action, document ID)
- Permission checks enforced server-side; client-side UI reflects but does not solely enforce access
- Existing documents without an `access` field default to `team` level

## Open Questions

- [ ] **Guest links**: Should we support time-limited shareable links for external reviewers?
- [ ] **Inheritance**: Should sub-teams inherit parent-team roles automatically?
