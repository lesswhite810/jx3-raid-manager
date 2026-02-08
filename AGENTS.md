# AGENTS.md

This document provides guidelines for AI agents working on the JX3 Raid Manager codebase.

## Build & Development Commands

```bash
# Development
npm run dev          # Start Vite dev server (port 1420)
npm run build        # Type check + production build
npm run preview      # Preview production build

# Tauri (desktop app wrapper)
npm run tauri [command]  # Run Tauri CLI commands
```

**No dedicated test framework** - add tests using Vitest if needed. Run single test with `vitest run <file>`.

## Code Style Guidelines

### TypeScript
- Strict mode enabled in `tsconfig.json`
- **Never** suppress errors with `// @ts-ignore`, `as any`, or `@ts-expect-error`
- Use interfaces for object shapes, enums for fixed sets of values
- Enable `noUnusedLocals` and `noUnusedParameters` - fix all warnings

### React Patterns
- Use functional components with hooks (`useState`, `useEffect`, `useCallback`, `useMemo`)
- Export components as named exports: `export const ComponentName: React.FC<Props> = (...)`
- Use `Array.isArray()` guards before array operations
- Destructure props with explicit typing

### File Organization
```
src/           - TypeScript declarations
components/    - React components
services/      - Backend/API logic (db.ts, gameDirectoryScanner.ts, etc.)
utils/         - Pure utility functions
hooks/         - Custom React hooks (index.ts exports barrel)
constants/     - Constants (enums, static data)
contexts/      - React contexts
data/          - Static data files
```

### Naming Conventions
- **Components**: PascalCase (`Dashboard.tsx`, `DualRangeSlider.tsx`)
- **Functions/Variables**: camelCase (`sortAccounts`, `filteredRecords`)
- **Constants/Enums**: UPPER_SNAKE_CASE or PascalCase enum values
- **Files**: Descriptive, kebab-case for utilities, PascalCase for components
- **IDs**: Use UUID strings for unique identifiers

### Styling (Tailwind CSS)
- Use CSS variables for theming: `rgb(var(--bg-base) / <alpha-value>)`
- Follow `design_system.md` for color tokens and spacing
- Dark mode support via `[data-theme="dark"]` selector
- Use `text-main`, `text-muted`, `bg-base`, `bg-surface`, `border-base` utility classes

### Imports & Exports
- Use barrel pattern in `hooks/index.ts`, `services/ai/index.ts`
- Group imports: React → external libs → internal components/utils → types
- Relative imports for internal code (`../types`, `./utils/accountUtils`)

### Error Handling
- Always wrap async operations in try-catch
- Log errors with `console.error('Failed to...:', error)`
- Re-throw critical errors or return graceful fallbacks
- Use Toast system (`utils/toastManager.ts`) for user-facing errors

### State Management
- Use React Context for global theme/auth state (`contexts/ThemeContext.tsx`)
- Services (db.ts) handle data persistence via Tauri backend
- Local component state via `useState`

### Git Workflow
- Commit messages in Chinese (project is Chinese-localized)
- Group related changes in single commits
- No force pushes to shared branches
