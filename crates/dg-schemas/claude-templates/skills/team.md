# Fill in Team Documentation

Populate a team doc (`docs/teams/{id}.md`) with detailed content for all recommended sections.

## When to Use

- After `dg team add-team` creates a new team doc with placeholder sections
- When `dg suggest` flags missing or incomplete team doc sections
- When onboarding a new team into DecisionGraph

## Workflow

1. **Check existing teams**:
   ```bash
   dg team list
   ```

2. **Identify the team doc** to fill in — it should be at `docs/teams/{id}.md`

3. **Ask the user about each section**:
   - **Charter**: What does the team own? What are the boundaries?
   - **Communication**: What Slack channels, mailing lists, or forums does the team use?
   - **On-Call**: Is there an on-call rotation? What's the escalation path?
   - **Getting Started**: What does a new team member need to set up on day one?
   - **Processes**: How does the team handle PRs, deploys, RFCs?
   - **Key Contacts**: Who are the external stakeholders and partner teams?

4. **Fill in the sections** with concrete details — no TBD/FIXME markers

5. **Verify completeness**:
   ```bash
   dg suggest
   ```

## Section Guidelines

- **First paragraph** (before any heading): One-sentence team description — shown in listings and search
- **Charter**: 3-5 bullet points covering ownership areas and explicit non-responsibilities
- **Communication**: Markdown table with Channel + Purpose columns
- **On-Call**: Rotation cadence, escalation chain, link to runbooks
- **Getting Started**: Numbered list of setup steps a new hire follows on day one
- **Processes**: Brief description of PR review, deploy, and RFC workflows
- **Key Contacts**: External stakeholders with their role/relationship

## Quality Standards

- Every section should have real content, not placeholders
- Use concrete channel names, tool names, and URLs where possible
- Keep each section focused — if a section grows too large, link to external docs
