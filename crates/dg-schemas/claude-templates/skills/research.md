# Research and Propose Opportunities

When your task list is empty, switch to discovery mode to find new opportunities.

## When to Use

- All current tasks are completed
- User asks "what should we work on next?"
- After finishing a major milestone

## Workflow

1. **Review current state**: Run `dg list` to see all documents and their statuses
2. **Identify gaps**: Look for:
   - OPPs without implementing ADRs
   - ADRs without SPECs
   - SPECs without linked code
   - Stale documents (old dates, no recent updates)
3. **Research the domain**: Use web search to find:
   - Competitor features and approaches
   - Industry best practices
   - Emerging technologies relevant to the project
   - Common problems users face in this domain
4. **Propose opportunities**: For each idea:
   - Explain the problem/opportunity clearly
   - Estimate potential impact
   - Note any risks or dependencies
5. **Ask the user**: Present 3-5 ideas ranked by potential impact
6. **Document accepted ideas**: Use `dg new opp "Title"` for approved opportunities

## Tips

- Focus on end-user value, not technical novelty
- Consider the project's existing tech stack and conventions
- Look for quick wins alongside longer-term opportunities
- Check if similar problems were already addressed (search existing docs)
- Use footnotes for external references: `[^1]` in text, `[^1]: [Title](URL) — context.` at file bottom. Never create a `## References` section.
