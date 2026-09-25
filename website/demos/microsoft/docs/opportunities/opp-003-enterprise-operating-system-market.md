---
status: completed
author: billg
owner: billg
date: 1988-01-01

tags:
  - enterprise
  - unix
  - server
  - windows-nt
enables:
  - ODR-011
  - ADR-002
related:
  - OPP-001
  - OPP-002
---

# Enterprise Operating System Market

## Description

By 1988, Microsoft dominated the desktop but had no credible enterprise/server operating system. UNIX owned datacenters, and IBM controlled mainframes. This was a massive market Microsoft couldn't address.

### Problem Statement

Enterprise customers needed:
- Protected memory (DOS crashed constantly)
- Multi-user support (DOS was single-user)
- Networking (DOS had no native networking)
- Security (DOS had none)
- Reliability (DOS was a toy to IT departments)

DOS/Windows couldn't scale to enterprise needs. Microsoft was locked out of the lucrative server market.

### Evidence

- UNIX server market growing 20%+ annually
- IBM AS/400 and mainframes dominated enterprise
- Novell NetWare owned file/print serving
- Fortune 500 IT departments dismissed Microsoft as "consumer software"
- Intel 386 enabled protected mode, making enterprise OS possible on commodity hardware

### Market Gap

| Segment | Leader | Microsoft Position |
|---------|--------|-------------------|
| Mainframe | IBM | None |
| Minicomputer | DEC, Sun | None |
| Server | UNIX, Novell | None |
| Desktop | Microsoft | Dominant |
| Workstation | Sun, SGI | None |

### Opportunity Size

- Server OS market: $10B+ annually
- Growing faster than desktop
- Higher margins than consumer software
- Recurring revenue from support contracts

## Requirements

| Status | Requirement | Owner |
|---|---|---|
| completed | Build a portable, secure, preemptive kernel from scratch (ADR-002) | @cutler |
| completed | Keep Win32 application compatibility with desktop Windows | @cutler |
| completed | Stand up an enterprise sales and support organization (ODR-018) | @steveb |

## Strategic Alignment

- [x] Builds **Scale Economies** — Same codebase scales from desktop to server
- [x] Strengthens **Network Effects** — Windows everywhere = developer preference
- [x] Enables **Counter-Positioning** — UNIX vendors fragmented; Microsoft unified
- [x] Increases **Switching Costs** — Active Directory, Exchange lock-in
- [x] Enhances **Brand** — Enterprise credibility elevates entire company
- [ ] Secures **Cornered Resource**
- [x] Develops **Process Power** — Cutler's team brought enterprise OS expertise

## Outcome

Microsoft responded by:
1. ODR-011: Recruiting Dave Cutler from DEC (legendary VMS architect)
2. ADR-002: Building Windows NT with Hardware Abstraction Layer

Windows NT launched in 1993 and eventually conquered the enterprise:
- Windows NT Server replaced Novell NetWare
- Windows 2000/2003 Server challenged UNIX
- Today, Windows Server runs majority of enterprise workloads

**Result**: Microsoft became an enterprise company, not just a consumer software vendor.
