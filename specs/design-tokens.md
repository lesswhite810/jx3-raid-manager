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

## 卡片与交互样式规范

随着应用的视觉优化全面走向扁平极简，请在开发与重构组件时严格遵守卡片、按钮的外观规范：

- **卡片/容器**：使用单色无渐变的柔和背景色，例如默认区域使用 `bg-slate-50`，状态区域使用 `bg-emerald-50` 或 `bg-amber-50`。禁用任何悬浮带来的发光、强阴影(`shadow-lg`, `shadow-md` 等)效果，并舍弃线性渐变底色。
- **状态筛选/多选 Tag**：多标签切换（比如团队副本、百战、试炼的复选框状态）或选择器中，其激活选中态颜色应用统一的 `emerald` 方案 (`bg-emerald-50 text-emerald-700`)，未选中为低对比度 `bg-base`。不在不同维度间创造花哨或跳跃的高对比颜色系统。
- **图标克制**：不在信息已经非常明确的选择器文本之前附加不必要的装饰图标（比如“眼睛”图标），追求内容本体的直观和清爽。

---

## 语义颜色速查

| 用途 | 色系 | 示例 |
|------|------|------|
| 收入/正值/可用/通用激活态 | `emerald` | 总收入、可打状态、通用 Tab 激活项 |
| 支出/警示/已用 | `amber` | 总支出、已打状态、玄晶标签 |
| 错误/删除 | `red` | 删除按钮、错误提示 |
| 主操作 | `primary` | 提交按钮 |
| 信息 | `blue` | 装备分数、翻牌序号 |
| 百战/独立模块辅助 | 尽量并入 `emerald` 或扁平主题 | 取消渐变，保持 `bg-slate-50` 或统一激活浅绿系 |
| 试炼/独立模块辅助 | 尽量并入 `emerald` 或扁平主题 | 取消阴影，维持平淡边框样式以服从主干 |
