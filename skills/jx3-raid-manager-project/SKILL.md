---
name: jx3-raid-manager-project
description: Use when working in the JX3 Raid Manager repository on React or Tauri changes, release or version updates, Chinese documentation edits, account-management features, or repo-specific verification and UI conventions.
---

# JX3 Raid Manager Project

## Overview

Repository-specific workflow for JX3 Raid Manager. Use it to stay aligned with this repo's React + Tauri architecture, Chinese UTF-8 rules, release process, and flat local-tool visual style.

## Start Here

- Read `contexts/context.md` for product scope and module boundaries.
- Read `specs/design-tokens.md` before touching UI, icon semantics, or income/expense colors.
- Read `docs/release-process.md` before changing versions, release notes, updater assets, or GitHub release text.
- Check `src-tauri/` before assuming a new `invoke` command or filesystem capability exists.

## Core Rules

- All user-facing UI text, toast text, and release notes must use simplified Chinese.
- Do not write Chinese files through raw PowerShell text commands. Prefer repository edits plus UTF-8 readback verification.
- User-visible feature or bug changes must update `README.md`.
- Release-facing changes must update the matching file under `release-notes/`.
- Release notes only use two sections: `新增需求` and `修复 bug`.
- Keep the established flat, local-tool visual style. Avoid heavy shadows, strong gradients, and decorative icons.
- Income uses `TrendingUp` with `emerald`; expense uses `TrendingDown` with `amber`. Reserve `red` for deletion and errors.
- Prefer semantic Tailwind classes such as `bg-surface`, `border-base`, and `text-muted`.
- When 1rem text is needed, prefer `text-[1rem]` because this repo has a `text-base` naming collision risk.

## Version And Release Checklist

- When bumping a release, keep `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml` on the same version.
- If `npm` rewrites the lockfile during a version bump, keep `package-lock.json` aligned too.
- Keep release note files in `release-notes/v<version>.md`.
- Use `npm run release:notes -- <tag> <notes-file>` instead of hand-editing GitHub release text with Chinese command-line arguments.

## Verification

- Frontend-only changes: run `npm test` and `npm run build`.
- Rust or Tauri backend changes: also run `cargo check` inside `src-tauri`.
- After editing Chinese docs, re-read them with UTF-8 tooling such as Node before claiming they are correct.

## High-Value Files

- `components/AccountManager.tsx`: account UI, drag sorting, account actions
- `services/db.ts`: frontend persistence interface
- `src-tauri/src/db.rs`: Rust-side persistence commands
- `utils/accountUtils.ts`: account sorting and drag helpers
- `types.ts`: shared TypeScript data contracts
