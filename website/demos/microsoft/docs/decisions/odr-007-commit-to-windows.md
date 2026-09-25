---
status: accepted
author: billg
date: 1983-11-10

tags:
  - product
  - strategy
enables:
  - ODR-008
depends_on:
  - ODR-006
---

# Commit to GUI (Windows) Development

## Setting
Apple releases the Lisa and then the Macintosh. The GUI (Graphical User Interface) is clearly the future. MS-DOS is command-line. Microsoft needs a GUI answer, but IBM is pushing OS/2 (with Microsoft). Bill Gates knows they need their own GUI product that runs on top of DOS to bridge the gap.

## People
- **Responsible**: Bill Gates
- **Approvers**: Steve Ballmer (marketing lead for Windows)
- **Consulted**:
- **Informed**: IBM (awkward conversation)

## Alternatives

### Option A: Stick with DOS
**Pros:**
- Cash cow.
- High market share.

**Cons:**
- Obsolete long-term.
- Macintosh will eat the high-end market.

### Option B: Focus purely on OS/2 (IBM partnership)
**Pros:**
- Keeps IBM happy.
- Technically superior (protected mode).

**Cons:**
- IBM controls the standard.
- Heavy, expensive requirements.

### Option C: Build Windows (GUI on DOS)
**Pros:**
- Backward compatibility with DOS (huge installed base).
- Microsoft owns it 100%.
- Defensive play against Apple.

**Cons:**
- Technically extremely difficult (DOS wasn't meant for this).
- Performance is terrible on 1983 hardware.
- "Vaporware" reputation (took 2 years to ship 1.0).

## Decision
Chosen: **Option C**

Rationale: "Windows is the bridge." It allows existing PC users to move to GUI without throwing away their hardware or software. It protects the DOS franchise.

## Consequences

### Positive
- Windows 3.0 (1990) eventually conquers the world.
- Defeats OS/2.
- Defeats Mac (in market share).

### Negative
- Years of struggle and bad reviews for Windows 1.0 and 2.0.
