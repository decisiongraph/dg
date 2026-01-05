# Create Behavioral Specification

Capture a user story with Gherkin scenarios as a first-class decision graph document.

## When to Use

- Defining user-facing behavior before implementation
- Translating opportunities into testable acceptance criteria
- Documenting edge cases and business rules
- Aligning stakeholders on expected behavior

## Workflow

### Step 1: Clarify the behavior

Before creating a spec, ask:
1. **Who** is the user/persona? What role do they have?
2. **What** do they want to accomplish?
3. **Why** — what value does it deliver?
4. **Edge cases** — what happens when things go wrong?
5. **Constraints** — rate limits, permissions, data formats?

### Step 2: Create the spec

```bash
dg new spec "Lama milk checkout flow"
```

Good title: "Lama milk checkout flow"
Bad title: "Checkout feature"

### Step 3: Write the Story section

Use the standard user story format:

> As a [role], I want [capability] so that [benefit].

### Step 4: Write Gherkin scenarios

Put scenarios in fenced code blocks:

~~~markdown
```gherkin
Feature: Lama milk checkout flow

  Scenario: Successful purchase with cold shipping
    Given a customer with lama milk in their cart
    When the customer completes checkout
    Then the order is placed with cold chain shipping
    And a confirmation email is sent

  Scenario: Out of stock during checkout
    Given a customer with lama milk in their cart
    When the product goes out of stock before payment
    Then the checkout is halted
    And the customer sees an out-of-stock message
```
~~~

### Step 5: Link to related records

```bash
# Spec implements an opportunity
dg new spec "Lama milk checkout flow" implements OPP-001

# Spec depends on a policy
dg new spec "Lama milk checkout flow" depends_on POL-001
```

## Status Lifecycle

- `draft` — initial capture, scenarios being written
- `review` — ready for stakeholder review
- `approved` — signed off, ready for implementation
- `implemented` — behavior verified in code
- `deprecated` — no longer relevant

## Tips

- Link specs to OPPs with `implements:` to show which opportunity a spec realizes
- Link specs to POLs with `depends_on:` when behavior must comply with a policy
- Each scenario should test one behavior — keep them focused
- Use Background blocks for shared Given steps
- Add Acceptance Criteria section for non-Gherkin conditions (performance, accessibility)
