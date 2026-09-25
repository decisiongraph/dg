---
status: superseded
author: dinesh
date: 2015-03-01

tags:
  - architecture
  - b2b
  - enterprise
  - scalability
superseded_by: ADR-004
depends_on:
  - ADR-001
---

# Enterprise Platform Architecture

## Context

After winning TechCrunch Disrupt and securing funding from Raviga Capital, Pied Piper needed to transform from a demo into a production-ready enterprise platform. The middle-out compression algorithm had been proven, but the infrastructure to deliver it as a service to enterprise customers did not exist. Companies like Hooli, EndFrame, and potential enterprise clients demanded SLAs, security guarantees, and scalable architecture.

The initial prototype was a single-server application that could demonstrate compression but couldn't handle concurrent users or provide the reliability enterprises required.

## Decision

We will build a cloud-based platform architecture with the following components:

1. **API Gateway**: RESTful API for compression/decompression requests
2. **Microservices Architecture**: Separate services for encoding, decoding, and job management
3. **Horizontal Scaling**: Auto-scaling worker nodes for compression jobs
4. **Multi-tenant Isolation**: Separate data and processing for each enterprise client
5. **AWS Infrastructure**: Leverage EC2, S3, and SQS for reliable cloud hosting

The architecture follows a producer-consumer pattern where compression jobs are queued and processed by available workers.

## Consequences

### Positive

- Enables serving multiple enterprise clients simultaneously
- Provides 99.9% uptime SLA capability
- Allows incremental scaling as customer base grows
- Establishes foundation for SOC 2 compliance

### Negative

- Significant infrastructure costs compared to single-server approach
- Increased operational complexity requiring DevOps expertise
- Latency overhead from distributed architecture
- Dependency on AWS limits negotiating leverage

### Neutral

- Requires hiring dedicated infrastructure engineers
- Sets precedent for cloud-first deployment strategy

## Alternatives Considered

### Alternative 1: On-Premises Appliance

**Description:** Ship physical hardware appliances to enterprise customers with pre-installed compression software.

**Pros:**
- No ongoing cloud infrastructure costs
- Data never leaves customer premises
- One-time sale model

**Cons:**
- High upfront manufacturing costs
- Difficult to update and maintain
- Requires hardware support team

**Why not chosen:** Lack of capital for hardware manufacturing and desire for recurring revenue model.

### Alternative 2: Peer-to-Peer Distribution

**Description:** Leverage user devices to form a distributed compression network.

**Pros:**
- Near-zero infrastructure costs
- Massive potential scale
- Decentralized and resilient

**Cons:**
- Complex coordination challenges
- Unpredictable performance
- Privacy and security concerns

**Why not chosen:** Enterprise customers require guaranteed performance and data isolation. This approach was later revisited as PiperNet (ADR-004).

## References

- Raviga Capital term sheet requirements
- Hooli enterprise RFP specifications
- AWS Well-Architected Framework

## Notes

- This architecture served Pied Piper through Series A but was eventually superseded by the decentralized PiperNet architecture (ADR-004) when the team pivoted away from centralized cloud hosting.
- The "Tres Commas" tequila incident occurred during the architecture planning phase.
