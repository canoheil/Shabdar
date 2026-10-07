# Shabdar: agent instructions

## Project
Persian RTL classifieds marketplace. Launch focus: Rasht. Goal: national coverage later.
Stack: Rust + Axum (API), Nuxt (SSR frontend), MariaDB (native, no Docker).

## Skills
Skills are in .agents/skills/. Each skill has a SKILL.md.
At the start of a session:
1. Read .agents/skills/using-superpowers/SKILL.md (this is the entry point for the Superpowers set).
2. Read .agents/skills/ponytail/SKILL.md and .agents/skills/ui-ux-pro-max/SKILL.md.
For any other task, read only the SKILL.md of the skill that matches the task
(for example: writing-plans before planning, test-driven-development before writing code,
systematic-debugging when fixing a bug, ui-styling or design for UI work).
Do not read every skill. In the first reply of each session, list the skills you loaded.
If a file cannot be read, say so before continuing.

## Docs (source of truth)
- docs/PRODUCT.md, docs/DATABASE.md, docs/API.md, docs/ARCHITECTURE.md
- docs/STATUS.md: current phase and next step. Read it first.

## Rules
- Do not write application code until the user approves the current phase.
- Use sqlx runtime queries only. No compile-time query macros.
- Listings start as `pending` and are published only after admin approval.
- Comments are published immediately in v1. Users can report harmful comments, and reported comments are hidden for admin review.
- Automated checks only flag content for review. They never approve or reject on their own.
- Keep dependencies minimal. Check Termux compile cost before adding any dependency.
- Write simple, commented Rust. Avoid advanced lifetimes, generics, and macros.
- Never commit secrets, .env files, or real phone numbers.

## Git
- Commit and push to a feature branch. The user opens and merges PRs from the GitHub UI.
- Update docs/STATUS.md at the end of every task.
