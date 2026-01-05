# Auth Module

## Login

```gherkin
Feature: Login

  Scenario: Login success
    Given user on login page
    When user enters credentials
    Then user sees dashboard
```

## Logout

```gherkin
Feature: Logout

  Scenario: Logout success
    Given user is logged in
    When user clicks logout
    Then user sees login page
```
