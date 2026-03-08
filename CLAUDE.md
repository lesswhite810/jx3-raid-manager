# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# CLAUDE.md

This is the **Core Routing & Red-Line Guide** for Claude (claude.ai/code) when working with JX3 Raid Manager.

To avoid redundancy and maintain a Single Source of Truth (SSOT), detailed documentation is separated into specialized files. **Before writing any code or making assumptions, YOU MUST READ the appropriate spec file below:**

## 📚 Documentation Router

| Area | You should read... | Description |
|------|--------------------|-------------|
| **Business & Context** | 👉 `contexts/context.md` | Core project introduction, what is JX3 Raid Manager, main features, file structure, and core logic components. |
| **Code Style & Rules** | 👉 `AGENTS.md` | TypeScript/React best practices, testing commands, folder structures, and Tauri (Frontend-Backend) IPC conventions. |
| **UI & Design Tokens** | 👉 `specs/design-tokens.md` | **CRITICAL for UI work.** Contains the comprehensive flat & minimalist design principles, semantic colors (`emerald/amber`), container specs, and interaction colors. |

---

## 🛑 Strict Red Lines (MUST FOLLOW)

1. **Localization**: **All UI text MUST be in Simplified Chinese (简体中文).**
2. **Type Safety**: The project runs in strict TypeScript mode. **NEVER** use `as any`, `@ts-ignore`, or `@ts-expect-error` to suppress type errors.
3. **No Fabrication**: **NEVER** invent non-existent backend command names (`invoke('something_new')`) without writing the corresponding Rust function in `src-tauri/` first.
4. **Git Commits**: Commit messages must be in Chinese and follow the format: `类型: 描述` (e.g., `feat: 新增记录分类`, `fix: 修复显示错误`).
5. **Sync Logs**: When adding significant features, fixing bugs, or updating versions, always update the changelog in `README.md`.
6. **Release Notes**: When creating GitHub releases, follow this user-friendly format (NO technical code details):

   ```markdown
   ## ✨ 新增功能

   - **功能名称** - 简短描述功能对用户的价值

   ## ⚡ 性能优化

   - **优化项** - 用户可感知的改进描述

   ## 🐛 问题修复

   - 修复问题描述（用户视角）

   ---

   **完整更新日志**: https://github.com/lesswhite810/jx3-raid-manager/compare/v旧版本...v新版本
   ```
