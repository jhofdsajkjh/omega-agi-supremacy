name: Bug Report
description: Create a report to help us improve
title: '[Bug] '
labels: ['bug']
assignees: []
body:
  - type: markdown
    attributes:
      value: |
        ## Description
        Describe the bug here. Please be as detailed as possible.
        
        ## Steps to Reproduce
        1. Go to '...'
        2. Run '...'
        3. See error

        ## Expected Behavior
        What you expected to happen.

        ## Actual Behavior
        What actually happened.

        ## Environment
        - OS: [e.g., Ubuntu 22.04]
        - Version: [e.g., 1.0.0]
        - Rust version: [e.g., 1.70.0]
        
        ## Screenshots
        If applicable, add screenshots to help explain your problem.

        ## Logs
        ```
        Paste your logs here
        ```

        ## Additional Context
        Add any other context about the problem here.
        
  - type: textarea
    id: repro
    attributes:
      label: Reproduction Code
      description: A minimal code snippet that reproduces the bug
      placeholder: |
        fn main() {
            // Your code here
        }
    validations:
      required: false

  - type: input
    id: version
    attributes:
      label: Version
      description: Version where bug was found
      placeholder: e.g., 1.0.0
    validations:
      required: true

  - type: dropdown
    id: severity
    attributes:
      label: Severity
      description: How severe is this bug?
      options:
        - Low
        - Medium
        - High
        - Critical
        - Blocker
    validations:
      required: true