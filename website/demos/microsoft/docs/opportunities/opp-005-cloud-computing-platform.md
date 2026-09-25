---
status: completed
author: rayozzie
owner: rayozzie
date: 2006-03-14

tags:
  - cloud
  - aws
  - azure
  - platform-shift
enables:
  - ADR-003
related:
  - OPP-004
  - ODR-015
---

# Cloud Computing Platform

## Description

In 2006, Amazon launched EC2 and S3, pioneering Infrastructure-as-a-Service (IaaS). This represented a fundamental shift from software licensing to utility computing—threatening Microsoft's core Windows Server and enterprise licensing business.

### Problem Statement

Cloud computing inverted the enterprise software model:

```text
Traditional:  License → Install → Manage → Scale manually
Cloud:        API call → Instant → Managed → Scale automatically
```

If compute became a commodity utility, Windows Server licenses would be replaced by pay-per-hour cloud instances. Microsoft's enterprise cash cow was at risk.

### Evidence

- AWS S3 launched March 2006, EC2 August 2006
- Startups choosing AWS over Windows Server for new applications
- Developers praising "spin up server in minutes, not weeks"
- Enterprise IT exploring "utility computing" to reduce data center costs
- Google investing heavily in cloud infrastructure

### Threat Level: Strategic

| Asset | Risk Level | Cloud Threat |
|-------|------------|--------------|
| Windows Server | Critical | VMs commoditize OS |
| SQL Server | High | RDS, managed databases |
| Enterprise Licensing | Critical | Pay-per-use replaces perpetual |
| Developer Tools | Medium | Cloud-native tools emerge |
| System Center | High | Cloud management built-in |

### Opportunity

Turn the threat into next growth platform:
- Build Microsoft cloud platform (Azure)
- Differentiate with PaaS, not just IaaS
- Migrate enterprise customers to Microsoft cloud
- Capture new cloud-native workloads
- Leverage existing developer ecosystem (.NET)

## Requirements

| Status | Requirement | Owner |
|---|---|---|
| completed | Launch Windows Azure as a platform-as-a-service offering (ADR-003) | @satya |
| completed | Add infrastructure-as-a-service VMs, including Linux | @satya |
| completed | Move enterprise customers to the cloud with hybrid tooling | @satya |

## Strategic Alignment

- [x] Builds **Scale Economies** — Massive infrastructure investment creates barriers
- [x] Strengthens **Network Effects** — Azure ecosystem attracts developers and ISVs
- [x] Enables **Counter-Positioning** — PaaS differentiation vs. AWS IaaS
- [x] Increases **Switching Costs** — Azure-specific services create lock-in
- [ ] Enhances **Brand**
- [ ] Secures **Cornered Resource**
- [x] Develops **Process Power** — Enterprise sales relationships + cloud operations

## Outcome

Microsoft responded with:
1. Ray Ozzie's "Internet Services Disruption" memo (November 2006)
2. ADR-003: Azure Platform-as-a-Service First architecture

The strategy evolved:
- Azure launched February 2010 (PaaS)
- Added IaaS in 2012 (Virtual Machines)
- Satya Nadella (ODR-015) made "cloud-first" the company strategy in 2014
- By 2023, Azure became Microsoft's largest revenue driver

**Result**: Microsoft successfully transitioned from packaged software to cloud platform, becoming the #2 cloud provider behind AWS.
