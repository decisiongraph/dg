# Create Opportunity Document

Capture a business opportunity using the Opportunity Solution Tree framework.

## When to Use

- New feature requests
- Market opportunities identified
- Product ideas
- User pain points discovered
- Business expansion plans

## Workflow

### Step 1: List clarifying questions in your response

Before creating the document, write out questions about:
- **Outcome**: What business outcome are we trying to achieve? What metric improves?
- **Problem**: What specific problem or pain point does this address? How do we know it's real?
- **Audience**: Who benefits and how? Is this a learning project or real business opportunity?
- **Assumptions**: What makes us think we can do this better than existing solutions?

### Step 2: Gather answers or proceed

**In interactive sessions**: Use `AskUserQuestion` to get answers to critical questions before creating the document.

**In batch mode**: Proceed with your best understanding and mark unknowns with FIXMEs.

```bash
dg new opp "Selling lama milk online"    # Add --field=value to set fields at creation
```

Good title: "Selling lama milk online"
Bad title: "Build e-commerce app"

### Step 3: Fill in the document

Edit the created document to add:
- **Description**: Problem statement and evidence (required)
- **Impact**: Expected business impact with revenue/retention/competitive data
- **Success Metrics**: Measurable KPIs — how do we know this delivered value?
- **Non-Goals**: What is explicitly out of scope to prevent scope creep
- **Alternatives Considered**: Table of options evaluated (❓→✅/🚫 / Option / Rationale)
- **Risks**: Known risks and mitigations
- **Requirements**: Technical and business requirements (becomes a tracked table with Status/Requirement/Owner when status is `pursuing` or `completed` — run `dg format` to auto-convert)

### Step 4: Mark unknowns with FIXMEs

```markdown
## Evidence
<!-- FIXME: Need user research data to validate this problem -->

## RICE Score
<!-- FIXME: Need customer data to estimate reach and impact -->
```

## Tips

- Title OPPs by the problem/opportunity, not the solution
- Status: `proposed` -> `validated` -> `in-progress` -> `done`
- FIXMEs are OK — they track what questions need answers later
- Link ADRs that realize this opportunity using `implemented_by:` in OPP or `implements:` in ADR
- Use `depends_on:` when this opportunity is blocked by another
- Use `conflicts_with:` when this opportunity creates tension with a policy
