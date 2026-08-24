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


---

## 设计系统提案 token 增量（2026-08-23 纳入）

> 本节是**新增**附录，与上文的 `bg-emerald-600 / bg-amber-600 / text-red-600` 等**已有** Tailwind 调色板**共存**而非替换。
> 引入动机：采用 Open Design 导出的设计系统快照（`tmp/design-zip/DESIGN.md · colors_and_type.css §3`），在不破坏既有 chrome 视觉的前提下暴露完整的设计 token。
> 迁移策略：新建组件 / 重构时优先采用本节 token；存量组件保留不动。

### 1. 提案 token 全表

> 真实值的唯一来源：`index.css :root` 块（光主题）+ `[data-theme="dark"]` 块（暗主题）。
> 命名约定：`--{role}` · 调色板以 `var(--xxx)` 在 `tailwind.config.js` 的 `extend.colors` 中二次暴露。
> Tailwind 工具类前缀 `ds-`，避免与默认调色板（`neutral-* / amber-* / sky-*` 等）冲突。

#### 1.1 表面层级（Surface）

| Token | 光 | 暗 | Tailwind 工具类示例 |
|---|---|---|---|
| `--bg` | `#faf7f2` 暖米杏 | `#15110d` | `bg-ds-bg` |
| `--surface-ds` | `#ffffff` | `#1f1a14` | `bg-ds-surface` |
| `--surface-2` | `#f3efe8` | `#2a241c` | `bg-ds-surface-2` |
| `--surface-3` | `#ede7dc` | `#352d22` | `bg-ds-surface-3` |
| `--border-ds` | `#e3dccc` | `#3a3127` | `border-ds-border` |
| `--border-strong` | `#d4c8b3` | `#4a3f31` | `border-ds-border-strong` |

#### 1.2 文本层级（Foreground）

| Token | 光 | 暗 | Tailwind 工具类示例 |
|---|---|---|---|
| `--fg` | `#1f1a14` 黑棕 | `#f3ede2` | `text-ds-fg` |
| `--fg-soft` | `#4a4339` | `#c9c0b0` | `text-ds-fg-soft` |
| `--fg-mute` | `#8a7f70` | `#8a7f70` | `text-ds-fg-mute` |
| `--fg-inverse` | `#ffffff` | `#15110d` | `text-ds-fg-inverse` |

#### 1.3 主强调（Accent · 沉香褐 · 2026-08-23 由暖橙 `#c96442` 降饱和）

| Token | 光 | 暗 | 备注 |
|---|---|---|---|
| `--accent` | `#a8633f` | `#c08566` | **单屏 ≤ 2 处**（主操作 + 1 状态点）；`bg-ds-accent` |
| `--accent-soft` | `#f3e8df` | `#2e1d14` | hover / 选中底；不计 ≤2 限制；`bg-ds-accent-soft` |
| `--accent-strong` | `#8a4f30` | `#d49b7c` | 按下态 / 焦点描边；`border-ds-accent-strong` |

**与现有 primary 的关系**：
- 存量 `bg-primary` (= `--primary-base` = `#4f46e5` 靛蓝) 保留继续可用，不删除。
- 提案 `--accent` 为沉香褐 `#a8633f`，**色相完全不同于** 现有 `--primary-base` 的靛蓝 `indigo-600`。
- 决策点（待团队评审）：
  - 选项 A：保留双轨 — 旧 chrome 用 `bg-primary`，新组件用 `bg-ds-accent`（推荐，几乎零迁移成本）
  - 选项 B：把 `--primary-base` 改为 `#a8633f`（破坏性变更，需同步修改所有 chrome / 主题切换逻辑）

#### 1.4 语义色（Semantic · 同步降饱和）

