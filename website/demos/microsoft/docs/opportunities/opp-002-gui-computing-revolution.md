---
status: completed
author: billg
owner: billg
date: 1983-01-19

tags:
  - gui
  - apple
  - platform-shift
enables:
  - ODR-007
related:
  - OPP-001
---

# GUI Computing Revolution

## Description

In January 1983, Apple launched the Lisa, followed by the Macintosh in January 1984. These demonstrated that graphical user interfaces (GUI) were the future of personal computing, threatening Microsoft's command-line DOS dominance.

### Problem Statement

The Macintosh proved users preferred point-and-click interfaces over typing commands. If GUI became the standard, DOS would become obsolete, and Microsoft would lose its platform position.

### Evidence

- Apple Lisa (1983) showed GUI was technically feasible
- Macintosh (1984) achieved mainstream visibility
- Xerox PARC research validated by commercial products
- User studies showed GUI dramatically reduced learning curve
- Desktop publishing (PageMaker, 1985) created "killer app" for GUI

### Threat Assessment

| Factor | DOS Status | Mac Status |
|--------|------------|------------|
| User Experience | Poor | Excellent |
| Learning Curve | Steep | Gentle |
| Visual Appeal | None | High |
| Future-proof | No | Yes |

### Market Window

- Apple's premium pricing left room for affordable GUI
- IBM PC's installed base wanted GUI without switching hardware
- 3-5 year window before Mac might dominate

## Requirements

| Status | Requirement | Owner |
|---|---|---|
| completed | Ship a graphical shell on top of MS-DOS (Windows 1.0, 1985) | @billg |
| completed | Port the productivity apps to the GUI before competitors do (ODR-008) | @steveb |
| completed | Reach a usable, mass-market GUI with Windows 3.0 (ODR-007) | @billg |

## Strategic Alignment

- [x] Builds **Scale Economies** — Windows runs on commodity PCs
- [x] Strengthens **Network Effects** — DOS app compatibility preserves ecosystem
- [x] Enables **Counter-Positioning** — Apple can't license macOS to clones
- [x] Increases **Switching Costs** — Win32 API creates developer lock-in
- [ ] Enhances **Brand**
- [ ] Secures **Cornered Resource**

## Outcome

Microsoft responded with ODR-007 (Commit to Windows). Despite early struggles (Windows 1.0 and 2.0 were poor), Windows 3.0 (1990) succeeded by:
- Running on existing DOS PCs
- Supporting existing DOS applications
- Costing far less than Macintosh

**Result**: Windows eventually achieved 90%+ market share, defeating both Mac and OS/2.
