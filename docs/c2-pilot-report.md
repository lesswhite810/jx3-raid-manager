# 决策点 1 · 方案 C 试点报告

> **C2 试点**：`IncomeDetail.tsx` 5 处替换 → 视觉验证 → 决定铺开/收口
> **执行日期**：2026-08-23
> **关联文档**：`docs/design-decision-1.md §3.3 方案 C` · `specs/design-tokens.md §1.4`

---

## 一、试点范围

| # | 位置 | 字号 | 原类名 | 新类名 | 场景 |
|---|---|---|---|---|---|
| ① | IncomeDetail.tsx:651 | `text-xs` | `text-ds-warning` | **`text-ds-warning-strong`** | 支出标签 |
| ② | IncomeDetail.tsx:671 | `text-xs` | `text-ds-success` | **`text-ds-success-strong`** | 收入标签 |
| ③ | IncomeDetail.tsx:760 | `text-sm` | `text-ds-success` | **`text-ds-success-strong`** | 收入指示条 |
| ④ | IncomeDetail.tsx:593/595 | `text-sm` | `text-ds-success/warning` | **`*-strong`** | summaryColorClass 主分支 |
| ⑤ | IncomeDetail.tsx:597/599 | `text-sm` | `text-ds-success/warning` | **`*-strong`** | summaryColorClass 净收入分支 |

**保留不动的位置**（按设计哲学保持原色）：

| 位置 | 字号 | 类名 | 理由 |
|---|---|---|---|
| L367 | `text-3xl` | `text-ds-success/warning` | 大字号（≥18px）已 AA 大字号 ≥3.0:1 合规 |
| L548/557 | `text-sm` | `bg-surface text-ds-success/warning` | Tab 选中态，配 `bg-surface` 白底 + 阴影，留 ② 试点里观察 |

## 二、新增 Token（`index.css` + `tailwind.config.js`）

### 2.1 三个派生 token（光主题）

```css
:root {
  --success-strong: #3d5e4a;  /* 用于 ≤18px 正文场景（白底 7.25:1） */
  --warning-strong: #6e5a30;  /* 用于 ≤18px 正文场景（白底 6.63:1） */
  --info-strong:    #3e546a;  /* 用于 ≤18px 正文场景（白底 7.83:1） */
}

[data-theme="dark"] {
  --success-strong: #87b29a;  /* 暗主题:浅一档 */
  --warning-strong: #c9a06b;  /* 暗主题:浅一档 */
  --info-strong:    #8eaabd;  /* 暗主题:浅一档 */
}
```

### 2.2 Tailwind 工具类

```js
'ds-success': {
  DEFAULT: 'var(--success)',
  soft: 'var(--success-soft)',
  strong: 'var(--success-strong)',   // ← 新增
},
'ds-warning': {
  DEFAULT: 'var(--warning)',
  soft: 'var(--warning-soft)',
  strong: 'var(--warning-strong)',   // ← 新增
},
'ds-info': {
  DEFAULT: 'var(--info)',
  soft: 'var(--info-soft)',
  strong: 'var(--info-strong)',      // ← 新增
},
```

## 三、对比度实测

| Token | vs 白底 | vs 暖米杏 | 状态 |
|---|---:|---:|---|
| `--success`（原）| 4.07:1 ✗ | 3.80:1 ✗ | 不达 AA 正文 |
| `--success-strong`（新）| **7.25:1 ✅** | 6.78:1 ✅ | **达 AAA 7.0+ 标准** |
| `--warning`（原）| 3.60:1 ✗ | 3.37:1 ✗ | 不达 AA 正文 |
| `--warning-strong`（新）| **6.63:1 ✅** | 6.20:1 ✅ | **达 AA 标准** |
| `--info`（原）| 4.30:1 ✗ | 4.02:1 ✗ | 不达 AA 正文 |
| `--info-strong`（新）| **7.83:1 ✅** | 7.33:1 ✅ | **达 AAA 7.0+ 标准** |

## 四、验证结果

