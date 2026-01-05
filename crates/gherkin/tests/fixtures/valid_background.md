# Shopping Cart

```gherkin
Feature: Shopping Cart

  Background:
    Given the user is logged in
    And the cart is empty

  Scenario: Add item
    Given the user views a product
    When the user clicks add to cart
    Then the item appears in the cart

  Scenario: Remove item
    Given the cart has one item
    When the user removes the item
    Then the cart is empty
```
