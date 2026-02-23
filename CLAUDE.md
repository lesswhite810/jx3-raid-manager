# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

JX3 Raid Manager (剑网三副本管家) - A localized raid data management desktop app for 《剑网三》(JX3) players.

| Layer | Stack |
|-------|-------|
| Frontend | React 18 + TypeScript + Vite + Tailwind CSS |
| Backend | Rust (Tauri v1.5) |
| Database | SQLite (managed by Rust) |
| Icons | Lucide React |

## Common Commands

```bash
# Install dependencies
npm install

# Development (frontend + Tauri backend)
npm run tauri dev

# Frontend-only dev (browser preview, no Tauri IPC)
npm run dev

# Production build
npm run tauri build

# Type check
npm run build   # runs tsc && vite build

# Run tests
npm run test          # vitest run
npm run test:watch    # vitest watch
```

## Architecture

### Frontend-Backend Communication
- **Pattern**: Request/response via Tauri `invoke`
- **Frontend wrapper**: `services/db.ts` encapsulates all Rust commands
- **Naming convention**:
  - Frontend methods: `camelCase` (e.g., `getAccounts`)
  - Rust commands: `snake_case` (e.g., `db_get_accounts`)

### Data Persistence
- **Primary storage**: Local SQLite database (managed by Rust in `src-tauri/src/db.rs`)
- **Data location**: `~/.jx3-raid-manager/jx3-raid-manager.db`
- **Frontend state**: `services/db.ts` for data access, `contexts/ThemeContext.tsx` for global state

### Key Files
- `App.tsx` - Root component with tab navigation and data loading
- `services/db.ts` - Database service layer, all Tauri IPC calls
- `src-tauri/src/main.rs` - Rust entry point, registers all Tauri commands
- `src-tauri/src/db.rs` - SQLite database operations
- `types.ts` - TypeScript type definitions
- `data/raidBosses.ts` - Static boss configuration data

## Code Style

### TypeScript
- Use `interface` for object types, `enum` for fixed value sets
- **Strict mode enabled**: Never use `as any`, `@ts-ignore`, or `@ts-expect-error`
- Array safety: Always check `Array.isArray()` before operations

### React Components
- Function components with named exports
- Use `useMemo` for computed values
- Props interface defined above component

### Naming Conventions
| Type | Convention | Example |
|------|------------|---------|
| Components | PascalCase | `Dashboard.tsx` |
| Hooks | camelCase + use prefix | `useCountdown` |
| Rust commands | snake_case | `db_get_accounts` |
| CSS variables | kebab-case | `--bg-surface` |

### Tailwind CSS
Use semantic color names, not raw values:
```tsx
// ✅ Correct
<div className="bg-base text-main border-border">
<div className="bg-surface text-muted">

// ❌ Avoid
<div className="bg-white text-slate-900">
```

## Localization

**All UI text must be in Simplified Chinese (简体中文).**

## Git Workflow

- Commit messages in Chinese
- Format: `类型: 描述`
- Examples: `feat: 新增副本收益统计图表`, `fix: 修复自动扫描路径错误`

## Important Rules

1. **Analyze before modifying**: Check both `services/db.ts` and corresponding Rust code in `src-tauri/`
2. **Safe refactoring**: When modifying `db.ts`, ensure the Rust command exists
3. **No fabrication**: Never invent non-existent Tauri commands
4. **Type safety**: Strict TypeScript, no type suppression

## Error Handling

Always use try-catch with toast feedback for async operations:
```typescript
try {
  await db.saveRecord(data);
  toast.success('保存成功');
} catch (error) {
  console.error('Failed to save record:', error);
  toast.error('保存失败，请重试');
}
```

## Release Process

Releases are triggered by pushing version tags (e.g., `v1.2.0`) which runs `.github/workflows/release.yml` to build MSI and NSIS installers.
