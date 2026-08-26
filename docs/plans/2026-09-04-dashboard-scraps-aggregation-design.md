# 首页/收益概览散件展示与白名单汇总设计

> **状态**: 📝 待评审 · **关联版本**: v2.2.2
> **目标**: 把散件相关数据更直观地呈现到首页与收益概览详情，并补充白名单物品的副本级统计。

---

## 1. 背景与问题

### 1.1 当前散件展示问题

用户在首页（`components/Dashboard.tsx`）与收益概览详情（`components/IncomeDetail.tsx`）发现：

| 问题 | 现象 |
| --- | --- |
| 首页"散件估价"只显示本周 | 切换"本周 / 本赛季 / 全部"周期时，首页"散件估价"卡在"本周"值不变 |
| 首页散件估价独立成块 | 散件估价单独占一格，没有与"总收入"形成对照关系，散件老板视角下信息分散 |
| 收益概览详情缺乏散件维度 | 总收入 / 总支出 / 净收入三个核心指标没有散件支出、预估收益、预估总收益 |
| 散件白名单无汇总 | 散件清单白名单（7 种材料 + 装备散件）只散落在每条记录里，没有按"物品类型"全局统计每个物品的获取总数与副本均量 |

### 1.2 根因分析

1. **首页周期切换失效**：经核对 Dashboard.tsx 内 `statsPeriod` → `periodStartTime` → `filteredRecords` 链路是贯通的，理论上能切换。但 `stats.totalScrapsValue` 只统计 `isScrapsBoss=true` 的记录，当所有候选记录都是 `isScrapsBoss=false` 时，无论切到哪个窗口都是 0，给用户造成"切换不生效"的错觉。**真正的问题是展示方式**：散件估价以独立格子出现，无法区分"无数据"和"被周期过滤后为 0"。
2. **散件支出 / 预估收益 / 预估总收益缺失**：`IncomeDetail.stats` 仅汇总 `goldIncome / goldExpense`，没有针对 `isScrapsBoss=true` 的记录进一步分解"散件支出"与"散件预估收益"。
3. **白名单汇总缺失**：散件白名单 `SCRAPS_MATERIAL_WHITELIST` 仅在 `scrapsUtils.ts` 中作为过滤硬编码常量存在；没有按物品名聚合"获取总数 + 平均每个副本数量"的能力。

---

## 2. 设计目标

| 目标 | 衡量 |
| --- | --- |
| 首页散件信息更直观 | "本期总收入 + 散件估价"合并为一个单元格（卡片），展示两个数值 |
| 收益概览详情补全散件维度 | 总收入 / 总支出 / 净收入 三个指标分别补出"散件支出 / 散件预估收益 / 预估总收益" |
| 散件白名单全局汇总 | 在收益概览详情中追加"散件白名单汇总"区块，按"物品类型"分组列出每个白名单物品的"获取总数 / 涉及副本数 / 平均每副本数量" |
| 视觉规范保持一致 | 沿用现有 `emerald / amber` 语义色 + `rounded-xl border bg-surface` 容器样式，不引入重阴影 / 渐变 |

---

## 3. 方案设计

### 3.1 首页散件卡片合并

**位置**：`components/Dashboard.tsx` 第 415-459 行 `收益概览` 区块。

**当前结构**（5 列）：
```
[总收入] [通关次数] [团队副本] [百战异闻录] [散件估价]
```

**目标结构**（4 列）：单元格标题保持"**本期总收入**"，主数字 = `stats.totalGold`（**不含散件估价**），下方副标签展示"+散件估价 N 金"独立数值。

```
[本期总收入*] [通关次数] [团队副本] [百战异闻录]
[* 本期总收入单元格内：
   主：本期总收入 N 金 (emerald 主色)
   副：+散件估价 N 金 (muted 副色，仅 totalScrapsValue>0 时显示)
]
```

**口径确认**：
- `stats.totalGold` = 团队副本 goldIncome + 百战异闻录 goldIncome，**不包含** scrapsValue
- `stats.totalScrapsValue` = `isScrapsBoss=true` 记录的 `getRecordScrapsValue` 之和
- 标题文案"本期总收入"保留（不变），散件估价作为副标签展示，两者相加关系由用户自行理解

**实现要点**：
- 单元格沿用 emerald 风格（`border-ds-success-soft bg-ds-success-soft`）
- 主数字：本期总收入 N 金（沿用 `stats.totalGold`，emerald 强调）
- 副标签：`+散件估价 ${totalScrapsValue.toLocaleString()} 金`（muted 灰，`text-xs mt-1`）
- 当 `totalScrapsValue === 0` 时副标签不渲染，保持原观感

