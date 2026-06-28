# 副本掉落自动扫描设计（B 阶段）

## 背景

当前项目的收支记录完全依赖用户手动输入（`AddRecordModal.tsx` + `RaidLogger.tsx`），
缺少基于茗伊 chatlog 的自动化采集能力。B 阶段目标是自动扫描 chatlog 中的装备分配事件，
记录每场副本的掉落物，减少手工录入负担。

与长安记银的"实时监控工资条"路线不同，本设计聚焦于"装备掉落记录"，不记录金额，
通过 JCL 文件名识别副本实例，从 MSG_TEAM 提取分配事件，实现轻量化的自动采集。

## 调研发现

### ChatLog 表结构

文件位置：`<JX3>\bin\zhcn_hd\interface\my#data\{uid}@zhcn_hd\userdata\chat_log\chatlog_<hash>.v2.db`

```sql
CREATE TABLE ChatLog (
    hash INTEGER NOT NULL,
    type VARCHAR(64) NOT NULL,
    time INTEGER NOT NULL,
    talker NVARCHAR(20) NOT NULL,
    text NVARCHAR(400) NOT NULL,
    msg NVARCHAR(4000) NOT NULL,
    PRIMARY KEY (time, hash)
);
CREATE INDEX ChatLog_type_idx ON ChatLog(type);
CREATE INDEX ChatLog_talker_idx ON ChatLog(talker);
CREATE INDEX ChatLog_text_idx ON ChatLog(text);

CREATE TABLE ChatInfo (
    key NVARCHAR(128) NOT NULL,
    value NVARCHAR(4096) NOT NULL,
    PRIMARY KEY (key)
);
```

`ChatInfo` 表含 `version`、`user_global_id`（当前登录角色 ID）、`min_time`、`max_time`。

### 收支相关消息样本

**MSG_TEAM（团队分配，本设计数据源）**
```
[团队][少年白了发]：[弥狂护腕] 0金 分配给 [雪意垂野]
[团队][少年白了发]：[星雷陨铁] 0金 分配给 [雪意垂野]
```
正则：`\[([^\]]+)\]\s*\d+金\s*分配给\s*\[([^\]]+)\]`

**MSG_ROOM（GKP 拍卖，本设计不采集）**
```
[房间][购买者·区服]：[购买者·区服]花费[6000金]购买了[风掣链]
[房间][购买者·区服]：[购买者·区服]花费[1金砖5000金]购买了[暮峰护腕]
```

**MSG_MONEY（系统金钱，本设计不采集）**
```
你获得：1095493。   ← 铜币总数
```

**MSG_ITEM（系统物品，本设计不采集）**
```
你获得：[秘境宝藏·普通白帝江关]。
雪意垂野获得：[弥狂护腕]。
分配者将：[弥狂护腕]分配给雪意垂野。
```

### JCL 文件名格式

文件位置：`<JX3>\bin\zhcn_hd\interface\my#data\{uid}@zhcn_hd\userdata\combat_logs\*.jcl`

格式：`YYYY-MM-DD-HH-MM-SS-副本名(副本ID)-BOSS名(BOSS ID).jcl`

样本：
- `2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl`
- `2026-06-14-16-50-08-英雄天龙寺(683)-枯荣大师·幻影(129124).jcl`
- `2026-06-15-23-56-47-试炼之地(143)-寅(17616).jcl`

JCL 文件名为 GBK 编码，需用 `encoding_rs` 解码后解析。

### JCL 副本名与 raids 表匹配

JCL 副本名格式：`{人数}人{难度}{副本名}` 或 `{难度}{副本名}` 或 `{副本名}`

匹配策略：剥离前缀（人数+难度）后与 `raids.name` 做后缀匹配。

| JCL 副本名 | raids.name | 难度 | 人数 |
|-----------|-----------|------|------|
| `25人普通阆风悬城` | `阆风悬城` | 普通 | 25 |
| `25人英雄阆风悬城` | `阆风悬城` | 英雄 | 25 |
| `英雄天龙寺` | `天龙寺` | 英雄 | - |
| `试炼之地` | `试炼之地` | - | - |

匹配后从 `raid_bosses` 表查询该副本的 BOSS 总数与名称列表。

### 现有 CD 系统调研

