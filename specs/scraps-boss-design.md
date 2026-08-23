# 散件老板功能设计文档

## 1. 概述

### 1.1 功能目标

副本中部分材料和不可交易装备无人竞拍时，通常由"散件老板"统一接收。本功能在自动扫描副本掉落时，自动识别散件老板接手的物品清单并估算价值，为散件老板的收支核算提供数据支撑。

### 1.2 核心规则（基于当前角色）

- **散件材料**：当前角色购买的所有材料（0 金或非 0 金都算）
- **散件装备**：当前角色 0 金购买的装备（=没人要的散件）
- **估价策略**：扫描时快照（材料调 JX3Box 价格 API，装备读 NPC 卖价）
- **统计方式**：`scrapsValue` 作为单独一列"散件估价"，不并入 `goldIncome`
- **散件老板身份**：用户在确认弹窗中勾选"我是散件老板"后生效（`isScrapsBoss=true`），默认 `false`

### 1.3 设计原则

- 不新建数据库表，散件清单存入 records 表 `data` JSON 字段
- 不长期维护材料单价表（材料价格每日变动，长期存储无意义）
- 扫描时调 API 获取单价快照，缓存仅内存级（1 小时）
- API 不可用时降级为用户手填当日单价

## 2. 现状分析

### 2.1 现有数据流

自动扫描主流程位于 [drop_scanner.rs](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs)：

```
scan_raid_drops_with_raids
  → extract_drops_from_records  (从 chatlog 提取 drops、购买物品、收入支出)
  → upsert_raid_drop_record     (分类、构造 JSON、写入 records 表)
```

### 2.2 关键现状

