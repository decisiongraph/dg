# E-Commerce Platform

```gherkin
Feature: E-Commerce

  Scenario: Browse products
    Given the user is on the homepage
    When the user clicks on a category
    Then products in that category are shown

  Scenario: Search products
    Given the user is on the homepage
    When the user types in the search bar
    And the user presses enter
    Then matching products are displayed

  Scenario: Add to cart
    Given the user views a product
    When the user clicks add to cart
    Then the product is added to the cart
    And the cart count increases

  Scenario: Checkout
    Given the user has items in the cart
    When the user proceeds to checkout
    And the user enters shipping info
    And the user enters payment info
    Then the order is confirmed

  Scenario: Order tracking
    Given the user has placed an order
    When the user views order status
    Then the current status is displayed
```
