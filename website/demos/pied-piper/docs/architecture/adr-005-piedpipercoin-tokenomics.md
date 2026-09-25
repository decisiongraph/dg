---
status: deprecated
author: gilfoyle
date: 2018-09-01

tags:
  - architecture
  - blockchain
  - pipernet
  - tokenomics
depends_on:
  - ADR-004
related:
  - INC-003
  - ODR-005
---

# PiedPiperCoin Tokenomics

> **DEPRECATED**: PiperNet and PiedPiperCoin were intentionally destroyed on December 21, 2019 (see ODR-005). This ADR is preserved for historical reference only.

## Context

PiperNet (ADR-004) requires an incentive mechanism to reward node operators who contribute storage and bandwidth to the decentralized network. Without economic incentives, the network would rely on altruism, which has proven insufficient for decentralized systems at scale.

Following the cryptocurrency boom of 2017-2018, creating a native token seemed like the obvious solution. PiedPiperCoin (PPC) would serve as the medium of exchange within the PiperNet ecosystem, rewarding node operators and enabling users to pay for premium services.

Key considerations:
- Token must incentivize long-term network participation, not speculation
- Must avoid regulatory classification as a security (SEC scrutiny)
- Economic model must be sustainable without external funding
- Token distribution must be fair and resist manipulation

## Decision

We will create PiedPiperCoin (PPC) with the following tokenomics:

### Token Distribution

| Allocation | Percentage | Vesting |
|------------|------------|---------|
| Node Rewards Pool | 60% | Released over 10 years via mining |
| Founding Team | 15% | 4-year vesting, 1-year cliff |
| Raviga/Investors | 10% | 2-year vesting |
| Network Development Fund | 10% | Controlled by governance vote |
| Initial Liquidity | 5% | Immediate for exchange listings |

### Mining/Reward Mechanism

- **Proof of Resource**: Nodes earn PPC proportional to storage provided × uptime × bandwidth served
- **Block time**: 60 seconds
- **Initial block reward**: 100 PPC, halving every 2 years
- **Minimum stake**: 100 PPC to operate a node (prevents sybil attacks)
- **Slashing**: Nodes lose stake for data loss or excessive downtime

### Utility Functions

1. **Storage payments**: Users pay PPC to store data on PiperNet
2. **Bandwidth payments**: High-speed retrieval costs additional PPC
3. **Governance voting**: Token holders vote on protocol changes
4. **Node staking**: Required deposit to operate a node

### Anti-Speculation Measures

- Long vesting schedules for team/investor tokens
- Utility-focused token with real network usage requirements
- No ICO - tokens only earned through network participation
- Transfer lockup periods for large transactions

## Consequences

### Positive

- Creates sustainable incentive for node operators
- Enables permissionless network participation
- Aligns long-term incentives between Pied Piper and node operators
- Governance mechanism allows community-driven protocol evolution
- Proof-of-Resource more energy-efficient than Proof-of-Work

### Negative

- Regulatory uncertainty around token classification
- Token price volatility affects node operator economics
- Complex economic modeling required for sustainability
- Attracts speculators who don't care about the network
- Team token allocation creates perception of "pre-mine"
- **Ultimate negative**: PiperNet destruction meant all PPC became worthless

### Neutral

- Requires cryptocurrency exchange listings for liquidity
- Node operators must learn cryptocurrency custody
- Creates tax implications for token earnings
- Competes in crowded crypto token market

## Alternatives Considered

### Alternative 1: Fiat-Only Payments

**Description:** Users pay USD/EUR directly, node operators receive traditional payment.

**Pros:**
- No regulatory uncertainty
- Familiar payment model for enterprises
- Stable pricing

**Cons:**
- Requires payment processing infrastructure
- High transaction fees for micropayments
- No decentralized governance mechanism
- Requires Pied Piper to remain centralized payment processor

**Why not chosen:** Contradicts the decentralization goals of PiperNet. Would make Pied Piper a single point of failure.

### Alternative 2: Use Existing Cryptocurrency (ETH/BTC)

**Description:** Use Ethereum or Bitcoin as the network's currency instead of creating a new token.

**Pros:**
- Established liquidity and exchanges
- No token launch complexity
- Leverages existing ecosystem

**Cons:**
- No control over monetary policy
- Transaction fees on ETH/BTC are volatile and high
- Cannot implement custom economic incentives
- No governance token for protocol decisions

**Why not chosen:** PiperNet requires custom economic mechanisms (Proof-of-Resource, slashing) that cannot be implemented on existing chains without a native token.

## References

- Filecoin tokenomics whitepaper (comparative analysis)
- SEC Framework for "Investment Contract" Analysis of Digital Assets
- Bitcoin halving schedule analysis
- Ethereum 2.0 staking economics

## Notes

- Token generation event (TGE) was scheduled for January 2019
- Legal counsel from Perkins Coie confirmed utility token classification
- Gilfoyle implemented the mining algorithm in Rust
- The 51% attack (INC-003) exposed vulnerabilities in the staking mechanism
- **Final note**: All PPC was intentionally made worthless when PiperNet was destroyed (ODR-005). The encryption paradox discovery (INC-004) made this necessary to prevent the technology from enabling untraceable criminal activity.