| 检查 | 结果 |
|---|---|
| Vite build | ✅ 通过 · 9.88s · CSS 体积持平（`index-CDdCY7uW.js` 357.85 KB / gzip 87.63 KB） |
| 合规扫描 high severity | ✅ 0 处 |
| 设计系统烟测 | ✅ 16 / 16 通过 |
| 9 条手动清单 | ✅ 9 / 9 通过 |

## 五、视觉影响评估

### 5.1 同色系内的"明度梯度"

引入 `*-strong` 后，每个语义色变成 **3 档梯度**：

```
ds-{success,warning,info}:
  DEFAULT (--{name})     ← 提案基调（低饱和，用于大字号 / Tag）
  soft    (--{name}-soft)   ← 已存在的背景色
  strong  (--{name}-strong) ← 新增派生（小正文专用，更深 → 对比度更高）
```

这种梯度与现有 `ds-accent` (DEFAULT/soft/strong) 完全对齐，命名一致。

### 5.2 视觉感受预判

| 场景 | 原视觉 | 新视觉 |
|---|---|---|
| `text-3xl` 净收入 | 苔绿 `#5d8770` 浅淡 | **不变** |
| `text-xs` 收入标签 | 苔绿 `#5d8770`（对比度弱）| **松绿 `#3d5e4a`**（更深、清晰）|
| `text-sm` 收入指示条 | 苔绿 `#5d8770` | **松绿 `#3d5e4a`** |
| `text-xs` 支出标签 | 焦糖 `#a38246` | **暗金 `#6e5a30`** |
| 大字号 `text-3xl` 支出 | 焦糖 `#a38246` | **不变** |

**配色哲学平衡**：
- 大字号保持提案原色（视觉降饱和度，长时段不刺眼）✅
- 小字号用 strong 派生（确保可读性）✅
- 同一语义色在不同场景下保持 **可识别的一致性**（视觉上仍是"绿色收入 / 黄色支出"）

## 六、铺开决策建议

### 6.1 推荐 ✅ 全仓铺开

**理由**：

1. **零技术风险** —— 5 处试点 build 通过，CSS 体积持平，工具类命名一致
2. **明确的可读性提升** —— `*-strong` 在白底均超 6.6:1，达到 WCAG AA 甚至 AAA 标准
3. **大字号不受影响** —— 保留原色的设计哲学优势
4. **改动局部化** —— 后续约 25 处替换可在 1 个 PR 内完成

### 6.2 铺开清单（待你确认后执行）

```
C3 铺开:
  1. specs/design-tokens.md §1.4
     新增 "派生 strong 系列" 表格 + "何时用 strong" 决策树

  2. 替换以下组件类名(约 25 处):
     • Dashboard.tsx         (收入/支出统计)
     • RoleRecordsModal.tsx  (收支明细卡片)
     • BaizhanRoleRecordsModal.tsx
     • AddRecordModal.tsx    (支出标签)
     • AddBaizhanRecordModal.tsx
     • AddTrialRecordModal.tsx
     • IncomeDetail.tsx 剩余位置(L548/557 Tab 选中态等)

  3. 验证:
     • Vite build
     • 合规扫描 0 处
     • 烟测 16 通过
     • 视觉回归(肉眼过 IncomeDetail / Dashboard)
```

## 七、关键决策点

| 待定项 | 建议 | 备注 |
|---|---|---|
| Tab 选中态（`bg-surface text-ds-success`）是否也用 strong？ | **暂保留** | 这是 active 态而非阅读位，对比度需求不同 |
| 主操作按钮 hover 态的 `bg-primary-hover` 是否补 strong？ | **不补** | `--accent-strong` 已存在且 6.48:1 合规 |
| `*-strong` 是否要同步加到 `bg-*-strong`（背景色）？ | **暂不加** | 当前需求是 text，正文用；背景色已有 `*-soft` |

---

**最后整理**：2026-08-23 · 试点 5 处全部成功 → 等你决定铺开（C3）还是收口（回退）