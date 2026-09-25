---
status: deprecated
author: billg
date: 1995-12-07

tags:
  - antitrust
  - browser
  - bundling
  - internet
  - strategy
enables:
  - INC-001
depends_on:
  - ODR-009
---

# Bundle IE Free with Windows

## Setting

December 7, 1995. Netscape dominates the browser market. Their strategy: make the browser the platform, making the OS irrelevant.

Bill Gates' "Internet Tidal Wave" memo (ODR-009) declared the browser war Microsoft's top priority. But Microsoft was years behind.

The question: How to catch up?

## People

- **Responsible**: Bill Gates
- **Approvers**: Microsoft Executive Team
- **Consulted**: Jay Allard (Internet strategy), Steven Sinofsky
- **Informed**: PC OEMs, Industry

## The Netscape Threat

Netscape's "Netscape Everywhere" vision:

```text
Traditional:  Apps → Windows → Hardware
Netscape:     Apps → Browser → Any OS → Hardware
```

If applications ran in the browser, Windows became irrelevant. Netscape could have done to Windows what Windows did to IBM.

## Alternatives

### Option A: Compete on Product Quality
**Pros:**
- No legal risk
- Win on merit

**Cons:**
- Years behind Netscape
- Netscape improving fast
- Market may tip before catch-up

### Option B: License/Acquire Netscape
**Pros:**
- Instant market share
- Remove competitor

**Cons:**
- Netscape not selling
- Antitrust scrutiny
- Hostile acquisition difficult

### Option C: Bundle Free with Windows
**Pros:**
- Instant distribution (100M+ PCs/year)
- Zero marginal cost
- Removes Netscape's pricing power
- "Good enough" beats "best" if free and convenient

**Cons:**
- Netscape will cry foul
- Antitrust exposure
- Damages browser market economics

## Decision

**Chosen: Option C — Bundle IE free with Windows**

Announced December 7, 1995:
- Internet Explorer would be **free**
- IE would ship **integrated with Windows**
- OEMs would include IE on every PC

**Netscape stock dropped 33% that day.**

## The Technical Integration

IE wasn't just "included"—it was **integrated**:

| Integration | Purpose |
|-------------|---------|
| Shell integration | File Explorer uses IE rendering |
| HTML Help | Windows help system uses IE |
| Component sharing | Windows apps use IE DLLs |
| Default browser | No prompt, just default |

This deep integration made removal nearly impossible, strengthening Microsoft's legal position ("it's not bundled, it's integrated").

## Consequences

### Positive (For Microsoft)

1. **Browser war won** — IE reached 95%+ market share by 2003
2. **Netscape destroyed** — Acquired by AOL (1998), browser development halted
3. **Windows protected** — Browser-as-platform threat neutralized
4. **Web standard influence** — IE's quirks became de facto standards

### Negative (For Microsoft)

1. **DOJ lawsuit** (INC-001) — Bundling was central allegation
2. **Breakup order** — Judge Jackson ordered Microsoft split (later reversed)
3. **21-year scrutiny** — Antitrust oversight until 2011
4. **Reputation damage** — "Evil empire" narrative solidified
5. **Innovation slowdown** — IE development stagnated post-victory

### For the Industry

1. **Browser innovation frozen** — IE 6 stagnated for 5 years
2. **Standards fragmented** — IE-specific web development
3. **Firefox emergence** — Open source response to IE dominance
4. **Chrome's rise** — Google eventually disrupted IE

## The Antitrust Central Question

Was IE integration:
- A **feature** (legitimate product improvement)?
- **Tying** (illegal bundling to extend monopoly)?

Microsoft argued: Integration benefits users—faster, more secure, more capable.

DOJ argued: Integration was designed to destroy Netscape and maintain Windows monopoly.

Judge Jackson's 1999 finding: **Both were true.**

## Historical Verdict

The browser bundling decision worked—spectacularly, ruthlessly, and at enormous cost.

Microsoft won the browser war. But the decade of antitrust distraction arguably cost Microsoft the mobile war. The company that destroyed Netscape was too constrained to effectively respond to the iPhone.

> "We won the battle and lost the war. We killed Netscape but Google came out of nowhere and ate the web anyway." — Anonymous Microsoft executive
