---
title: Real-time Document Sync via WebSocket
status: approved
priority: must
author: alice
date: 2025-04-15
tags: [collaboration, websocket, crdt]
implements: [OPP-001]
depends_on: [ADR-001]
---

## Story

As a team member editing a shared document, I want my changes to appear on my teammates' screens within 200ms, so that we can collaborate in real time without overwriting each other's work.

## Scenarios

```gherkin
Feature: Real-time document synchronization

  Scenario: Two users edit the same paragraph
    Given Alice has document "quarterly-report" open
    And Bob has document "quarterly-report" open
    When Alice types "Q3 revenue grew 15%" at line 5
    Then Bob sees "Q3 revenue grew 15%" at line 5 within 200ms

  Scenario: Offline edits merge on reconnect
    Given Alice has document "quarterly-report" open
    And Alice loses network connectivity
    When Alice edits the document offline
    And Alice regains network connectivity
    Then Alice's offline changes are merged without data loss
    And Bob sees the merged document

  Scenario: Concurrent edits to different sections
    Given Alice is editing the "Summary" section
    And Bob is editing the "Details" section
    When both save simultaneously
    Then both sections contain the correct edits
    And no conflict dialog is shown

  Scenario: Connection degradation fallback
    Given Alice has a WebSocket connection to the sync server
    When the WebSocket connection drops
    Then the client falls back to HTTP polling every 5 seconds
    And a "reconnecting" indicator is shown
    And no edits are lost during the transition
```

## Acceptance Criteria

- CRDT engine (Yjs) handles all merge conflicts without user intervention
- WebSocket connection established on document open, cleaned up on close
- Fallback to HTTP long-polling when WebSocket is unavailable
- Sync state indicator visible in the editor toolbar (connected / syncing / offline)
- Maximum 50 concurrent editors per document without degradation

## Open Questions

- [ ] **Cursor presence**: Should we show other users' cursor positions in the MVP?
- [ ] **Undo scope**: Does Ctrl+Z undo only local changes or all recent changes?
