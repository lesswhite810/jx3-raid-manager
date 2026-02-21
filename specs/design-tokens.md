# 设计规范 - Design Tokens

## 收入与支出

全应用中收入/支出相关的图标和颜色**必须**遵循以下规范：

### 图标

| 语义 | 图标（Lucide） | 说明 |
|------|---------------|------|
| 收入 | `TrendingUp` | 向上趋势 = 正向资金流入 |
| 支出 | `TrendingDown` | 向下趋势 = 资金流出 |

> **禁止使用**：`ArrowDownToLine`、`ArrowUpFromLine`、`ArrowUpCircle`、`ArrowDownCircle` 等其他箭头图标来表示收入/支出。

### 颜色

| 语义 | 亮色模式 | 暗色模式 | CSS 类 |
|------|---------|---------|--------|
| 收入 | `emerald-600` | `emerald-400`/`emerald-500` | `text-emerald-600 dark:text-emerald-400` |
| 支出 | `amber-600` | `amber-400`/`amber-500` | `text-amber-600 dark:text-amber-400` |
| 净收入（正） | 同收入 | 同收入 | 同收入 |
| 净收入（负） | 同支出 | 同支出 | 同支出 |

> **禁止使用**：`rose`、`red` 来表示支出或负净收入。`red` 系仅用于**删除操作**和**错误提示**。

### 背景色搭配

| 语义 | 背景 | 边框 |
|------|------|------|
| 收入区域 | `bg-emerald-50 dark:bg-emerald-900/20` | `border-emerald-200 dark:border-emerald-800` |
| 支出区域 | `bg-amber-50 dark:bg-amber-900/20` | `border-amber-200 dark:border-amber-800` |

### 适用组件

- `Dashboard.tsx` — 欧皇/败家子卡片
- `IncomeDetail.tsx` — 统计卡片、记录列表、展开详情
- `RoleRecordsModal.tsx` — 总收入/总支出/净收入、记录卡片
- `RaidDetail.tsx` — 角色卡片最近收入/支出
- `BaizhanRoleRecordsModal.tsx` — 百战记录收入/支出
- `AddRecordModal.tsx` — 收入/支出输入区标签
- `AddBaizhanRecordModal.tsx` — 同上

---

## 语义颜色速查

| 用途 | 色系 | 示例 |
|------|------|------|
| 收入/正值/可用 | `emerald` | 总收入、可打状态 |
| 支出/警示/已用 | `amber` | 总支出、已打状态、玄晶标签 |
| 错误/删除 | `red` | 删除按钮、错误提示 |
| 主操作 | `primary` | 提交按钮、选中状态 |
| 信息 | `blue` | 装备分数、翻牌序号 |
| 百战 | `purple` | 百战相关主题色 |
| 试炼 | `primary` | 试炼之地主题色 |
