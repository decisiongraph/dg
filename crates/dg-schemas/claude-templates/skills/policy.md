# Create Policy Document

Capture an organizational or business policy — regulatory constraints, compliance requirements, operational rules.

## When to Use

- Regulatory/legal requirements (GDPR, PCI-DSS, SOX)
- Compliance constraints
- Business rules or operational policies
- Security policies
- Governance decisions

## Workflow

### Step 1: List clarifying questions in your response

Before creating the document, write out questions about:
- **Driver**: What regulation, law, or business need requires this policy?
- **Scope**: Which teams/systems are affected? Are there exceptions?
- **Enforcement**: What's the enforcement mechanism? What happens on non-compliance?
- **Authority**: Do we have authority to set this policy? Is there an existing one this replaces?

### Step 2: Gather answers or proceed

**In interactive sessions**: Use `AskUserQuestion` to get answers to critical questions before creating the document.

**In batch mode**: Proceed with your best understanding and mark unknowns with FIXMEs.

```bash
dg new pol "Cold chain logistics for lama milk delivery"  # Add --field=value to set fields at creation
```

Good title: "Cold chain logistics for lama milk delivery"
Bad title: "Shipping stuff"

### Step 3: Fill in the document

Edit the created document to add:
- **Purpose**: Why this policy exists, what regulation/need drives it
- **Scope**: Who and what it applies to
- **Background**: Context, regulatory references, prior state
- **Policy Statement**: The actual rules/constraints (clear, actionable)
- **Implementation**: How to comply, what changes are needed
- **Compliance**: How compliance is measured and enforced

### Step 4: Link and mark unknowns

- Link to related OPP/ADR documents using frontmatter relation fields
- Add FIXMEs for gaps:
  ```markdown
  <!-- FIXME: Need legal review of data retention periods -->
  <!-- FIXME: Confirm DPO appointment with HR -->
  ```

## Tips

- Title POLs by what they govern, not why they exist
- Status: `draft` -> `active` -> `superseded`
- Always reference the specific regulation/law driving the policy
- Link ADRs that implement this policy using `implemented_by:`
- Use `conflicts_with:` when a decision contradicts this policy
