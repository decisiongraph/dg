# dg — DecisionGraph CLI

Structured decision documentation tool for software projects.

## Installation

```bash
cargo install --path crates/dg-cli
```

## Commands

### Project Setup

```bash
dg init                    # Initialize a dg project
dg init --with-claude      # Include Claude Code integration
```

### Document Management

```bash
dg new opp --title "..."   # Create a business opportunity
dg new pol --title "..."   # Create a policy/constraint
dg new adr --title "..."   # Create an architecture decision
dg new inc --title "..."   # Create an incident report
dg delete <id>              # Delete a document

dg list                    # List all documents
dg show <id>               # Show a document
dg refs <id>               # Show references from/to a document
```

### External Document Hooks

Executable `.dg/hooks/on_create`, `.dg/hooks/on_update`, and
`.dg/hooks/on_delete` scripts run after CLI document mutations. They receive
`<document-id> <event-type>` arguments and document JSON on stdin. Update
payloads contain `before`, `after`, and a structured `diff`. Hook failures
only emit warnings.

### Validation

```bash
dg validate                # Validate documents against schema
dg lint                    # Validate + check graph health
```

### Claude Code Integration

```bash
dg claude                  # Start Claude Code with dg workflow prompt
dg claude -p "message"     # Pass arguments to claude
dg guide                   # Show workflow guide
```

## dg claude

The `dg claude` command starts Claude Code with a system prompt that enforces the DecisionGraph workflow:

1. Ask clarifying questions before implementation
2. Create decision documents (OPP, POL, ADR) before code
3. Follow the workflow described in CLAUDE.md

This ensures Claude follows a decision-first approach when working on your project.

```bash
# Start interactive session with dg workflow
dg claude

# Pass a prompt directly
dg claude -p "We should add user authentication"
```
