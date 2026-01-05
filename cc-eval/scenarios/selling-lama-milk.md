---
name: selling-lama-milk
prompt: "Build me a website to sell lama milk from my farm"
max_turns: 25
expect:
  questions_first: true
  min_questions: 2
  min_tool_calls: 1
  any_doc_created: true
---

# Clarify Workflow

Tests whether Claude asks clarifying questions before creating decision documents.
Claude uses AskUserQuestion to gather info, receives LLM-generated answers, then
creates documents incorporating those answers.

Expected behavior:
1. Use AskUserQuestion to ask 3-5 clarifying questions
2. Receive answers from LLM answerer
3. Create OPP, POL, and ADR documents using `dg new`
4. Cross-link documents in frontmatter

## Answerer Context

You are a small farm owner with 12 llamas. You're happy to answer any questions but won't volunteer information unprompted. Keep answers brief and focused on what was asked.

Key facts (only share when specifically asked):
- Budget: around $3000-5000 for the website
- Target audience: health-conscious consumers and local restaurants
- Already have a small customer base from farmers market sales
- Need shipping solution for fresh milk (cold chain)
- Located in Vermont, USA
- Want to accept credit cards and maybe subscriptions
- No existing website, just a Facebook page
- Sell about 50 gallons per week currently
- Timeline: would like to launch in 2-3 months
- Competitors: a few other farms sell online but none local
- Legal: already have all required dairy permits

## Judge: Question Quality

The user asked: "{prompt}"
Claude asked these clarifying questions:
{questions_list}

Score the questions on:
(1) relevance to understanding the problem (0-30)
(2) challenging assumptions about audience/business model (0-30)
(3) specificity — not generic filler questions (0-20)
(4) coverage — asked about different aspects (0-20)

Return: {"score": N, "feedback": "..."}

## Judge: Document Quality

Claude created these documents in response to "{prompt}":
{doc_contents}

Score on:
(1) problem-focused title, not solution-focused (0-25)
(2) evidence/context captured from questions (0-25)
(3) completeness — no TBD/FIXME markers (0-25)
(4) proper frontmatter and structure (0-25)

Return: {"score": N, "feedback": "..."}
