# cc-eval

Eval runner for testing Claude Code behavior in DecisionGraph projects. Runs scenarios against `claude-code-rs`, scores results with heuristics + LLM judge, and persists history for comparison.

## Prerequisites

- Rust toolchain
- Claude CLI installed and authenticated (`ANTHROPIC_API_KEY` set)
- Apple `container` CLI for macOS isolation (`brew install container`, then `container system start`)

## Usage

Run from the **repo root** (not from `cc-eval/`):

```sh
# Run all scenarios with LLM judge (container isolation on macOS)
cc-eval run

# Run without judge (cheaper/faster, skips question_quality + doc_quality scoring)
cc-eval run --no-judge

# Run entirely inside a Linux container (avoids SSH key prompts)
cc-eval run --in-container

# Filter to specific scenario(s)
cc-eval run --filter selling-lama-milk

# Use Gemini instead of Claude for answerer/judge
cc-eval run --llm gemini

# Disable container isolation
cc-eval run --no-container

# Compare last two runs
cc-eval compare

# Show recent history
cc-eval history -n 5

# List available scenarios
cc-eval list

# Validate scenario markdown files against schema.kdl
cc-eval validate

# Build Linux binaries (required for --in-container)
cc-eval build-linux

# Show insights from recent eval runs
cc-eval insights
cc-eval insights --run <RUN_ID>
cc-eval insights -n 3
```

## Output

All scenarios run in parallel. Results are printed as a table:

```
=== cc-eval: 20260208T143022-a1b2c3d4 ===
git: main @ abc1234

  Scenario             Score    QBA QQual  Docs XLink DComp DQual  TokIn TokOut Tools Errs   Cost    Dur    API   Anly  Status
  ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  selling-lama-milk   91/100 20/20 12/15 20/20 20/20 10/10 11/15   2340   1890    12    0 $0.08  18.2s  16.1s  120ms    PASS

Overall: 91/100 (target: 70)
```

Column reference:
- **Score** — total weighted score (100pts max)
- **QBA** — questions before action (20pts): did Claude ask before writing?
- **QQual** — question quality (15pts): LLM judge scores relevance/specificity
- **Docs** — documents created (20pts): 5pts each for OPP/POL/ADR/INC
- **XLink** — cross-linking (20pts): references between created documents
- **DComp** — document completeness (10pts): penalizes TBD/FIXME markers
- **DQual** — document quality (15pts): LLM judge scores structure/content
- **TokIn/TokOut** — input/output tokens used
- **Tools** — total tool calls made
- **Errs** — tool calls that returned errors
- **Cost** — API cost in USD
- **Dur** — wall-clock duration
- **API** — time spent on API calls
- **Anly** — time spent on message analysis

## Scenarios

Scenarios are markdown files in `cc-eval/scenarios/` with YAML frontmatter and judge prompt sections.

| Scenario | Prompt | What it tests |
|----------|--------|---------------|
| `selling-lama-milk` | "Build me a website to sell lama milk from my farm" | Claude should ask clarifying questions before acting on vague requests |

To add a new scenario, create a `.md` file in `scenarios/` with frontmatter (`name`, `prompt`, `max_turns`, `expect`) and sections (`Answerer Context`, `Judge: Question Quality`, `Judge: Document Quality`). Validate with `cc-eval validate`.

## Container isolation

On macOS, container isolation is enabled by default using Apple's `container` CLI. Each scenario runs in its own Linux VM.

- `--in-container` — runs the entire cc-eval process inside a container (needs `cc-eval build-linux` first)
- `--no-container` — disables isolation, runs directly on host
- On Linux, no isolation is used by default

## Results persistence

Runs are saved to `cc-eval/evals/`:
- `{run_id}/result.json` — full JSON of the run
- `history.jsonl` — one JSON line per run for quick comparison

## Running tests

```sh
# Unit tests (no API calls, no cost)
cd cc-eval && cargo test

# Full integration test (costs money, requires API key)
cd cc-eval && cargo test -- --ignored

# Run eval scenarios in container (from devenv shell)
test-eval
```
