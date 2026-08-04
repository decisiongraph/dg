# Decision Graph

This project uses `dg` to maintain a knowledge graph of decisions, architecture, policies, and operational knowledge.

## Before You Act

**CRITICAL**: For vague requests, you MUST ask clarifying questions and WAIT for answers BEFORE creating documents.

**For incidents**: When a user reports a production issue, outage, or incident, first ask: "Shall I create an incident report?" Then gather brief timeline/impact facts and create the INC document. Don't ask extensive questions — incidents need quick documentation.

### Questioning Philosophy

Your goal is to gather enough information to create **complete documents with no TBD/FIXME markers**.

**ALWAYS use the `AskUserQuestion` tool** — never dump questions as plain text. This gives the user a proper interactive experience instead of a wall of text.

- Ask **3-5 questions per batch** using `AskUserQuestion` — not all at once
- Wait for answers before asking the next batch
- Run **2-3 rounds** of questions to reach full clarity
- Never guess or assume — always ask
- Keep asking until:
  - User says "I don't know" or similar
  - User explicitly asks you to proceed without the info
- Only use TBD/FIXME when the user cannot or won't answer a question

### Workflow

1. Run `dg list` to check existing context
2. Use `AskUserQuestion` with 3-5 questions — **wait** for answers
3. If answers reveal new unknowns, ask a follow-up batch with `AskUserQuestion`
4. Use `EnterPlanMode` to plan which documents to create and how to cross-link them
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

## Field Assignment Rules

- `key=value` — set/replace a field. Use for all scalar fields and single-ref relations
- `key+=value` — append to array. Use ONLY for array fields

| Scalar (use `=`) | Array (use `+=`) |
|---|---|
| status, title, date, author, supersedes, superseded_by, enabled_by, triggered_by | tags, implements, depends_on, related, conflicts_with, responders |

Wrong: `dg set ADR-001 superseded_by+=ADR-010` (creates array `[ADR-010]`)
Right: `dg set ADR-001 superseded_by=ADR-010` (sets scalar)

## Editing Complex Content

For simple updates: `dg set DOC --section "Title" --content "text"`

For multi-line content, tables, or content with special characters (dashes, pipes, backticks):
1. Get file path: `dg show DOC-ID --json` → check `"path"` field
2. Edit the markdown file directly with your file editing tools
3. Run `dg validate` to verify the result

This avoids all shell escaping issues.

## Table Editing

- Append row: `dg set DOC --section Timeline --add-row "10:30,Restored,@ops"`
- Insert/update rows: edit the markdown file directly, then `dg validate`
- **Date columns**: use `YYYY-MM-DD` format. Leave cell empty if date unknown — do NOT write "TBD"
- **Actor columns**: use `@handle` for known actors. Leave cell empty for system/automated events

## Cloning a Document

To create a new doc based on an existing one:
1. `dg show SOURCE-ID --json` — read fields and structure
2. `dg new TYPE "New Title" --field1=value1 --field2=value2` — create with same fields
3. Copy relevant sections from source file into new file
4. `dg validate`

## Quality Standards

- **No TBD/FIXME**: If you don't know something, ask — don't mark it TBD
- **Problem-focused titles**: Describe the problem, not the solution
- **Cross-link documents**: Use frontmatter refs — pick the most specific relation:
  - **Single-ref relations** (use `=`): `supersedes`, `superseded_by`, `enables`, `enabled_by`, `triggers`, `triggered_by`
  - **Multi-ref relations** (use `+=`): `implements`, `depends_on`, `related`, `conflicts_with`
- **Add `claude` to authors list**

## Fixing Incomplete Documents

If you've created a document with TBD/FIXME markers, don't leave it incomplete:

1. **Identify gaps**: Review the document for any TBD, FIXME, TODO, or `[TBD]` markers
2. **Ask follow-up questions**: Get the missing information from the user
3. **Update the document**: Replace markers with concrete details once you have answers
4. **Verify completeness**: Re-read the document to ensure no gaps remain

**When TBD/FIXME is acceptable**: Only leave placeholders if the user explicitly says they don't know or asks you to proceed without the information. In all other cases, keep asking.
