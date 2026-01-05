---
name: incident
description: Create blameless incident reports and post-mortems
---
# Create Incident Report

Capture a post-mortem / incident report using blameless format.

## When to Use

- Production outages
- Security incidents
- Data loss events
- Service degradation
- Near-misses worth documenting

## Workflow

### Step 1: Confirm and create

When a user reports an incident:
1. Ask: "This sounds important. Shall I create an incident report?"
2. After confirmation, create immediately:

```bash
dg new inc "Checkout page down for 2 hours"
```

Incidents should be documented quickly. Gather basic facts (timeline, impact) but don't ask extensive discovery questions.

Good titles: "Checkout page down for 2 hours", "Customer data leaked"
Bad title: "Things broke"

### Step 2: Fill in the timeline

As events unfold or are reported:
- When did it start?
- When was it detected?
- When was it mitigated?
- When was it resolved?

### Step 3: After resolution, complete

- **Impact**: Users affected, duration, revenue impact
- **Root Cause**: Use 5 Whys technique
  ```
  1. Why did the service go down? → Database connection pool exhausted
  2. Why was the pool exhausted? → Query N+1 problem under load
  3. Why wasn't this caught? → No connection pool monitoring
  4. Why no monitoring? → Monitoring setup not in deployment checklist
  5. Why not in checklist? → No formal deployment review process
  ```
- **What went well**: Detection, response, communication
- **What went poorly**: Gaps in monitoring, slow response, missing runbooks
- **Action items**: With owners and due dates

### Step 4: Link to related records

```bash
# Link to ADRs that need updating based on lessons learned
# Link to OPPs if this reveals a business opportunity (e.g., better monitoring product)
# Link to POLs if this reveals need for new policies
```

### Step 5: Create follow-up records

Incidents often reveal needs for:
- New ADRs (technical changes to prevent recurrence)
- New POLs (process/policy changes)
- Updated existing records

## Example

User: "The checkout page was down for 2 hours yesterday"

1. Create: `dg new inc "Checkout page down for 2 hours"`
2. Fill timeline, impact, root cause
3. Add action items with owners
4. Link to ADRs that need updating
5. Create new ADR if architectural changes needed

## Tips

- **Blameless**: Focus on systems and processes, not individuals
- Title INC by what happened and when
- Status: `open` -> `mitigated` -> `resolved`
- Action items should be specific and assigned
- Link to ADRs that need updating using `triggers:` field
- Use `related:` for loosely associated docs
