---
status: accepted
author: gilfoyle
date: 2018-01-15

tags:
  - architecture
  - decentralized
  - p2p
  - web3
supersedes: ADR-002
enables:
  - ADR-005
depends_on:
  - ADR-001
  - ODR-002
implements:
  - ODR-002
related:
  - INC-003
  - INC-004
  - ADR-003
---

# PiperNet Decentralized Architecture

## Context

After the COPPA incident destroyed PiperChat and the "Box" hardware pivot proved uninspiring, we needed a new direction. The fundamental problem with all previous architectures was centralization—single points of failure, regulatory targets, and data ownership concerns.

Richard proposed building a "New Internet" that embodies our Anti-Hooli principle at the protocol level.

## Decision

Build PiperNet: a fully decentralized peer-to-peer network that eliminates centralized cloud providers and ISPs.

## Architecture Overview

```d2
direction: down

mesh: PiperNet Mesh {
  phone1: Phone Node {
    shape: rectangle
    style.fill: "#4CAF50"
  }
  laptop: Laptop Node {
    shape: rectangle
    style.fill: "#2196F3"
  }
  iot: IoT Node {
    shape: rectangle
    style.fill: "#FF9800"
  }
  phone2: Phone Node {
    shape: rectangle
    style.fill: "#4CAF50"
  }

  phone1 <-> laptop: P2P
  laptop <-> iot: P2P
  iot <-> phone2: P2P
  phone2 <-> phone1: P2P
}

ledger: Distributed Ledger\n(Shard Registry) {
  shape: cylinder
  style.fill: "#9C27B0"
}

mesh -> ledger: Shard metadata
```

### Core Components

1. **P2P Mesh Network**
   - Data sharded and distributed across millions of devices
   - No central servers or data centers
   - Each node stores encrypted fragments

2. **Distributed Ledger**
   - Tracks shard locations across the network
   - Consensus mechanism for data integrity
   - Immutable audit trail

3. **Middle-Out Compression Layer**
   - Reduces bandwidth overhead for shard transmission
   - Critical for unstable peer connections
   - Enables efficient IoT participation

4. **Incentive Layer (PiedPiperCoin)**
   - Users earn PPC for providing storage/bandwidth
   - Developers pay PPC to host applications
   - See ADR-005 for tokenomics details

## Technical Specifications

### Data Sharding

```python
def shard_data(data: bytes, redundancy: int = 3) -> List[Shard]:
    # Compress with Middle-Out
    compressed = piper_compress(data)

    # Split into encrypted shards
    shards = []
    for i in range(0, len(compressed), SHARD_SIZE):
        shard = Shard(
            data=encrypt(compressed[i:i+SHARD_SIZE]),
            hash=sha256(compressed[i:i+SHARD_SIZE])
        )
        shards.append(shard)

    # Replicate for redundancy
    return replicate_shards(shards, redundancy)
```

### Node Requirements

| Device Type | Min Storage | Min Bandwidth | Reward Multiplier |
|-------------|-------------|---------------|-------------------|
| Smartphone | 1 GB | 1 Mbps | 1x |
| Laptop | 10 GB | 10 Mbps | 5x |
| Server | 100 GB | 100 Mbps | 20x |
| IoT Device | 100 MB | 100 Kbps | 0.1x |

## Consequences

### Positive
- No single point of failure
- Censorship resistant
- User-owned data
- Aligns with Anti-Hooli principle
- Regulatory arbitrage (no central entity to sue)

### Negative
- Complex consensus mechanisms
- Vulnerable to 51% attacks (see INC-003)
- Unpredictable node availability
- Higher latency than centralized solutions

### Risks Realized
- INC-003: 51% Attack attempt by Hooli
- INC-004: AI optimization led to encryption-breaking discovery

## Migration Path

1. Phase 1: Beta network with 10,000 nodes
2. Phase 2: Public launch with mobile app
3. Phase 3: Developer SDK release
4. Phase 4: Enterprise integration APIs
