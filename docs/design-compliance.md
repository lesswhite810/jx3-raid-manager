# 设计规范合规扫描报告

> 自动生成 by `scripts/design-compliance-scan.mjs` · 2026-08-23
> 规则来源：`specs/design-tokens.md §4` + `§卡片与交互样式规范`

## 📊 统计

| 维度 | 值 |
|---|---|
| 扫描文件数 | 39 |
| 命中违规数 | 282 |
| 涉及组件数 | 24 |
| 干净组件数 | 15 |

## 📋 按规则分布

| 规则 | 命中 | 严重度 | 说明 | 修复建议 |
|---|---:|---|---|---|
| R1 | 0 | high | §4-4 阴影规则：禁用 shadow-md / shadow-lg / shadow-xl / shadow-2xl | 改用 shadow-ds-flat / shadow-ds-stack / shadow-ds-modal |
| R2 | 0 | high | §卡片 禁用 linear-gradient / radial-gradient | 用单色 + shadow-ds-* 实现视觉层级 |
| R3 | 0 | medium | §4-6 动效：duration > 400ms | 改用 duration-ds-fast / duration-ds-base / duration-ds-slow |
| R4 | 0 | medium | §4-6 动效：禁用弹性缓动 ease-bounce / ease-spring | 改用 ease-ds-ease 或默认 linear/ease-in-out |
| R5 | 0 | low | §4-7 文案：emoji 图标 (🎮⚔🩸⏰👁🏆💎💰🔍✨🐛📊) | 用 Lucide React 图标替代；保留清单见 AGENTS.md §5 |
| R6 | 0 | low | §4-7 文案：连续感叹号堆叠 (!!! 或 ！！！) | AGENTS.md §4.4 禁止感叹号堆叠 |
| R7 | 45 | info | §4-7 红色直接用作文字色（非 hover/focus）—— 实际合规场景：删除按钮 / 必填星号 / 错误提示 | 合规：删除按钮 + 必填星号 + 错误提示均使用 red 系；design-tokens.md 未细化必填星号颜色 |
| R7-info | 237 | info | §4-7 红色系总用量（含合规场景：删除按钮 / 错误提示 / 必填星号） | 以下分布仅供参考：red 系全部用于合规场景（删除按钮 hover、错误消息、必填星号、表单验证） |
| R8 | 0 | low | §4-5 字体：font-sans 显式调用（演示位应用 font-display / font-body） | 展示位改用 font-display / font-body |

## 📁 Top 10 违规组件

| 命中数 | 文件 |
|---:|---|
| 56 | `src/data/kungfuData.ts` |
| 30 | `components/MigrationStatus.tsx` |
| 21 | `components/AccountManager.tsx` |
| 16 | `components/PendingRecordsPanel.tsx` |
| 15 | `components/RoleRecordsModal.tsx` |
| 14 | `components/IncomeDetail.tsx` |
| 13 | `components/AddRoleModal.tsx` |
| 12 | `components/TrialRoleRecordsModal.tsx` |
| 11 | `components/BaizhanRoleRecordsModal.tsx` |
| 11 | `components/ErrorBoundary.tsx` |

## 📝 命中明细

### R7 · §4-7 红色直接用作文字色（非 hover/focus）—— 实际合规场景：删除按钮 / 必填星号 / 错误提示