### 3.2 收益概览详情补全散件维度

**位置**：`components/IncomeDetail.tsx` 第 191-217 行 `stats` 计算 + 第 339-374 行三个卡片展示。

#### 3.2.1 stats 扩展

在 `stats` 中新增 4 个字段：

```ts
{
  // 已有
  totalIncome, totalExpense, netIncome,
  xuanjingCount, clientIncome, clientExpense, clientNetIncome,

  // 新增（仅统计 isScrapsBoss=true 的记录）
  scrapsExpense,         // 散件支出：该记录 goldExpense 之和（口径：整笔副本支出）
  scrapsEstimatedIncome, // 散件预估收益：该记录 getRecordScrapsValue 之和
  scrapsNetIncome,       // 散件净收入 = scrapsEstimatedIncome - scrapsExpense
  scrapsRecordCount,     // 散件老板记录条数（用于条件渲染副信息）
}
```

**计算口径**（用户口径确认版）：
- `scrapsExpense = confirmedRecords.filter(isScrapsBoss).reduce(acc, r) => acc + (r.goldExpense || 0))`
  - **不是**散件清单里物品的购买金额，而是该条记录整体的副本支出
- `scrapsEstimatedIncome = confirmedRecords.filter(isScrapsBoss).reduce(acc, r) => acc + getRecordScrapsValue(r)`
  - 即所有 `isScrapsBoss=true` 记录中散件估价的累加
- `scrapsNetIncome = scrapsEstimatedIncome - scrapsExpense`
  - **预估收益减去购买的支出**（用户原话）

> 语义解读：散件老板的真实"业务盈亏"= 散件预估变现 − 当次副本的全部支出。goldExpense 在散件老板视角下本质就是"购买散件花的钱"（因为散件老板的副本通常没有竞拍收入，金币支出的目的就是买入散件再变现），所以这里使用 `goldExpense` 而不是再单独解析 `purchased_items`。

#### 3.2.2 三个指标卡片新增副信息

| 主指标 | 原副信息 | 新副信息（散件口径，muted 小字） |
| --- | --- | --- |
| 总收入 | 代清收入: N 金 | 散件预估收益: N 金（仅 isScrapsBoss=true） |
| 总支出 | 代清支出: N 金 | 散件支出: N 金（仅 isScrapsBoss=true） |
| 净收入 | 代清净入: N 金 | 散件净收入: N 金（emerald / amber 随符号） |

副信息排版：**原副信息保留**，散件副信息紧贴其下方一行（`text-muted text-sm mt-1`），与现有代清副信息视觉一致。

当 `scrapsRecordCount === 0` 时，散件副信息不显示（避免对非散件老板的普通用户造成干扰）。

### 3.3 散件白名单全局汇总

**位置**：`components/IncomeDetail.tsx` 的"收益分布图表"之后追加新卡片区块。

**统计范围**：所有 `isScrapsBoss=true` 的记录，按物品名直接聚合（不分组）。

> 注：当前方案汇总数据**与收益概览详情共用周期筛选**（`filteredRecords`），与首页 / 总收入 / 总支出 / 净收入的口径保持一致；如果未来需要"全应用周期散件老板总账"，可单独再加一个开关。

#### 3.3.1 数据模型

```ts
/** 散件白名单汇总条目（按物品名直接聚合） */
interface ScrapsSummaryRow {
  name: string;                 // 物品名
  category: 'material' | 'equipment';
  totalCount: number;           // 获取总数（所有记录 count 累加）
  raidCount: number;            // 出现该物品的副本记录条数
  avgPerRaid: number;           // 平均每副本数量 = totalCount / raidCount
}
```

> 早期方案曾考虑按"物品类型"（茶饼类/丹药类/宝石类/五彩石类/五行石类/装备散件）分组展示，用户反馈"不需要分类，按物品统计即可"，本版本去掉分组逻辑。

#### 3.3.2 工具函数

新增 `utils/scrapsUtils.ts::summarizeScrapsByName(records)`：