- **CD 数据源**：唯一的 `records` 表，`bossIds` 字段标记 BOSS 击杀
- **CD 计算**：`calculateCooldown`（utils/cooldownManager.ts）+ `calculateBossCooldowns`（utils/bossCooldownManager.ts）
- **CD 周期**：25人本每周一7点刷新，10人本一周两次（周一/周五7点）
- **三态显示**：未清（emerald）/ 部分清（amber）/ 完全清（slate）
- **手动记录**：用户在 AddRecordModal 勾选 BOSS + 填写收支，写入 records 表的 RaidRecord

## 设计方案

### 1. 扩展 `records` 表

不新建独立表，直接扩展现有 `records` 表，让自动扫描记录与手动记录共享同一数据源，
CD 计算逻辑几乎不变（只需排除 rejected 记录）。

```sql
-- records 表新增字段（v12 迁移）
ALTER TABLE records ADD COLUMN source TEXT DEFAULT 'manual';   -- 'auto'（自动扫描）/ 'manual'（手动）
ALTER TABLE records ADD COLUMN status TEXT DEFAULT 'confirmed';-- 'pending' / 'confirmed' / 'rejected'
ALTER TABLE records ADD COLUMN drops TEXT;                     -- JSON 数组，原始物品名 ["弥狂护腕","星雷陨铁"]
ALTER TABLE records ADD COLUMN jcl_files TEXT;                 -- 关联 JCL 文件名列表 JSON（自动扫描溯源）
CREATE INDEX IF NOT EXISTS idx_records_status ON records(status);
CREATE INDEX IF NOT EXISTS idx_records_source ON records(source);
```

**字段决策**：
- `source`：区分自动扫描与手动记录，默认 `manual` 保持现有数据兼容
- `status`：标记待确认状态。现有手动记录默认 `confirmed`；自动扫描记录初始为 `pending`
- `drops`：JSON 数组存原始物品名（不映射装备 ID），延迟映射到 `equipments` 表获取图标/装分
- `jcl_files`：自动扫描记录的溯源信息，手动记录为 NULL
- 复用现有 `bossIds` / `bossNames` 字段标记 BOSS 击杀，参与 CD 计算

**幂等设计**：
- 自动扫描时按 `(account_id, raid_name, date, source='auto')` 查询是否已存在
- 已存在且 `status='pending'` → 覆盖 drops / bossIds
- 已存在且 `status='confirmed'` → 不覆盖（用户已确认，保留用户修改）
- 不存在 → 插入新记录，`status='pending'`

**RaidRecord 类型扩展**（types.ts）：
```ts
interface RaidRecord {
  // ... 现有字段
  source?: 'auto' | 'manual';     // 默认 'manual'
  status?: 'pending' | 'confirmed' | 'rejected'; // 默认 'confirmed'
  drops?: string[];               // 原始物品名列表
  jclFiles?: string[];            // 关联 JCL 文件名
}
```

### 2. 触发与循环扫描

复用现有 `useActivePoller` 全局轮询器（30s/60s/120s 自适应）：

```
useActivePoller 检测到在线角色 (active/recent/idle)
       │
       ▼
触发该角色的掉落扫描
       │
       ▼
应用未退出 → 持续循环扫描
应用退出 → 关闭所有进行中实例并入库
```

扫描范围：仅扫描在线角色所属账号目录下的 `combat_logs/*.jcl` 和 `chat_log/*.v2.db`。

### 3. 副本实例状态机

副本实例在内存中维护，end_time 动态确定。

```
进行中 (in-memory)
   │ account_id, raid_name, start_time
   │ bosses_killed: []  ← 持续追加
   │ last_jcl_time: ...  ← 每张 JCL 更新
   │
   ├── 检测到新 JCL
   │     ├── 属于当前实例 (同副本 + BOSS未全杀 + 间隔<30min)
   │     │     → 更新 bosses_killed, last_jcl_time
   │     │
   │     └── 不属于当前实例
   │           → 关闭当前实例: end_time = 新JCL时间戳
   │           → 入库 + 开始新实例缓存
   │
   ├── 角色离线
   │     → 关闭当前实例: end_time = 离线时间
   │     → 入库
   │
   └── 应用退出
         → 关闭所有进行中实例: end_time = 当前时间
         → 入库 (保留 pending 状态)
```

