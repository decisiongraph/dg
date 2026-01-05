---
name: adr
description: Create architecture decision records using MADR 4.0 format
---
# Create Architecture Decision Record

Capture a technical architecture decision using MADR 4.0 format.

## When to Use

- Technical architecture choices (database, framework, protocol)
- Infrastructure decisions (hosting, CI/CD, deployment)
- Code organization choices (monorepo vs multi-repo, module structure)
- API design decisions

## Workflow

1. **Check existing context**:
   ```bash
   dg list
   dg validate
   ```

2. **Link to OPP when applicable**: If this ADR implements a business opportunity, reference it.

3. **Create the ADR**:
   ```bash
   dg new adr "Use Ruby on Rails with PostgreSQL"
   ```

4. **Fill in the document**:
   - **Context and Problem Statement**: What is the issue?
   - **Decision Drivers**: What factors matter?
   - **Considered Options**: At least 2-3 alternatives
   - **Decision Outcome**: Which option and why
   - **Consequences**: Good, bad, and neutral

## Status Lifecycle

- `proposed` — Under discussion, not yet decided
- `accepted` — Decision is final and active
- `deprecated` — No longer relevant
- `superseded` — Replaced by a newer ADR (link to it)

## Tips

- Good title: "Use Ruby on Rails with PostgreSQL"
- Bad title: "Tech stack decision"
- Always document rejected alternatives — they prevent revisiting the same discussion
- Link to OPPs this decision realizes using `implements:` field
- Link to POLs that constrain this decision using `implements:` or `conflicts_with:` fields
- Use `depends_on:` for decisions that block or are blocked by other docs
- Use `<!-- FIXME: ... -->` for information gaps that need follow-up
