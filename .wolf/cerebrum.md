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
- [2026-11-08] sandbox-policies ceilings: Security signed the currently applied digest (Flux REVISION column) instead of the new one. Always hand back sign-ceilings.sh with the full digest from the publish job summary or from Flux's "no matching signatures" message, and say not to copy the REVISION column.
- [2026-11-21] One changelog only: `.claude/CHANGELOG.md`. The root `CHANGELOG.md` was removed at the maintainer's request; never recreate it or add a Keep a Changelog file. Release notes are written per GitHub release from the `.claude/CHANGELOG.md` entries.
- [2026-11-07] (obsolete since 2026-11-21, the file is gone) CHANGELOG.md: searching for "### Added" after "## Unreleased" found the released [0.3.6] section's heading, so entries landed in a shipped release. Bound the search to the Unreleased section (up to the next "## ["), and create the heading there when it is missing.
- [2026-11-07] ROADMAPS.md is pulled into the docs site: link docs and ADRs from it with absolute URLs (GitHub or the Pages site), never repo-relative paths, or `make docs --strict` fails.

## Decision Log

<!-- Significant technical decisions with rationale. Why X was chosen over Y. -->
- [2026-11-22] `git commit -S` from the agent shell can fail with "gpg: signing failed: Timeout" when the key needs pinentry. Stage the files and hand Erick the exact commit command (or a script) instead of retrying.