**副本实例关闭条件**（任一满足即关闭）：
1. 出现非当前副本实例的 JCL：
   - 不同副本名（如从阆风悬城 → 天龙寺）
   - 同副本名但属于新实例（BOSS 已全杀后再出现该副本第一个 BOSS，或时间间隔 > 30min）
2. 角色退出登录（useActivePoller 检测到 active → offline）
3. 应用退出

**end_time 取值**：
- 条件 1 触发：`end_time = 下一张非当前实例 JCL 的时间戳`
- 条件 2 触发：`end_time = 角色离线检测时间`
- 条件 3 触发：`end_time = 当前时间`

### 4. 入库流程

副本实例关闭时触发入库，直接写入 `records` 表：

```
1. 用副本实例时间窗口 [start_time, end_time] 过滤 chatlog MSG_TEAM
2. 正则提取物品名: \[([^\]]+)\]\s*\d+金\s*分配给\s*\[([^\]]+)\]
3. 去重物品名列表
4. 将 BOSS 名列表映射为 BOSS ID 列表（通过 raid_bosses 表）
5. 构造 RaidRecord:
   - id: crypto.randomUUID()
   - raidName: JCL 原始副本名（如 "25人英雄阆风悬城"）
   - accountId, roleId, date: start_time
   - bossIds, bossNames: 已击杀 BOSS 列表
   - source: 'auto'
   - status: 'pending'
   - drops: 物品名列表
   - jclFiles: 关联 JCL 文件名
   - goldIncome/goldExpense 等收支字段: 0 或空（待用户确认时补充）
6. UPSERT records 表:
   - 按 (account_id, raid_name, date, source='auto') 查询
   - 已存在且 status='confirmed' → 不覆盖（保留用户修改）
   - 已存在且 status='pending' → 覆盖 drops / bossIds
   - 不存在 → 插入，status='pending'
```

### 5. CD 计算与显示

**CD 计算修改**（`utils/bossCooldownManager.ts` + `utils/cooldownManager.ts`）：

现有 CD 计算只查 `records` 表。修改后需排除 `status='rejected'` 记录：

```ts
// bossCooldownManager.ts
const recordsInWindow = roleBossRecords.filter(record => {
  if (record.status === 'rejected') return false;  // 新增：排除已拒绝
  if (record.bossId === boss.id) return true;
  if (record.bossIds && record.bossIds.includes(boss.id)) return true;
  return false;
});
```

```ts
// cooldownManager.ts - calculateCooldown
const recordsInCycle = records.filter(r => {
  if (r.status === 'rejected') return false;  // 新增：排除已拒绝
  return /* 现有时间窗口过滤 */;
});
```

**pending 记录共享 CD**：`status='pending'` 记录正常参与 CD 计算，标记 BOSS 为已打。
用户拒绝后 `status='rejected'`，CD 自动释放。

**UI 显示**（RaidDetail.tsx）：
- pending 记录与手动记录统一显示三态（未清/部分清/完全清）
- pending 记录的副本卡片添加"待确认"视觉标记（如小标签或边框样式）
- 提供"确认"/"拒绝"入口
- 确认后 status='confirmed'，可补充收支信息
- 拒绝后 status='rejected'，CD 释放

### 6. 与现有系统的关系

| 系统 | 关系 |
|------|------|
| `records` 表 | 扩展 4 个字段，自动扫描记录与手动记录共存 |
| `useActivePoller` | 复用，在线时触发扫描 |
| `active_detector.rs` | 复用，离线检测触发入库 |
| `equipments` 表 | 延迟映射，展示时查图标/装分 |
| `raids` / `raid_bosses` 表 | 查询副本 BOSS 总数与名称，BOSS 名→ID 映射 |
| `calculateCooldown` / `calculateBossCooldowns` | 仅增加 rejected 过滤，逻辑不变 |
| RaidDetail.tsx | 增加 pending 标记与确认/拒绝入口 |

