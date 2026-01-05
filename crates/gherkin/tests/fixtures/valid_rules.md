# Payments

```gherkin
Feature: Payment Processing

  Rule: Credit card payments
    Scenario: Successful charge
      Given a valid credit card
      When the user submits payment
      Then the payment is processed

    Scenario: Declined card
      Given an expired credit card
      When the user submits payment
      Then the payment is declined

  Rule: Gift card payments
    Scenario: Full balance
      Given a gift card with sufficient balance
      When the user submits payment
      Then the payment is processed
```
