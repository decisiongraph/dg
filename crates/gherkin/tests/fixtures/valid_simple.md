# User Authentication

As a registered user, I want to log in so I can access my account.

```gherkin
Feature: User Login

  Scenario: Successful login
    Given the user is on the login page
    When the user enters valid credentials
    Then the user is redirected to dashboard

  Scenario: Failed login
    Given the user is on the login page
    When the user enters invalid credentials
    Then an error message is shown
```