| Token | 光 | 暗 | Tailwind 工具类 | 语义 |
|---|---|---|---|---|
| `--success` | `#5d8770` 苔绿 | `#87b29a` | `bg-ds-success` / `text-ds-success` | 完成 / 入账 |
| `--success-soft` | `#e3ebe5` | `#1a2520` | `bg-ds-success-soft` | Tag 底 |
| `--warning` | `#a38246` 焦糖 | `#c9a06b` | `bg-ds-warning` / `text-ds-warning` | 待确认 / 提醒 |
| `--warning-soft` | `#f0e8d8` | `#2e261a` | `bg-ds-warning-soft` | Tag 底 |
| `--danger-ds` | `#a55a52` 砖红 | `#c87872` | `bg-ds-danger` / `text-ds-danger` | 失败 / 删除 |
| `--danger-soft` | `#ecdfdc` | `#2a1d1c` | `bg-ds-danger-soft` | Tag 底 |
| `--info` | `#5e7e94` 雾蓝 | `#8eaabd` | `bg-ds-info` / `text-ds-info` | 链接 / 信息 |
| `--info-soft` | `#dfe5ea` | `#1a2229` | `bg-ds-info-soft` | Tag 底 |
| `--neutral` | `#6a6258` | `#a89e8c` | `bg-ds-neutral` | 中性 Tag / 已读 |
| `--neutral-soft` | `#ebe6db` | `#2a2520` | `bg-ds-neutral-soft` | Tag 底 |

**与现有 emerald/amber/red 的关系**：
- 颜色 **同色系**（success-绿 / warning-橙 / danger-红），**具体 hex 不同**：
  - `emerald-600 = #059669` vs 提案 `--success = #5d8770`（更偏冷，更低饱和）
  - `amber-600 = #d97706` vs 提案 `--warning = #a38246`（更偏暖陶土，饱和度更低）
- 决策点（待团队评审）：
  - 选项 A：保留 `emerald-600 / amber-600`（已有 chrome 不变，新组件可选用提案 `--success` / `--warning`）
  - 选项 B：把 `--chart-income` 由 `5 150 105` 改为 `#5d8770` 的 RGB（破坏性变更，需同步修改 `chart-income` / `chart-expense` 引用方）

> **建议**：在收入/支出场景**继续沿用** `emerald-600 / amber-600`（既符合 `AGENTS.md §5.4` 又不破坏 `IncomeDetail` / `RoleRecordsModal` 等核心页面）。提案 token 优先用于 **新建** 的非收入/支出模块（如 KPI 卡、Toast、归档视图等）。

#### 1.4.1 派生 strong 系列（WCAG AA 4.5:1 小正文专用）

> 🆕 **v2.2.0（2026-08-23）** 新增 · 决策点 1 方案 C 落地

每个语义色提供 3 档梯度：

| 工具类后缀 | 派生值 | 对应 token | vs 白底对比度 | 用途 |
|---|---|---|---:|---|
| DEFAULT | 提案基调色 | `--{success,warning,info}` | 3.60 ~ 4.30:1 | 大字号（≥18px 或 ≥14px bold）· Tag 软底背景 |
| `soft` | 浅色背景 | `--{success,warning,info}-soft` | （背景色） | 浅底背景 / Tag 内底 |
| `strong` | 深色派生 | `--{success,warning,info}-strong` | **6.63 ~ 7.83:1** | ≤18px 正文 · 小标签 · 图标颜色 |

**色值**（光主题）：

```css
--success-strong: #3d5e4a;  /* 苔绿派生 */
--warning-strong: #6e5a30;  /* 焦糖派生 */
--info-strong:    #3e546a;  /* 雾蓝派生 */
```

**色值**（暗主题）：

```css
--success-strong: #87b29a;  /* 浅一档,与光主题对称 */
--warning-strong: #c9a06b;
--info-strong:    #8eaabd;
```

#### 1.4.2 何时用 strong 派生（决策树）

```
className 字号判断
├── text-base / text-lg / text-xl / text-2xl / text-3xl
│   └── 用 DEFAULT（大字号 AA ≥3.0:1 已合规）
├── font-bold + 默认 16px
│   └── 用 DEFAULT（14px bold 算大字号 AA 已合规）
├── text-xs / text-sm / text-[10px] / text-[12px]
│   └── 用 *-strong（小正文 AA 4.5:1 必需）
├── 透明度修饰 text-ds-{sem}/N
│   └── 替换为 text-ds-{sem}-strong（透明度会降低对比度）
├── 小图标 w-[1-4] h-[1-4]
│   └── 替换为 *-strong（图小色需更深补偿）
└── Tag/Pill 内文（bg-ds-{sem}-soft + text-ds-{sem}）
    └── 替换为 *-strong（软底上的文字色对比度不达 AA）
```

**全仓落地统计（2026-08-23）**：

- `text-ds-*-strong` 使用：142 行（小正文/标签/图标场景）
- DEFAULT 保留：72 行（大字号 / 大图标 / 数据文件）
- 设计哲学保持：提案基调色不变,仅补充深色派生