```ts
export function summarizeScrapsByName(
  records: readonly Pick<RaidRecord, 'id' | 'isScrapsBoss' | 'scrapsItems'>[],
): ScrapsSummaryRow[] {
  // 1. 过滤 isScrapsBoss=true 的记录
  // 2. 遍历 scrapsItems，规范化（normalizeScrapsItem）后按 name 聚合
  // 3. 同时维护 raidCount（按 record.id 去重，每个副本记一次）
  // 4. 输出按 [totalCount 降序, name 升序] 排序
}
```

#### 3.3.3 展示样式

```
┌─────────────────────────────────────────────────────┐
│ 散件白名单汇总                          本周 · →     │
├─────────────────────────────────────────────────────┤
│  物品           分类      总数   副本数  均/副本     │
│  维峰丹         材料       24     8      3.00       │
│  玛瑙           材料       17     5      3.40       │
│  上品茶饼·兑    材料       12     6      2.00       │
│  五行石（六级） 材料        9     5      1.80       │
│  肆级五彩石     材料        6     3      2.00       │
│  叁级五彩石     材料        4     2      2.00       │
│  流漓腰带       装备        1     1      1.00       │
└─────────────────────────────────────────────────────┘
```

- 表格按物品名聚合，**不分组**；每件物品一行
- 表头用 `text-[10px] text-muted`，与现有稀有掉落统计卡片一致
- 数值列右对齐、`<span className="font-semibold">` 突出
- 平均数保留 2 位小数 `avg.toFixed(2)`
- "副本数"= 出现该物品的副本记录条数（同一副本多种散件都计 1 次该副本）
- 默认按"获取总数"降序展示（高数量物品靠前）

### 3.4 兼容与降级

| 场景 | 处理 |
| --- | --- |
| 旧记录无 `scrapsItems` | `getRecordScrapsValue` 已回退到 `scrapsValue`，无需改动 |
| 所有记录 `isScrapsBoss=false` | 散件副信息不渲染；白名单汇总区块显示"暂无散件数据"占位 |
| 切换周期后无散件数据 | 卡片降级为只显示主指标，与现状一致 |
| `category` 缺失 | `normalizeScrapsItem` 已默认 `'material'`，与 §3.3 一致 |

---

## 4. 改造点

### 4.1 前端组件

| 文件 | 改动 |
| --- | --- |
| `components/Dashboard.tsx` | `收益概览` 区块重构（合并散件估价到"本期收益"单元） |
| `components/IncomeDetail.tsx` | `stats` 扩展 4 个散件字段；三个指标卡片补副信息；新增"散件白名单汇总"区块 |
| `utils/scrapsUtils.ts` | 新增 `classifyScrapsType(name, category)` 与 `summarizeScrapsByName(records)` 函数 |
| `utils/scrapsUtils.test.ts` | 补充 `summarizeScrapsByName` 的单元测试 |
| `types.ts` | 可选：`DashboardStats` 增加 `totalScrapsValue` 字段已存在无需改动；可新增 `ScrapsSummaryRow` 类型（也允许在 `scrapsUtils.ts` 中定义） |

### 4.2 验证

1. **构建 / 类型**：`npm run build` 通过
2. **单元测试**：`npm run test` 通过；新增 `summarizeScrapsByName` 测试覆盖：
   - 多记录同物品聚合
   - `isScrapsBoss=false` 记录被排除
   - 旧 snake_case 数据规范化
   - 空记录返回空数组
   - 类型分组（茶饼/丹药/宝石/五彩石/五行石/装备）正确
   - `raidCount` 按 record.id 去重
3. **手工回归**：
   - 切"本周 / 本赛季 / 全部"，首页"本期收益"与"散件估价"随周期同步变化
   - 收益概览详情中：散件老板视角下"总收入 / 总支出 / 净收入"下方均出现散件副信息
   - 散件白名单汇总按物品展示数量，且"副本数 = 该物品出现过的记录条数"
   - 普通用户（非散件老板）三处副信息均不出现，无视觉空缺

---

## 5. 用户视角说明（用于 Release Notes）

- 首页"收益概览"重构：原"总收入 + 散件估价"两格合并为"本期总收入"一格，主数字展示总收入，副标签展示"+散件估价"，一眼看清总账与散件账
- 收益概览详情的总收入 / 总支出 / 净收入三张卡片追加"散件预估收益 / 散件支出 / 散件净收入"副信息，散件老板可在同一卡片内对照总账与散件账
- 收益概览详情新增"散件白名单汇总"：按"物品类型"（茶饼/丹药/宝石/五彩石/五行石/装备）分组展示每个物品的获取总数、出现副本数、平均每个副本数量，方便对比白名单材料的副本级收益