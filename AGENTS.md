# Shabdar: agent instructions

## Project
Persian RTL classifieds marketplace. Launch focus: Rasht. Goal: national coverage later.
Stack: Rust + Axum (API), Nuxt (SSR frontend), MariaDB (native, no Docker).

## Skills
Before starting any task, find and read every SKILL.md under .agents/skills/
(run: find .agents/skills -name SKILL.md). Follow the instructions in each one
that applies to the task. At the start of the session, list the skills you loaded.
If a skill cannot be read, say so before continuing.

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