## 关键设计决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 表设计 | 扩展 records 表，不新建独立表 | 共享 CD，CD 逻辑几乎不变 |
| 主键 | 复用 records.id (UUID) | 与手动记录一致 |
| 幂等键 | (account_id, raid_name, date, source='auto') | 区分自动与手动记录 |
| ID 映射 | 仅记物品名，延迟映射 | 避免丢失陨铁/材料/旧赛季装备 |
| 数据源 | 仅 MSG_TEAM | 用户明确选择 |
| 触发 | 复用 useActivePoller | 在线时自动扫描 |
| 入库时机 | 副本实例关闭时 | 避免拍装备时间窗口截断 |
| end_time | 动态确定 | 延迟到下个非当前实例 JCL 或离线 |
| CD 共享 | pending 记录参与 CD 计算 | 用户明确要求共享 CD |
| rejected 处理 | 软删除（status='rejected'），CD 释放 | 保留溯源，可恢复 |
| UI 显示 | pending 与手动记录统一三态显示 | 用户明确要求相同显示 |

## 技术依赖

### 已有能力
- `encoding_rs`（Cargo.toml 已有）：JCL 文件名 GBK 解码
- `rusqlite`（已有）：读写 chatlog + records 表
- `useActivePoller`（前端已有）：在线检测 + 触发扫描
- `active_detector.rs`（已有）：离线检测
- `equipments` 表 + `jx3BoxApi.ts`（已有）：装备数据
- `calculateCooldown` / `calculateBossCooldowns`（已有）：CD 计算

### 新增内容
- Rust 模块 `drop_scanner.rs`：
  - JCL 文件名解析（GBK 解码 + 正则提取副本名/BOSS名/时间戳）
  - 副本实例聚类与状态机
  - chatlog MSG_TEAM 过滤与解析
  - BOSS 名→ID 映射（通过 raid_bosses 表）
  - 构造 RaidRecord 并 UPSERT records 表
- 数据库迁移：records 表新增 4 字段 + 2 索引（v12 迁移）
- Tauri 命令：
  - `scan_raid_drops(account_id)`：手动触发扫描（调试用）
  - `confirm_record(record_id)`：确认 pending 记录
  - `reject_record(record_id)`：拒绝 pending 记录（CD 释放）
- 前端 Hook `useDropScanner`：
  - 订阅 `useActivePoller` 的在线状态
  - 在线时调用后端扫描命令
  - 离线时触发入库
- 前端 UI 修改：
  - `bossCooldownManager.ts` / `cooldownManager.ts`：增加 rejected 过滤
  - `RaidDetail.tsx`：pending 记录视觉标记 + 确认/拒绝入口
  - `types.ts`：RaidRecord 增加 source/status/drops/jclFiles 字段

## 实施步骤

1. **数据库迁移**：records 表新增 source/status/drops/jcl_files 字段 + 索引（v12 迁移）
2. **类型扩展**：RaidRecord 增加 source/status/drops/jclFiles 字段
3. **Rust 后端**：
   - 新建 `drop_scanner.rs` 模块
   - 实现 JCL 文件名解析（复用现有 GBK 解码能力）
   - 实现副本实例状态机
   - 实现 chatlog MSG_TEAM 过滤
   - 实现 BOSS 名→ID 映射
   - 注册 Tauri 命令（scan_raid_drops / confirm_record / reject_record）
4. **CD 计算修改**：
   - `bossCooldownManager.ts`：增加 rejected 过滤
   - `cooldownManager.ts`：增加 rejected 过滤
5. **前端 Hook**：
   - 新建 `useDropScanner` Hook
   - 订阅 `useActivePoller`
   - 调用扫描命令
6. **UI 修改**：
   - RaidDetail.tsx：pending 记录视觉标记 + 确认/拒绝入口
7. **测试验证**：
   - 用实际 chatlog + JCL 样本验证解析
   - 验证副本实例聚类与时间窗口
   - 验证 UPSERT 幂等性
   - 验证离线入库时机
   - 验证 pending 记录共享 CD
   - 验证 rejected 后 CD 释放

## 防乱码注意

- chatlog 的 `text` 字段为 UTF-8（已验证），可直接读取
- JCL 文件名为 GBK 编码（项目记忆），需用 `encoding_rs` 解码
- 数据库写入中文内容时，rusqlite 默认 UTF-8，无需特殊处理
- 日志文案遵循 AGENTS.md 中文规范