### 2. 几何 / 排版 token

#### 2.1 圆角（严格 2 档）

| Tailwind 工具类 | 值 | 用途 |
|---|---|---|
| `rounded-card` | `var(--r-md)` = 4px | 卡片 / 输入框 / Toast / 弹窗 |
| `rounded-tag` | `var(--r-sm)` = 2px | Tag / Chip / 按钮 |

**保留**（向后兼容）：`rounded-lg` (8px) / `rounded-md` (6px) / `rounded-sm` (4px) 继续可使用，存量 chrome 不变。

**反模式（参考 `AGENTS.md §5.3`）**：禁止出现 6px / 8px / 12px / 50% 圆角。新组件请使用 `rounded-card` / `rounded-tag` 二选一。

#### 2.2 间距（4 倍数 · 8 档）

| Tailwind 工具类 | 值 | 用途 |
|---|---|---|
| `ds-1` | 4px | 极小间距 |
| `ds-2` | 8px | Tag 内边距 |
| `ds-3` | 12px | 控件间距 |
| `ds-4` | 16px | 卡片内边距基准 |
| `ds-5` | 24px | 卡片内边距大 |
| `ds-6` | 32px | 区块分隔 |
| `ds-7` | 48px | 章节分隔 |
| `ds-8` | 64px | 页面分隔 |

#### 2.3 字号（major-third · 1.25 倍 · 8 档）

| Tailwind 工具类 | 像素 | 行高 | 用途 |
|---|---|---|---|
| `text-ds-xs` | 12 | 1.5 | 元信息 / 时间戳 |
| `text-ds-sm` | 13 | 1.5 | 表格副文 |
| `text-ds-md` | 14 | 1.5 | 正文基线 |
| `text-ds-lg` | 16 | 1.5 | 卡片标题 |
| `text-ds-xl` | 20 | 1.4 | 区段标题 |
| `text-ds-2xl` | 24 | 1.3 | 弹窗标题 |
| `text-ds-3xl` | 30 | 1.25 | KPI 大数字 |
| `text-ds-4xl` | 36 | 1.2 | 文档标题 |

**规则（`AGENTS.md §5.3` 强化）**：正文不 < 12px · 不 > 18px（KPI 数字除外）。所有金额 / 装分 / 人数 / 时间戳必须用等宽字体（见 §2.4 mono）。

#### 2.4 字体（系统回退 · 不下载 Web Font）

| Tailwind 工具类 | 字体 | 回退 | 用途 |
|---|---|---|---|
| `font-display` | **Source Han Serif SC** 思源宋体 | Noto Serif SC / 宋体 / Songti SC / serif | 标题 / 文档场景 |
| `font-body` | **PingFang SC** 苹方 | 微软雅黑 / Hiragino Sans GB / system-ui / sans-serif | 正文 / 表单 |
| `font-mono` | **SF Mono** | JetBrains Mono / Menlo / Consolas / monospace | 数字 / 等宽（强制 `tabular-nums`） |
| `font-han` | 同 `font-display` | — | 别名 |
| `font-pingfang` | 同 `font-body` | — | 别名 |

**反模式（`AGENTS.md §9`）**：禁止 Inter / Roboto / Arial / Fraunces 作**展示字体**。`font-sans` 默认仍为 Inter（向后兼容），新代码展示场景请用 `font-display` / `font-body` / `font-mono`。

### 3. 阴影与动效

#### 3.1 阴影（低强度 · 3 档）

| Tailwind 工具类 | 值（光） | 用途 |
|---|---|---|
| `shadow-ds-flat` | `0 1px 2px / 4%` | 卡片悬浮 |
| `shadow-ds-stack` | `0 2px 6px / 6%` | 弹窗 / 浮层 |
| `shadow-ds-modal` | `0 6px 16px / 8%` | Toast / 抽屉 |

暗主题下透明度自动加深到 20% / 28% / 36%（避免「浮不起」）。

**保留**（向后兼容）：`shadow-sm` / `shadow-md` / `shadow-lg` / `shadow-xl` / `shadow-2xl` 仍可用，但与 `AGENTS.md §5.3`「禁止重阴影（`shadow-lg`、`shadow-xl`）」存在冲突 —— **新组件优先使用 `shadow-ds-*`**。

#### 3.2 动效（短过渡 · 3 档）

