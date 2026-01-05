---
name: opportunity
description: Create business opportunity documents with thorough questioning
---
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

**In interactive sessions**: Ask questions and get answers to critical questions before creating the document.

**In batch mode**: Proceed with your best understanding and mark unknowns with FIXMEs.

```bash
dg new opp "Selling lama milk online"
```

Good title: "Selling lama milk online"
Bad title: "Build e-commerce app"

### Step 3: Fill in the document

Edit the created document to add:
- **Outcome**: The metric and target (use FIXME if unknown)
- **Opportunity**: Problem statement and evidence
- **Solutions**: Multiple approaches (don't commit to one)
- **RICE Score**: If enough info; otherwise add FIXME
- **Strategic Alignment**: Which competitive moats this builds

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