| 现状 | 位置 | 影响 |
|---|---|---|
| `purchased_items_set` 用 `HashSet<String>` 去重，丢失数量 | [drop_scanner.rs:2084](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2084) | 需改为 `HashMap<String, PurchaseInfo>` |
| `EXPENSE_MSG_RE` 只匹配当前角色 | [drop_scanner.rs:2127-2129](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2127) | 符合设计，散件清单只看当前角色 |
| 购买金额已解析但未存入 purchased_items | [drop_scanner.rs:2130](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2130) | 复用 `parse_expense_amount` 即可 |
| `drops_set` 来源含任意玩家购买 | [drop_scanner.rs:2164](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2164) `PURCHASED_ITEM_RE` | 不能用 `drops - purchased` 推断"没人要" |
| `drop_items` 表有 `Price` 字段 | [db.rs:1391](file:///e:/Data/jx3-raid-manager/src-tauri/src/db.rs#L1391) | 装备估价可直接读 |
| RaidRecord 无散件字段 | [types.ts:66-97](file:///e:/Data/jx3-raid-manager/types.ts#L66) | 需新增字段 |
| 项目无物价 API 实现 | [drop_table.rs:7-8](file:///e:/Data/jx3-raid-manager/src-tauri/src/drop_table.rs#L7) | 需新建价格 API 客户端 |

### 2.3 非散件老板场景

`isScrapsBoss=false`（默认）时，散件清单仍会自动计算并存入 `scrapsItems` / `scrapsValue`，但不计入收支统计。用户可在确认弹窗中查看散件清单，决定是否勾选"我是散件老板"。

## 3. 散件清单规则

### 3.1 数据来源

**仅基于当前角色的购买记录**（`purchased_items`），不提取其他玩家的购买记录。

> 设计讨论中曾考虑"提取所有玩家购买记录 → 计算 drops - 所有购买 = 没人要"的方案。审查发现 `drops_set` 已包含任意玩家购买（`PURCHASED_ITEM_RE`），`drops - purchased_items` 差集中混入其他玩家购买物品，无法可靠识别"没人要"。故规则简化为：只统计当前角色购买的物品。

### 3.2 散件材料识别

```rust
// 7 种材料白名单（硬编码常量）
const SCRAPS_MATERIAL_WHITELIST: [&str; 7] = [
    "上品茶饼·兑", "维峰丹", "玛瑙", "猫眼石",
    "叁级五彩石", "五行石（六级）", "肆级五彩石",
];
```

**判定条件**：`purchased_items` 中物品名 ∈ 白名单 → 散件材料

不依赖 `drop_items.category`，原因：
- `purchased_items` 中的物品不一定在 `drop_items` 缓存中（副本外交易也会出现在购买记录里）
- 白名单精确匹配更可靠

### 3.3 散件装备识别

**判定条件**：`purchased_items` 中物品 `category == 'equipment'` 且 `total_price == 0`

- `category` 来自 `drop_items` 表（通过 [drop_scanner.rs:2709](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2709) `classify_drops_sync` 已填充 `item_categories`）
- `total_price == 0` 表示 0 金购买（即没人要）
- **不查询 `CanTrade` 字段**（已确认不需要排除可交易装备）

### 3.4 不计入散件的物品

- 非 0 金购买的装备（正常购买，不算散件）
- 不在白名单的材料（如副本外的杂物）
- 其他玩家购买的物品（不在 `purchased_items` 中）

### 3.5 数量统计

同一物品名多次购买累加：

```
购买记录 1: 维峰丹 × 1 (0金)
购买记录 2: 维峰丹 × 1 (0金)
购买记录 3: 玛瑙  × 1 (500金)

→ 散件清单:
  维峰丹 × 2, unitPrice=JX3Box单价, category=material
  玛瑙  × 1, unitPrice=JX3Box单价, category=material
```

## 4. 估价规则

### 4.1 估价来源

| 物品类型 | 估价来源 | 降级方案 |
|---|---|---|
| 散件材料 | JX3Box 价格 API 当日单价 | API 不可用 → 用户手填 |
| 散件装备 | `drop_items.Price`（NPC 卖价） | drop_items 无记录 → `unitPrice=null`，用户手填 |

### 4.2 单位约定（v2.2.1 修正）

- `drop_items.Price` 为 **NPC 卖价，单位铜**；存入 `unitPrice` 前必须换算为金：`(copper + 5000) / 10000`（四舍五入）
- JX3Box / 手填价格本身已是金，直接存入
- v2.1.53 曾把铜当金存入，导致装备估价虚高约一万倍；前端展示层已做旧数据兼容（仅对 npc 来源的旧格式记录重算）

### 4.3 价格快照

扫描时获取单价并存入 `scrapsItems[].unitPrice`，作为历史快照。后续查看记录时不再重新拉取价格。

### 4.4 JX3Box 价格 API（已验证：不可用 → 走降级方案）

**P0 阶段验证结果**：3 个候选端点均返回 Not Found，公开搜索无 JX3Box 交易行价格 API 文档。

| 候选端点 | 返回 |
|---|---|
| `https://node.jx3box.com/item/sumary/3_5` | `{"code":1000,"msg":"Not Found"}` |
| `https://node.jx3box.com/item/price/5_44430` | `{"code":1000,"msg":"Not Found"}` |
| `https://next2.jx3box.com/api/float-market/item/5_44430` | `Not Found` |

**结论**：JX3Box 价格 API 端点无法确认可用，**P2 阶段采用降级方案**：

- 扫描时材料 `unitPrice = null`、`priceSource = 'manual'`
- 前端确认弹窗显示输入框，用户手填当日单价
- 后续如发现可用的价格 API，可在 `drop_table.rs` 新增 `fetch_material_price` 函数，扫描时自动填充 `unitPrice`

**降级方案**：API 不可用时，`unitPrice = null`、`priceSource = 'manual'`，前端确认弹窗显示输入框让用户手填当日单价。

### 4.4 内存缓存

价格 API 结果内存缓存 1 小时（同一次扫描多个副本可复用）：

```rust
// Rust 全局缓存
static MATERIAL_PRICE_CACHE: OnceLock<Mutex<HashMap<String, (i64, Instant)>>> = OnceLock::new();
// key: 物品名, value: (单价, 缓存时间)
// 缓存有效期 1 小时，过期后重新拉取
```

不持久化到数据库（材料价格每日变动，长期存储无意义）。

### 4.5 scrapsValue 计算

```
scrapsValue = Σ(scrapsItems[].count × unitPrice)
```

- `unitPrice = null` 的物品按 0 计入（用户手填后前端重新计算并回写）
- `isScrapsBoss = false` 时 `scrapsValue` 仍计算存储，但不计入收支统计

## 5. 数据结构

### 5.1 Rust（drop_scanner.rs 新增）

```rust
/// 当前角色购买物品的信息（替换原 HashSet<String>）
#[derive(Debug, Clone)]
struct PurchaseInfo {
    count: u32,         // 购买次数（同物品多次购买累加）
    total_price: i64,   // 累计购买价（用于判断 0 金）
}

/// 散件清单项
#[derive(Debug, Clone, Serialize)]
struct ScrapsItem {
    name: String,
    count: u32,
    unit_price: Option<i64>,    // None 表示需用户手填
    category: String,           // "material" | "equipment"
    price_source: String,       // "jx3box" | "npc" | "manual"
}

const SCRAPS_MATERIAL_WHITELIST: [&str; 7] = [
    "上品茶饼·兑", "维峰丹", "玛瑙", "猫眼石",
    "叁级五彩石", "五行石（六级）", "肆级五彩石",
];
```

### 5.2 TypeScript（types.ts RaidRecord 新增）

```ts
export interface ScrapsItem {
  name: string;
  count: number;
  unitPrice: number | null;    // null 表示需用户手填
  category: 'material' | 'equipment';
  priceSource: 'jx3box' | 'npc' | 'manual';
}

export interface RaidRecord {
  // ... 现有字段 ...

  /** 散件清单（自动扫描） */
  scrapsItems?: ScrapsItem[];
  /** 散件估价总和（扫描时快照，unitPrice 未填齐时为部分和） */
  scrapsValue?: number;
  /** 是否为散件老板（用户在确认弹窗勾选，默认 false） */
  isScrapsBoss?: boolean;
}
```

### 5.3 records 表 JSON 结构

散件字段存入 `data` JSON，与 `drops`、`jclFiles` 并列：

```json
{
  "id": "...",
  "accountId": "...",
  "raidName": "25人普通阆风悬城",
  "goldIncome": 50000,
  "goldExpense": 8000,
  "drops": ["流漓腰带", "维峰丹", ...],
  "jclFiles": [...],
  "scrapsItems": [
    {
      "name": "维峰丹",
      "count": 2,
      "unitPrice": 1500,
      "category": "material",
      "priceSource": "jx3box"
    },
    {
      "name": "流漓腰带",
      "count": 1,
      "unitPrice": 25000,
      "category": "equipment",
      "priceSource": "npc"
    }
  ],
  "scrapsValue": 28000,
  "isScrapsBoss": false
}
```

## 6. 改造点

### 6.1 Rust 后端

| 优先级 | 改造点 | 位置 |
|---|---|---|
| P0 | 验证 JX3Box 价格 API 端点可用性 | 手动 curl 候选 URL |
| P0 | `purchased_items_set` 改为 `HashMap<String, PurchaseInfo>`，去重通过后 `count += 1`、`total_price += amount` | [drop_scanner.rs:2084](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2084)、[:2143](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2143) |
| P0 | `all_purchased_items` 同步改为 HashMap 合并 | [drop_scanner.rs:3241](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L3241) |
| P0 | `upsert_raid_drop_record` 签名改为接收 `HashMap<String, PurchaseInfo>` | [drop_scanner.rs:2529](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2529) |
| P1 | `extract_drops_from_records` 返回值第 8 项改为 `HashMap<String, PurchaseInfo>` | [drop_scanner.rs:2049](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2049) |
| P1 | 新增散件清单计算逻辑（白名单匹配材料 + `category=='equipment' && total_price==0` 匹配装备） | [drop_scanner.rs:2754 之后](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2754) |
| P1 | RaidRecord JSON 新增 `scrapsItems`、`scrapsValue`、`isScrapsBoss=false` 字段 | [drop_scanner.rs:2799](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2799) |
| P1 | 现有 notes 构建逻辑适配 HashMap 迭代 | [drop_scanner.rs:2769](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L2769) |
| P2 | JX3Box 价格 API 客户端实现（内存缓存 1 小时） | drop_table.rs 新增函数 |
| P2 | 装备估价读 `drop_items.Price` | SQL 查询 |
| P2 | 材料估价调 JX3Box 价格 API，失败时 `unitPrice=null` | drop_table.rs |

### 6.2 前端

| 优先级 | 改造点 | 位置 |
|---|---|---|
| P1 | types.ts RaidRecord 新增 `scrapsItems`、`scrapsValue`、`isScrapsBoss` 字段 | [types.ts:66](file:///e:/Data/jx3-raid-manager/types.ts#L66) |
| P3 | PendingRecordsPanel 确认弹窗新增"散件老板"勾选框 | [PendingRecordsPanel.tsx](file:///e:/Data/jx3-raid-manager/components/PendingRecordsPanel.tsx) |
| P3 | 确认弹窗显示散件清单，`unitPrice=null` 项显示手填输入框 | PendingRecordsPanel.tsx |
| P3 | 用户手填单价后回写 `scrapsItems[].unitPrice` 并重新计算 `scrapsValue` | PendingRecordsPanel.tsx |
| P3 | 列表/详情新增"散件估价"列（与收入/支出并列） | RaidDetail.tsx、Dashboard.tsx |
| P3 | `isScrapsBoss=true` 的记录，`scrapsValue` 计入散件老板收支统计 | 统计逻辑 |

### 6.3 兼容性

- 旧记录无 `scrapsItems` 字段：前端 `?.` 可选链访问，显示为空
- `purchased_items` 改为 HashMap 后，`upsert_raid_drop_record` 内部 notes 构建逻辑需适配，但 notes 内容不变（仍为"购买装备: XX、YY"）
- `drops` 字段保持 `Vec<String>` 去重列表，向后兼容

## 7. UI/UX 设计

### 7.1 确认弹窗（PendingRecordsPanel）

在现有编辑弹窗中新增"散件老板"区块：

```
┌─────────────────────────────────────┐
│  确认副本记录                        │
├─────────────────────────────────────┤
│  副本名: 25人普通阆风悬城            │
│  收入: [50000]  支出: [8000]         │
│  ...                                │
├─────────────────────────────────────┤
│  ☑ 我是散件老板                     │
│                                     │
│  散件清单:                          │
│  ┌─────────────────────────────┐   │
│  │ 维峰丹 ×2  单价: [1500] 金  │   │
│  │ 玛瑙  ×1  单价: [___] 金    │   │  ← 空白需手填
│  │ 流漓腰带 ×1 单价: 25000(NPC)│   │  ← 只读
│  └─────────────────────────────┘   │
│  散件估价合计: 28000 金              │
├─────────────────────────────────────┤
│           [取消]  [确认]             │
└─────────────────────────────────────┘
```

- "我是散件老板"勾选框默认不勾选
- 散件清单默认展示（无论是否勾选），让用户预览
- `unitPrice=null` 的材料项显示输入框，用户填入后实时更新估价合计
- `priceSource='npc'` 的装备项只读显示
- `priceSource='jx3box'` 的材料项可编辑（允许用户覆盖 API 价格）

### 7.2 列表展示

在副本记录列表/详情中新增"散件估价"列：

| 副本 | 收入 | 支出 | 散件估价 | ... |
|---|---|---|---|---|
| 25人普通阆风悬城 | 50000 | 8000 | 28000* | ... |

- 带 `*` 表示 `isScrapsBoss=true`，该估价计入散件老板收支
- 不带 `*` 表示 `isScrapsBoss=false`，估价仅展示不计入统计

### 7.3 样式规范

遵循 [AGENTS.md](file:///e:/Data/jx3-raid-manager/AGENTS.md) 第 5 节：

- 散件估价列使用 `text-muted`（中性灰），不与收入（emerald）/支出（amber）色彩冲突
- "我是散件老板"勾选框激活色使用 emerald 系
- 散件清单容器使用单色背景 + 轻边框，禁止重阴影

## 8. 边界与降级

### 8.1 JX3Box 价格 API 不可用

- 扫描时：材料 `unitPrice=null`、`priceSource='manual'`，`scrapsValue` 为部分和（仅含已获取单价的物品）
- 确认时：前端显示手填输入框，用户填齐后回写
- 不阻塞扫描主流程，仅记录警告日志

### 8.2 drop_items 无装备记录

- 装备 `unitPrice=null`、`priceSource='manual'`
- 前端显示手填输入框

### 8.3 购买记录为空

- `scrapsItems=[]`、`scrapsValue=0`
- 确认弹窗中散件清单区显示"无散件物品"

### 8.4 0 金购买的装备不在 drop_items 缓存中

- `category` 无法判断 → 不计入散件装备（保守策略，避免误判）
- 日志记录跳过原因

## 9. 实现优先级

| 阶段 | 内容 | 交付物 |
|---|---|---|
| P0 | 验证 JX3Box 价格 API 端点 | API 返回格式文档、是否需鉴权 |
| P1 | Rust 侧 HashMap 改造 + 散件清单计算 + JSON 字段 | `cargo build` 通过，扫描日志含散件清单 |
| P2 | JX3Box 价格 API 客户端 + 装备估价 | 扫描后 `scrapsItems[].unitPrice` 已填充（API 可用时） |
| P3 | 前端字段 + 确认弹窗 + 列表展示 | 端到端可用，散件老板可勾选并查看估价 |

## 10. 测试验证

### 10.1 单元测试

- `parse_expense_amount("0金")` 返回 0（已有测试，[drop_scanner.rs:4827](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs#L4827)）
- 散件清单计算：
  - 白名单材料识别正确
  - 0 金装备识别正确
  - 非 0 金装备不计入散件
  - 同物品多次购买 count 累加正确
- `scrapsValue` 求和正确（含 `unitPrice=null` 按 0 计入）

### 10.2 集成测试

- 使用真实 chatlog 数据扫描，验证散件清单内容
- JX3Box 价格 API 调用（需网络，`#[ignore]` 标记）
- API 失败降级路径

### 10.3 前端验证

- 旧记录（无 scrapsItems 字段）正常显示，无报错
- 确认弹窗勾选"散件老板"后，`isScrapsBoss=true` 持久化
- 手填单价后 `scrapsValue` 实时更新
- 列表"散件估价"列正确展示

## 11. 参考链接

- [drop_scanner.rs](file:///e:/Data/jx3-raid-manager/src-tauri/src/mingyi/drop_scanner.rs) - 自动扫描主逻辑
- [drop_table.rs](file:///e:/Data/jx3-raid-manager/src-tauri/src/drop_table.rs) - JX3Box API 客户端
- [types.ts](file:///e:/Data/jx3-raid-manager/types.ts) - RaidRecord 类型定义
- [PendingRecordsPanel.tsx](file:///e:/Data/jx3-raid-manager/components/PendingRecordsPanel.tsx) - 确认弹窗
- [db.rs](file:///e:/Data/jx3-raid-manager/src-tauri/src/db.rs) - drop_items 表结构
- [AGENTS.md](file:///e:/Data/jx3-raid-manager/AGENTS.md) - 开发规范
- [specs/design-tokens.md](file:///e:/Data/jx3-raid-manager/specs/design-tokens.md) - UI 设计规范
