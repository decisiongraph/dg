---
status: accepted
author: cutler
date: 2006-11-01

tags:
  - architecture
  - azure
  - cloud
  - paas
  - strategy
depends_on:
  - ADR-002
---

# Azure: Platform-as-a-Service First

## Context

In 2006, Amazon launched EC2 (Infrastructure-as-a-Service). Microsoft faced pressure to respond:

1. **Internal pressure**: "Ship Windows in the cloud" — leverage existing Windows Server
2. **Market pressure**: AWS growing rapidly; Microsoft losing developer mindshare
3. **Technical reality**: Windows Server wasn't designed for multi-tenant cloud

Ray Ozzie (Chief Software Architect) argued for a different approach.

## Decision

Build Azure as **Platform-as-a-Service (PaaS)** first, not Infrastructure-as-a-Service (IaaS).

### Why PaaS?

| IaaS (AWS approach) | PaaS (Azure approach) |
|---------------------|----------------------|
| Rent VMs | Rent application platform |
| Customer manages OS | Microsoft manages everything below app |
| Low abstraction, high flexibility | High abstraction, less ops burden |
| Commoditizes Windows | Differentiates Microsoft |

### Steve Ballmer's Protection

Ballmer made a critical organizational decision: **separate Azure from Server & Tools division**.

Without this, the legacy Windows Server team would have:
- Pushed for IaaS tied to Windows licensing
- Prioritized existing customer migrations over new platform innovation
- Optimized for short-term revenue over long-term positioning

## Architecture

### Original Azure PaaS Stack (2010)

```mermaid
flowchart TB
    App["Your Application"]
    PaaS["Azure Web/Worker Roles (PaaS)"]
    Fabric["Azure Fabric Controller"]
    Hyper["Hypervisor (custom, NT-based)"]
    Infra["Physical Infrastructure"]
    App --> PaaS --> Fabric --> Hyper --> Infra
```

### Key Technical Decisions

| Decision | Rationale |
|----------|-----------|
| Custom hypervisor | Optimize for multi-tenant, not desktop virtualization |
| .NET first-class | Leverage existing Microsoft developer ecosystem |
| Stateless compute | Scale horizontally without session affinity |
| SQL Azure | Managed database without DBA overhead |
| Storage abstraction | Blob/Table/Queue, not filesystem |

## Alternatives Considered

### Option A: Windows Server in Cloud (IaaS)
**Rejected initially**: Would commoditize Windows; no differentiation from VMware/AWS.

### Option B: Pure PaaS (Chosen for v1)
**Pros:**
- Higher abstraction = higher margin
- Harder to commoditize
- Developer-friendly deployment model
- Operational simplicity for customers

**Cons:**
- Requires application rewrite for cloud-native
- Doesn't address "lift and shift" enterprise needs
- Smaller initial market

### Option C: Both (Eventually)
Azure added IaaS in 2012, but PaaS foundation remained core.

## Implementation

### Timeline

| Date | Milestone |
|------|-----------|
| Nov 2006 | Ray Ozzie's "Internet Services Disruption" memo |
| Oct 2008 | Azure announced at PDC |
| Feb 2010 | Azure general availability |
| Jun 2012 | IaaS preview (Virtual Machines) |
| Apr 2014 | Renamed "Microsoft Azure" (dropped "Windows") |

### Dave Cutler's Role

Cutler, architect of Windows NT, built Azure's hypervisor and core infrastructure. His involvement ensured:
- Enterprise-grade reliability
- Efficient virtualization
- Security architecture
- Performance optimization

## Consequences

### Positive

1. **Differentiated platform** — Not just "VMs in cloud"
2. **Developer adoption** — PaaS simplicity attracted startups
3. **Foundation for future** — Architecture scaled to support IaaS, Kubernetes, AI
4. **Margin protection** — Higher-level services = higher margins

### Negative

1. **Slower initial growth** — Enterprises wanted lift-and-shift (IaaS)
2. **AWS head start** — 4-year gap in IaaS capabilities
3. **Migration friction** — PaaS required app rewrites

## Strategic Evolution

| Era | Focus | Revenue Driver |
|-----|-------|---------------|
| 2010-2014 | PaaS | Web/Worker Roles, SQL Azure |
| 2014-2018 | IaaS + PaaS | Virtual Machines, hybrid cloud |
| 2018-present | AI + Platform | OpenAI, Cognitive Services, GitHub |

## Legacy

The PaaS-first architecture proved prescient:
- Kubernetes and serverless built on PaaS principles
- Higher-level services (AI, ML) are modern PaaS
- Azure's differentiation comes from platform, not just infrastructure

> "We insisted on PaaS because we knew infrastructure alone would become a commodity. Platforms create lock-in." — Ray Ozzie