| 文件 | 行 | 命中 | 上下文 |
|---|---:|---|---|
| `components/AccountManager.tsx` | 1148 | `text-red-600` | : 'bg-surface text-red-600 border border-red-200 hover:bg-red-50 hover:border-red-300 active:scale-[ |
| `components/AccountManager.tsx` | 1628 | `text-red-500` | 心法 <span className="text-red-500">*</span> |
| `components/AccountManager.tsx` | 1658 | `text-red-500` | <p className="text-xs text-red-500 mt-1">{roleFormErrors.equipmentScore}</p> |
| `components/AddAccountModal.tsx` | 84 | `text-red-600` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddAccountModal.tsx` | 92 | `text-red-500` | 账号名称 <span className="text-red-500">*</span> |
| `components/AddBaizhanRecordModal.tsx` | 216 | `text-red-600` | <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm f |
| `components/AddRecordModal.tsx` | 364 | `text-red-600` | <AlertCircle className="w-4 h-4 text-red-600 flex-shrink-0" /> |
| `components/AddRecordModal.tsx` | 365 | `text-red-700` | <p className="text-sm text-red-700 dark:text-red-400">{errorMessage}</p> |
| `components/AddRoleModal.tsx` | 98 | `text-red-600` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRoleModal.tsx` | 106 | `text-red-500` | 角色名称 <span className="text-red-500">*</span> |
| `components/AddRoleModal.tsx` | 127 | `text-red-500` | 大区 <span className="text-red-500">*</span> |
| `components/AddRoleModal.tsx` | 141 | `text-red-500` | 服务器 <span className="text-red-500">*</span> |
| `components/AddTrialRecordModal.tsx` | 678 | `text-red-600` | <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm f |
| `components/AddTrialRecordModal.tsx` | 1058 | `text-red-500` | className="col-span-full mb-2 flex items-center justify-center gap-2 py-2 rounded-lg border border-d |
| `components/BaizhanRoleRecordsModal.tsx` | 288 | `text-red-600` | <AlertCircle className="w-6 h-6 text-red-600 dark:text-red-400" /> |
| `components/ConfigManager.tsx` | 407 | `text-red-500` | {pathValid === false && <AlertTriangle className="w-4 h-4 text-red-500 flex-shrink-0" />} |
| `components/ConfigManager.tsx` | 434 | `text-red-500` | <div className="flex items-center gap-1.5 text-xs text-red-500"> |
| `components/Dashboard.tsx` | 372 | `text-rose-500` | <svg className="w-5 h-5 text-rose-500/80" fill="none" stroke="currentColor" viewBox="0 0 24 24"> |
| `components/Dashboard.tsx` | 392 | `text-rose-500` | <svg className="w-3.5 h-3.5 text-rose-500/70" fill="none" stroke="currentColor" viewBox="0 0 24 24"> |
| `components/ErrorBoundary.tsx` | 44 | `text-red-600` | <h2 className="text-2xl font-bold text-red-600 dark:text-red-400 mb-4">应用出错了</h2> |
| `components/ErrorBoundary.tsx` | 46 | `text-red-700` | <p className="text-red-700 dark:text-red-300 font-medium mb-2">错误信息:</p> |
| `components/ErrorBoundary.tsx` | 47 | `text-red-600` | <pre className="text-sm text-red-600 dark:text-red-400 overflow-auto max-h-40">{this.state.error?.to |
| `components/ImportRolesModal.tsx` | 340 | `text-red-600` | <div className="p-4 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/IncomeDetail.tsx` | 633 | `text-red-600` | <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 bg-red-50 dark:bg-red-900/20 text-re |
| `components/MigrationStatus.tsx` | 317 | `text-red-600` | <XCircle className="w-5 h-5 text-red-600 dark:text-red-400" /> |
| `components/MigrationStatus.tsx` | 415 | `text-red-600` | <XCircle className="w-5 h-5 text-red-600 dark:text-red-400" /> |
| `components/MigrationStatus.tsx` | 426 | `text-red-600` | <p className="text-red-600 dark:text-red-400 font-medium"> |
| `components/MigrationStatus.tsx` | 592 | `text-red-600` | <div className={`flex items-center gap-2 ${diagnostic.database.connectionOk ? 'text-ds-success dark: |
| `components/MigrationStatus.tsx` | 641 | `text-red-800` | <div className="bg-red-50 dark:bg-red-900/20 px-3 py-2 font-medium text-sm text-red-800 dark:text-re |
| `components/MigrationStatus.tsx` | 647 | `text-red-600` | <p key={index} className="text-sm text-red-600 dark:text-red-400"> |
| `components/PendingRecordsPanel.tsx` | 532 | `text-rose-500` | <Skull className="w-3.5 h-3.5 text-rose-500 flex-shrink-0" /> |
| `components/PendingRecordsPanel.tsx` | 536 | `text-rose-700` | className="px-1.5 py-0.5 rounded text-[11px] bg-rose-50 dark:bg-rose-900/20 text-rose-700 dark:text- |
| `components/PendingRecordsPanel.tsx` | 671 | `text-rose-500` | <Skull className="w-4 h-4 text-rose-500" /> |
| `components/RaidDetail.tsx` | 891 | `text-red-600` | : 'bg-red-50 text-red-600 hover:bg-red-100 dark:bg-red-900/20 dark:text-red-400' |
| `components/RaidManager.tsx` | 48 | `text-rose-700` | '挑战': 'bg-rose-50 text-rose-700 border-rose-200 dark:bg-rose-900/30 dark:text-rose-400 dark:border-r |
| `components/RoleRecordsModal.tsx` | 590 | `text-red-600` | <AlertCircle className="w-6 h-6 text-red-600 dark:text-red-400" /> |
| `components/TrialRoleRecordsModal.tsx` | 501 | `text-red-600` | <AlertCircle className="w-6 h-6 text-red-600 dark:text-red-400" /> |
| `src/data/kungfuData.ts` | 77 | `text-red-700` | "textColor": "text-red-700 dark:text-red-400", |
| `src/data/kungfuData.ts` | 86 | `text-red-700` | "textColor": "text-red-700 dark:text-red-400", |
| `src/data/kungfuData.ts` | 299 | `text-rose-700` | "textColor": "text-rose-700 dark:text-rose-400", |
| `src/data/kungfuData.ts` | 314 | `text-rose-700` | "textColor": "text-rose-700 dark:text-rose-400", |
| `src/data/kungfuData.ts` | 489 | `text-red-700` | "textColor": "text-red-700 dark:text-red-400", |
| `src/data/kungfuData.ts` | 495 | `text-red-700` | "textColor": "text-red-700 dark:text-red-400", |
| `src/data/kungfuData.ts` | 597 | `text-rose-700` | "textColor": "text-rose-700 dark:text-rose-400", |
| `src/data/kungfuData.ts` | 603 | `text-rose-700` | "textColor": "text-rose-700 dark:text-rose-400", |

### R7-info · §4-7 红色系总用量（含合规场景：删除按钮 / 错误提示 / 必填星号）

| 文件 | 行 | 命中 | 上下文 |
|---|---:|---|---|
| `components/AccountManager.tsx` | 1148 | `text-red-600` | : 'bg-surface text-red-600 border border-red-200 hover:bg-red-50 hover:border-red-300 active:scale-[ |
| `components/AccountManager.tsx` | 1148 | `border-red-200` | : 'bg-surface text-red-600 border border-red-200 hover:bg-red-50 hover:border-red-300 active:scale-[ |
| `components/AccountManager.tsx` | 1148 | `bg-red-50` | : 'bg-surface text-red-600 border border-red-200 hover:bg-red-50 hover:border-red-300 active:scale-[ |
| `components/AccountManager.tsx` | 1148 | `border-red-300` | : 'bg-surface text-red-600 border border-red-200 hover:bg-red-50 hover:border-red-300 active:scale-[ |
| `components/AccountManager.tsx` | 1347 | `text-red-600` | className="p-2 rounded-xl text-muted hover:text-red-600 hover:bg-red-50 active:scale-95 transition-a |
| `components/AccountManager.tsx` | 1347 | `bg-red-50` | className="p-2 rounded-xl text-muted hover:text-red-600 hover:bg-red-50 active:scale-95 transition-a |
| `components/AccountManager.tsx` | 1446 | `text-red-600` | className="p-1.5 rounded-lg text-muted hover:text-red-600 hover:bg-red-50 active:scale-95 transition |
| `components/AccountManager.tsx` | 1446 | `bg-red-50` | className="p-1.5 rounded-lg text-muted hover:text-red-600 hover:bg-red-50 active:scale-95 transition |
| `components/AccountManager.tsx` | 1523 | `bg-red-600` | className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white shadow-sm hover:shadow active:scale-[0.9 |
| `components/AccountManager.tsx` | 1523 | `bg-red-700` | className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white shadow-sm hover:shadow active:scale-[0.9 |
| `components/AccountManager.tsx` | 1562 | `bg-red-600` | className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white shadow-sm hover:shadow active:scale-[0.9 |
| `components/AccountManager.tsx` | 1562 | `bg-red-700` | className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white shadow-sm hover:shadow active:scale-[0.9 |
| `components/AccountManager.tsx` | 1601 | `bg-red-600` | className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white shadow-sm hover:shadow active:scale-[0.9 |
| `components/AccountManager.tsx` | 1601 | `bg-red-700` | className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white shadow-sm hover:shadow active:scale-[0.9 |
| `components/AccountManager.tsx` | 1628 | `text-red-500` | 心法 <span className="text-red-500">*</span> |
| `components/AccountManager.tsx` | 1651 | `border-red-300` | className={`w-full px-3 py-2 border rounded-lg bg-base text-main focus:ring-1 focus:ring-primary foc |
| `components/AccountManager.tsx` | 1651 | `bg-red-50` | className={`w-full px-3 py-2 border rounded-lg bg-base text-main focus:ring-1 focus:ring-primary foc |
| `components/AccountManager.tsx` | 1658 | `text-red-500` | <p className="text-xs text-red-500 mt-1">{roleFormErrors.equipmentScore}</p> |
| `components/AddAccountModal.tsx` | 84 | `bg-red-50` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddAccountModal.tsx` | 84 | `bg-red-900` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddAccountModal.tsx` | 84 | `border-red-200` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddAccountModal.tsx` | 84 | `border-red-800` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddAccountModal.tsx` | 84 | `text-red-600` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddAccountModal.tsx` | 84 | `text-red-400` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddAccountModal.tsx` | 92 | `text-red-500` | 账号名称 <span className="text-red-500">*</span> |
| `components/AddBaizhanRecordModal.tsx` | 216 | `bg-red-50` | <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm f |
| `components/AddBaizhanRecordModal.tsx` | 216 | `bg-red-900` | <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm f |
| `components/AddBaizhanRecordModal.tsx` | 216 | `text-red-600` | <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm f |
| `components/AddBaizhanRecordModal.tsx` | 216 | `text-red-400` | <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm f |
| `components/AddRecordModal.tsx` | 363 | `bg-red-50` | <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRecordModal.tsx` | 363 | `bg-red-900` | <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRecordModal.tsx` | 363 | `border-red-200` | <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRecordModal.tsx` | 363 | `border-red-800` | <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRecordModal.tsx` | 364 | `text-red-600` | <AlertCircle className="w-4 h-4 text-red-600 flex-shrink-0" /> |
| `components/AddRecordModal.tsx` | 365 | `text-red-700` | <p className="text-sm text-red-700 dark:text-red-400">{errorMessage}</p> |
| `components/AddRecordModal.tsx` | 365 | `text-red-400` | <p className="text-sm text-red-700 dark:text-red-400">{errorMessage}</p> |
| `components/AddRoleModal.tsx` | 98 | `bg-red-50` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRoleModal.tsx` | 98 | `bg-red-900` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRoleModal.tsx` | 98 | `border-red-200` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRoleModal.tsx` | 98 | `border-red-800` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRoleModal.tsx` | 98 | `text-red-600` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRoleModal.tsx` | 98 | `text-red-400` | <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-l |
| `components/AddRoleModal.tsx` | 106 | `text-red-500` | 角色名称 <span className="text-red-500">*</span> |
| `components/AddRoleModal.tsx` | 127 | `text-red-500` | 大区 <span className="text-red-500">*</span> |
| `components/AddRoleModal.tsx` | 141 | `text-red-500` | 服务器 <span className="text-red-500">*</span> |
| `components/AddTrialRecordModal.tsx` | 678 | `bg-red-50` | <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm f |
| `components/AddTrialRecordModal.tsx` | 678 | `bg-red-900` | <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm f |
| `components/AddTrialRecordModal.tsx` | 678 | `text-red-600` | <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm f |
| `components/AddTrialRecordModal.tsx` | 678 | `text-red-400` | <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm f |
| `components/AddTrialRecordModal.tsx` | 1058 | `border-red-300` | className="col-span-full mb-2 flex items-center justify-center gap-2 py-2 rounded-lg border border-d |

> 仅展示前 50 条 · 共 237 处

---

**重新扫描**：`node scripts/design-compliance-scan.mjs` · 输出会覆盖本文档
