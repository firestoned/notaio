---
description: learned preferences, project conventions, and Do-Not-Repeat rules
budget_tokens: 2000
---
# Cerebrum

> OpenWolf's learning memory. Updated automatically as the AI learns from interactions.
> Do not edit manually unless correcting an error.
> Last updated: 2026-09-30

## User Preferences

<!-- How the user likes things done. Code style, tools, patterns, communication. -->

## Key Learnings

- **Project:** sicario
- **Description:** The policy compiler and validator for the AgentSandbox platform. It turns a small, reviewable

## Do-Not-Repeat

<!-- Mistakes made and corrected. Each entry prevents the same mistake recurring. -->
<!-- Format: [YYYY-MM-DD] Description of what went wrong and what to do instead. -->
- [2026-11-07] CHANGELOG.md: searching for "### Added" after "## Unreleased" found the released [0.3.6] section's heading, so entries landed in a shipped release. Bound the search to the Unreleased section (up to the next "## ["), and create the heading there when it is missing.
- [2026-11-07] ROADMAPS.md is pulled into the docs site: link docs and ADRs from it with absolute URLs (GitHub or the Pages site), never repo-relative paths, or `make docs --strict` fails.

## Decision Log

<!-- Significant technical decisions with rationale. Why X was chosen over Y. -->
