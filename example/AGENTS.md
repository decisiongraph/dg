# Decision Graph

This project uses `dg` to maintain a knowledge graph of decisions, architecture, policies, and operational knowledge.

## Before You Act

**CRITICAL**: For vague requests, you MUST ask clarifying questions and WAIT for answers BEFORE creating documents.

**For incidents**: When a user reports a production issue, outage, or incident, first ask: "Shall I create an incident report?" Then gather brief timeline/impact facts and create the INC document. Don't ask extensive questions — incidents need quick documentation.

### Questioning Philosophy

Your goal is to gather enough information to create **complete documents with no TBD/FIXME markers**. Ask as many questions as needed — users prefer thorough questioning over incomplete documents.

- Ask **5-10 questions per round** until you have complete clarity
- It's OK to ask **20-30+ questions total** across multiple rounds
- Never guess or assume — always ask
- Keep asking until:
  - User says "I don't know" or similar
  - User explicitly asks you to continue/proceed without the info
- Only use TBD/FIXME when the user cannot or won't answer a question

### Workflow

1. Run `dg list` to check existing context
2. Ask clarifying questions and wait for answers
3. **WAIT** for the user's response — do NOT proceed without answers
4. If answers reveal new unknowns, ask follow-up questions
5. **Only when fully informed**: Create documents with `dg new`

### What to Ask About

For any request, ensure you understand:
- **Problem**: What problem are we solving? Why now?
- **Audience**: Who is affected? Who decides?
- **Constraints**: Budget, timeline, technical limitations?
- **Success criteria**: How do we know it worked?
- **Alternatives**: What options exist? What was rejected?
- **Dependencies**: What does this block or depend on?

Run `dg guide` for detailed workflow guidance.

## Creating & Updating Records

```bash
# Create — pass --field=value to set fields at creation (avoids separate dg set calls)
dg new opp "Problem-focused title"           # Business opportunity
dg new adr "Decision title" --status=accepted  # Set fields inline
dg new pol "Policy name"                     # Policy/constraint
dg new inc "Incident summary"                # Incident report
dg new spec "Spec title"                     # Behavioral specification

# Read
dg list [--type adr] [--status active]      # List documents
dg list --json --no-untyped                 # JSON output, typed docs only
dg list --group-by type                    # Group by type, sorted by date
dg show OPP-001                             # Display document
dg show OPP-001 --json                      # JSON output
dg refs OPP-001                             # Outgoing references
dg refs OPP-001 --backlinks                 # Incoming references

# Update — combine multiple assignments in one call
dg set OPP-001 status=completed date=2025-01-01  # Set multiple fields at once
dg set OPP-001 tags+=backend                # Append to array field
dg set OPP-001 --remove tags                # Remove field
dg set OPP-001 --section Decision --content "New text"  # Replace section
dg set OPP-001 --section Decision --append "Extra note" # Append to section

# Validate & lint
dg validate                                 # Schema validation
dg lint                                     # Validate + graph health checks
dg suggest                                  # Advisory improvement suggestions

# Team management (org.kdl)
dg team add-team vendors --kind=external   # External team (contractors, vendors)
dg team add-user ext-dev --kind=external --teams=vendors
dg team add-user jane --name="Jane" --teams=engineering   # Internal (default)
dg team depart-user former-dev             # Mark user as departed
dg team list                               # Show orgs, teams, users

# Maintenance
dg fmt                                      # Auto-format documents
dg renumber                                 # Reorder document IDs chronologically
```

Always use `dg new` and `dg set` — never create or edit markdown files manually.

## Quality Standards

- **No TBD/FIXME**: If you don't know something, ask — don't mark it TBD
- **Problem-focused titles**: Describe the problem, not the solution
- **Cross-link documents**: Use frontmatter refs — pick the most specific relation:
  - `supersedes` — replaces previous doc (inverse: `superseded_by`)
  - `enables` — prerequisite for target (inverse: `enabled_by`)
  - `triggers` — direct cause of target (inverse: `triggered_by`)
  - `depends_on` — blocked until target resolved (inverse: `dependency_of`)
  - `implements` — technical realization of target (inverse: `implemented_by`)
  - `conflicts_with` — contradicts or tensions with target
  - `related` — loose association (use a more specific relation when possible)
- **Add your name to authors list**

## Fixing Incomplete Documents

If you've created a document with TBD/FIXME markers, don't leave it incomplete:

1. **Identify gaps**: Review the document for any TBD, FIXME, TODO, or `[TBD]` markers
2. **Ask follow-up questions**: Get the missing information from the user
3. **Update the document**: Replace markers with concrete details once you have answers
4. **Verify completeness**: Re-read the document to ensure no gaps remain

**When TBD/FIXME is acceptable**: Only leave placeholders if the user explicitly says they don't know or asks you to proceed without the information. In all other cases, keep asking.
