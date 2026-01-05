---
title: JIRA Auto-linking Test
status: identified
author: alice
date: 2025-03-15
tags: [test, jira]
---

## Description

This document tests JIRA auto-linking with prefix CO.

Regular text should linkify CO-123 and CO-456.

Inline code should NOT linkify: `CO-789` or `ticket = "CO-111"`.

Code blocks should NOT linkify:

```python
def get_ticket():
    return "CO-222"
```

```bash
gh issue view CO-333
```

Mixed content: See CO-444 for details, but use `CO-555` in your code.

Multiple refs: CO-100, CO-200, and CO-300 are all linked.

Inside a code block (no linkification):
```
CO-400
CO-500
CO-600
```

But outside code blocks, these work: CO-700, CO-800, CO-900.

## Mathematical Notation

The ROI formula is $ROI = \frac{gain - cost}{cost} \times 100\%$.

Expected value calculation: $E[X] = \sum_{i=1}^{n} x_i \cdot p_i$

Display math for the cost-benefit analysis:

$$NPV = \sum_{t=0}^{T} \frac{C_t}{(1+r)^t}$$

Where $C_t$ is the cash flow at time $t$, $r$ is the discount rate, and $T$ is the total number of periods.

Risk-adjusted formula:

```math
\sigma = \sqrt{\frac{1}{N}\sum_{i=1}^{N}(x_i - \mu)^2}
```

Inline mixed with Jira: The variance for CO-123 is $\sigma^2 = 0.42$ and CO-456 has $\mu = 3.14$.

## Impact

Reference to CO-999 in regular paragraphs should be linked.
