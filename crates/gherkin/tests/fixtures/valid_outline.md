# Search Feature

```gherkin
Feature: Search

  Scenario Outline: Search by category
    Given the user is on the search page
    When the user searches for "<term>"
    Then results for "<category>" are shown

    Examples:
      | term    | category    |
      | laptop  | Electronics |
      | shirt   | Clothing    |
```