| Tailwind 工具类 | 值 | 用途 |
|---|---|---|
| `duration-ds-fast` | 120ms | hover / 焦点 |
| `duration-ds-base` | 180ms | 颜色 / 边距过渡 |
| `ease-ds-ease` | `cubic-bezier(.2,.6,.2,1)` | 全局缓动 |
| `duration-ds-slow` | 280ms | 抽屉 / 弹窗进入 |

**规则（`AGENTS.md §9`）**：禁止 > 400ms 过渡；禁止弹性 / 回弹。

### 4. 设计原则合规清单（新组件必过）

> 📋 **扫描工具**：`node scripts/design-compliance-scan.mjs` · 输出 `docs/design-compliance.md`
> 📅 **最近扫描**：2026-08-23 · 39 个文件 · **0 处 high severity 违规**（合规）
> 📅 **最近批量修复**：2026-08-23 · 56 处 `shadow-md/lg/xl/2xl` → `shadow-ds-stack/modal`

- [ ] 主屏 `--accent` ≤ 2 处
- [ ] 圆角仅 `rounded-card` / `rounded-tag`
- [ ] 数字列加 `font-mono` · 表格 `font-variant-numeric: tabular-nums`
- [x] 阴影用 `shadow-ds-*`，不用 `shadow-md / shadow-lg / shadow-2xl` ← 2026-08-23 已批量合规（56 处 → `shadow-ds-stack/-modal`）
- [ ] 演示字体用 `font-display` / `font-body`，不用 `font-sans`
- [ ] 过渡 ≤ 400ms，禁用弹性缓动
- [ ] 文案使用简体中文、动词驱动、无 emoji、无感叹号堆叠
- [ ] 主操作按钮 ≤ 1 个 / 屏

### 5. 缺失资产（待补）

设计系统包 `tmp/design-zip/` 承诺但不在原 ZIP 内：

- ✅ **8 个 `build/icon-*.svg`** —— 已补齐（check / edit / error / info / list / plus / trash / warning），位置：`tmp/design-zip/build/`
- ✅ `fonts/`（思源宋体 / 苹方 / SF Mono 文件） —— 不补，走 system fallback；当前 `index.css` 配置合规
- ❌ `assets/`（图标 / 图片占位） —— 尚未补；本次新增不影响

### 6. 待办与决策点

| 1 | accent 颜色统一 | ✅ 已落地 | `--primary-base` 改为 沉香褐（`168 99 63`）· v2.2.0（2026-08-23） |
| 2 | warning/success 语义统一 | ✅ 已落地 | `emerald-* / amber-*` → `ds-success / ds-warning`（764 处替换）· v2.2.0 |
| 3 | AUDIT-REPORT 反模式违例 | ✅ 已修复 | `preview/applied-surfaces.html` 2 处 8px 圆角 + `brand-assets.html` 3 处 linear-gradient · v2.2.0 |
| 4 | `--role-*` 注释 + 审计脚本误报 | ✅ 已修复 | `README.md §5.3` grep 改为 `^\s*--role-` 行首匹配 · v2.2.0 |

---

### 7. 合规扫描规则说明（供维护者参考）

> 详见 `scripts/design-compliance-scan.mjs`

| 规则 ID | 严重度 | 检测目标 | 修复方式 |
|---|---|---|---|
| R1 | high | `shadow-md / lg / xl / 2xl` | 改为 `shadow-ds-stack / -modal` |
| R2 | high | `linear-gradient / radial-gradient` | 用单色 + shadow-ds-* |
| R3 | medium | `duration-500/700/1000` | 改为 `duration-ds-fast/base/slow` |
| R4 | medium | `ease-bounce / ease-spring` | 改为 `ease-ds-ease` |
| R5 | low | emoji 图标（🎮⚔🩸⏰👁🏆💎💰🔍✨🐛📊 等） | 用 Lucide React |
| R6 | low | 连续感叹号 `!!!` / `！！！` | AGENTS.md §4.4 |
| R7 | info | 红色直接用作文字色 | 合规：删除按钮 / 必填星号 / 错误提示 |
| R7-info | info | red 系总用量 | 全部用于合规场景 |
| R8 | low | `font-sans` 显式调用 | 展示位用 `font-display / font-body` |

**退出码**：发现 high severity 违规时 `exit 1`（CI 友好）

---

**最后更新**：2026-08-23 · 与 `tmp/design-zip/DESIGN.md v2.1.52` 同步
