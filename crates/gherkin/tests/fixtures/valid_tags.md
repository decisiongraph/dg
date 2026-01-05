# Tagged Scenarios

```gherkin
@web
Feature: User Profile

  @smoke @critical
  Scenario: View profile
    Given the user is logged in
    When the user navigates to profile
    Then the profile page is displayed

  @regression
  Scenario: Edit profile
    Given the user is on the profile page
    When the user changes their name
    Then the name is updated
```
