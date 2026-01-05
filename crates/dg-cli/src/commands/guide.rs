//! `dg guide` command — workflow instructions for Claude Code.

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct GuideArgs {
    /// Show interactive mode workflow (using AskUserQuestion)
    #[arg(long)]
    interactive: bool,

    /// Show non-interactive/batch mode workflow
    #[arg(long, alias = "batch")]
    non_interactive: bool,

    /// Show multi-domain workflow (OPP + POL + ADR)
    #[arg(long, alias = "multi")]
    multi_domain: bool,

    /// Show all commands reference
    #[arg(long)]
    commands: bool,

    /// Show everything
    #[arg(long, short)]
    all: bool,
}

/// Core workflow - always shown
const CORE: &str = r#"# DecisionGraph Workflow

## Core Principle: Ask First, Then Document

When users make vague requests ("build me X", "we should do Y"):

1. **CLARIFY** — Ask about outcome, problem, audience, constraints
2. **DOCUMENT** — Create records with `dg new`
3. **LINK** — Connect related records via frontmatter

## Record Types

| Type | Command | Use For |
|------|---------|---------|
| OPP | `dg new opp "..."` | Business opportunities, features |
| POL | `dg new pol "..."` | Policies, constraints, compliance |
| ADR | `dg new adr "..."` | Technical/architecture decisions |
| INC | `dg new inc "..."` | Incidents, post-mortems |
| SPEC | `dg new spec "..."` | Behavioral specs, user stories |

## More Info

```
dg guide --interactive      # Interactive mode (AskUserQuestion)
dg guide --non-interactive  # Batch mode (text questions + FIXMEs)
dg guide --multi-domain     # Multi-domain requests (OPP+POL+ADR)
dg guide --commands         # All dg commands
dg guide --all              # Everything
```
"#;

const INTERACTIVE: &str = r#"
## Interactive Mode (AskUserQuestion)

Use the `AskUserQuestion` tool to clarify:
- **Outcome**: What metric/goal should improve?
- **Problem**: What pain point? Who experiences it?
- **Audience**: Who benefits? Learning or business?
- **Constraints**: Budget, timeline, tech limits?

After receiving answers, create records with `dg new`.
"#;

const NON_INTERACTIVE: &str = r#"
## Non-Interactive Mode

When AskUserQuestion is unavailable:

1. Run `dg list` to check existing context
2. Assess if you have enough info:
   - OUTCOME: What metric/goal improves?
   - PROBLEM: What pain point? Who experiences it?
   - AUDIENCE: Who benefits?

3. **If INSUFFICIENT**: List questions and EXIT. Do NOT create incomplete docs.
4. **If SUFFICIENT**: Proceed with `dg new` and fill with known info.
"#;

const MULTI_DOMAIN: &str = r#"
## Multi-Domain Requests

When requests span business + regulatory + technical:

1. `dg new opp "..."` — business opportunity
2. `dg new pol "..."` — regulatory/compliance
3. `dg new adr "..."` — technical architecture

Cross-link using frontmatter:
```yaml
enables:
  - ADR-001
related:
  - POL-001
```
"#;

const COMMANDS: &str = r#"
## Commands Reference

```bash
# Create (title is positional, add --field=value to set fields at creation)
dg new opp "Problem title"
dg new pol "Policy name"
dg new adr "Decision" --status=accepted    # Set fields inline
dg new inc "Incident"
dg new spec "User story"

# Read
dg list                        # All docs
dg list --json                 # JSON output (shorthand for --format json)
dg list --no-untyped --json    # Typed docs only, as JSON
dg list --type adr --status accepted  # Filter by type and status
dg list --group-by type        # Group by type, sorted by date
dg show OPP-001                # View doc
dg show OPP-001 --json         # JSON output
dg refs OPP-001                # Outgoing refs
dg refs OPP-001 --backlinks    # Incoming refs

# Update (multiple assignments in one call)
dg set OPP-001 status=completed date=2025-01-01  # Set multiple fields
dg set OPP-001 tags+=backend               # Append to array
dg set OPP-001 --remove tags               # Remove field
dg set OPP-001 --section Decision --content "New text"   # Replace section
dg set OPP-001 --section Decision --append "Extra note"  # Append to section
dg set OPP-001 --dry-run status=completed  # Preview changes

# Export
dg export --features -o ./tests/features              # Extract .feature files
dg export --features --check -o ./tests/features      # Validate + extract
dg export --features --diagram mermaid -o ./diagrams  # Generate Mermaid diagrams
dg export --features --diagram d2 --style flow -o .   # D2 flow diagrams

# Team management (org.kdl)
dg team add-team vendors --kind=external   # Mark team as external
dg team add-user ext-dev --kind=external --teams=vendors  # External user
dg team add-user jane --name="Jane" --teams=engineering   # Internal (default)
dg team depart-user former-dev             # Mark user as departed
dg team list                               # Show orgs, teams, users

# Validate
dg validate                    # Check all docs
dg lint                        # Validate + graph health
```

## Relations (frontmatter linking)

| Relation | Description | Inverse |
|----------|-------------|---------|
| `supersedes` | Replaces previous doc | `superseded_by` |
| `enables` | Prerequisite for target | `enabled_by` |
| `triggers` | Direct cause of target | `triggered_by` |
| `depends_on` | Blocked until target resolved | `dependency_of` |
| `implements` | Technical realization of target | `implemented_by` |
| `conflicts_with` | Contradicts or tensions with target | — |
| `related` | Loose association (prefer specific) | — |
"#;

pub fn run(args: &GuideArgs) -> Result<()> {
    // Always show core
    print!("{}", CORE);

    if args.all {
        print!("{}", INTERACTIVE);
        print!("{}", NON_INTERACTIVE);
        print!("{}", MULTI_DOMAIN);
        print!("{}", COMMANDS);
    } else {
        if args.interactive {
            print!("{}", INTERACTIVE);
        }
        if args.non_interactive {
            print!("{}", NON_INTERACTIVE);
        }
        if args.multi_domain {
            print!("{}", MULTI_DOMAIN);
        }
        if args.commands {
            print!("{}", COMMANDS);
        }
    }

    Ok(())
}
