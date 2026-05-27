name: Feature Request
description: Suggest an idea for this project
title: '[Feature] '
labels: ['enhancement']
assignees: []
body:
  - type: markdown
    attributes:
      value: |
        ## Summary
        Briefly describe the feature you'd like to request.

        ## Motivation
        Explain the use case. What problem does it solve? Who would benefit from this?

        ## Proposed Solution
        Describe what you want to happen. Include any technical constraints if known.

        ## Alternatives Considered
        Describe any alternative solutions you've considered.

        ## Additional Context
        Add any other context, mockups, or references about the feature request here.
        
  - type: dropdown
    id: priority
    attributes:
      label: Priority
      description: How important is this feature?
      options:
        - Low
        - Medium
        - High
        - Critical
    validations:
      required: true

  - type: dropdown
    id: component
    attributes:
      label: Component
      description: Which component is this for?
      options:
        - Core
        - Frontend
        - Backend
        - API
        - Documentation
        - CI/CD
        - Testing
        - Other
    validations:
      required: false

  - type: input
    id: related
    attributes:
      label: Related Issues
      description: Link any related issues (e.g., #123)
      placeholder: '#123, #456'
    validations:
      required: false

  - type: textarea
    id: implementation
    attributes:
      label: Implementation Ideas
      description: Any thoughts on how this could be implemented
      placeholder: |
        1. Option A: ...
        2. Option B: ...
    validations:
      required: false