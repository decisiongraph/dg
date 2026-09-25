---
status: deprecated
author: jack
date: 2017-06-01

tags:
  - architecture
  - enterprise
  - hardware
  - pivot
superseded_by: ADR-004
depends_on:
  - ADR-001
---

# The Box Hardware Appliance

## Context

Under new CEO jack's leadership, Pied Piper faced pressure to generate immediate revenue. The cloud platform (ADR-002) was struggling with customer acquisition costs and long enterprise sales cycles. Jack proposed pivoting to a hardware appliance model - "The Box" - which would provide instant revenue recognition and appeal to enterprise procurement processes familiar with buying hardware.

Jack's experience at Hooli convinced him that enterprises prefer to "own" their infrastructure rather than pay ongoing cloud subscriptions. The Box would be a physical server appliance pre-loaded with Pied Piper's compression technology.

## Decision

We will design and manufacture a hardware appliance called "The Box" with the following specifications:

1. **Form Factor**: 2U rack-mounted server appliance
2. **Software**: Embedded Pied Piper compression engine with local web UI
3. **Pricing**: $50,000 per unit with optional support contracts
4. **Target Market**: Enterprise data centers, media companies, healthcare providers
5. **Manufacturing**: Partner with contract manufacturers in Shenzhen

The Box will operate as a standalone appliance, processing data locally without requiring cloud connectivity.

## Consequences

### Positive

- Immediate revenue recognition upon hardware sale
- Appeals to security-conscious enterprises wanting on-premise solutions
- No ongoing infrastructure costs for Pied Piper
- Familiar procurement model for enterprise buyers

### Negative

- High upfront capital required for manufacturing
- Long lead times for hardware iteration
- Support and maintenance burden
- Cannot leverage software improvements without hardware refresh
- Limits market to enterprises with data center capacity

### Neutral

- Requires building hardware engineering competency
- Changes company identity from software to hardware

## Alternatives Considered

### Alternative 1: Enhanced Cloud Platform

**Description:** Double down on the cloud platform model with better sales and marketing.

**Pros:**
- Lower capital requirements
- Faster iteration cycles
- Recurring revenue model

**Cons:**
- Continued long sales cycles
- High customer acquisition costs
- Competitive pressure from larger cloud providers

**Why not chosen:** jack believed the sales cycle would never shorten enough to satisfy investors.

### Alternative 2: Hybrid Model

**Description:** Offer both cloud and hardware options to customers.

**Pros:**
- Addresses multiple market segments
- Flexibility for customer preferences

**Cons:**
- Split engineering focus
- Complex pricing and support
- Higher operational overhead

**Why not chosen:** Insufficient resources to pursue both strategies simultaneously.

## References

- jack's board presentation "The Future is Box-Shaped"
- Hooli Nucleus hardware division case study
- Contract manufacturing agreements with Shenzhen partners

## Notes

- richard strongly opposed this direction, leading to significant internal conflict
- The Box ultimately failed to gain market traction as enterprises increasingly preferred cloud solutions
- This decision was reversed after Jack's departure, with the team pivoting to the decentralized PiperNet model (ADR-004)
- Only 3 units were ever sold before the project was cancelled
