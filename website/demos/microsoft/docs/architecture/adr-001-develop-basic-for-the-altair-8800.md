---
status: accepted
author: billg
date: 1975-02-01

tags:
  - product
  - technical
enables:
  - ODR-003
depends_on:
  - ODR-001
---

# Develop BASIC for the Altair 8800

## Context
Hardware exists (Altair 8800), but it has no software. Users have to toggle switches to input code. To make it useful, it needs a high-level programming language. BASIC is the obvious choice because it's interpreted (good for limited memory), widely known in the hobbyist community, and relatively simple to implement.

## People
- **Responsible**: Bill Gates, Paul Allen, Monte Davidoff
- **Approvers**: Ed Roberts (MITS)
- **Consulted**:
- **Informed**:

## Alternatives

### Option A: Write an Operating System first
**Pros:**
- Controls the hardware directly.
- Standard approach for mainframes.

**Cons:**
- Too complex for 4KB of memory.
- Hobbyists want to write programs immediately.

### Option B: Write a BASIC Interpreter
**Pros:**
- Fits in 4KB memory (with incredible optimization).
- Allows users to write their own programs.
- Becomes the "OS" for the machine effectively.

**Cons:**
- Extremely difficult to fit full feature set into 4KB.
- Requires simulation of the 8080 chip on a PDP-10 to develop (since they didn't have an Altair yet).

## Decision
Chosen: **Option B**

Rationale: BASIC unlocks the utility of the hardware for the mass market (hobbyists). It acts as both the language and the operating environment.

## Consequences

### Positive
- Became the standard for all microcomputers (Altair BASIC, then Apple, Commodore, etc.).
- Generated initial revenue stream.

### Negative
- Rampant piracy (leading to the "Letter to Hobbyists").
