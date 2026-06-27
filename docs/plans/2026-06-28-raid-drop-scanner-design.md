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

## 设计方案

### 1. 数据表 `raid_drop_records`

```sql
CREATE TABLE IF NOT EXISTS raid_drop_records (
    account_id TEXT NOT NULL,              -- 账号 UID（路径 {uid}@zhcn_hd）
    role_id TEXT,                          -- 角色 ID（chatlog ChatInfo.user_global_id）
    raid_name TEXT NOT NULL,               -- 副本名（与 raids.name 匹配，如"阆风悬城"）
    raid_display_name TEXT,                -- JCL 原始副本名（如"25人英雄阆风悬城"，展示用）
    start_time INTEGER NOT NULL,           -- 副本开始时间（首张 JCL 时间戳）
    end_time INTEGER NOT NULL,             -- 副本结束时间（动态确定，见下文）
    drops TEXT NOT NULL,                   -- JSON 数组，原始物品名 ["弥狂护腕","星雷陨铁"]
    jcl_files TEXT,                        -- 关联 JCL 文件名列表 JSON
    bosses_killed TEXT,                    -- 已击杀 BOSS 名列表 JSON
    bosses_total INTEGER,                  -- 该副本总 BOSS 数（从 raid_bosses 查）
    is_completed INTEGER DEFAULT 0,        -- 0=未完成(BOSS未全杀) / 1=已完成
    source TEXT NOT NULL DEFAULT 'auto',   -- 'auto'（自动扫描）/ 'manual'（手动）
    status TEXT NOT NULL DEFAULT 'pending',-- 'pending' / 'confirmed' / 'rejected'
    created_at TEXT,
    updated_at TEXT,
    PRIMARY KEY (account_id, raid_name, start_time)
);
CREATE INDEX idx_raid_drop_account ON raid_drop_records(account_id);
CREATE INDEX idx_raid_drop_status ON raid_drop_records(status);
CREATE INDEX idx_raid_drop_start_time ON raid_drop_records(start_time);
```

**主键设计**：复合自然主键 `(account_id, raid_name, start_time)`，保证重复扫描同一副本实例时 UPSERT 幂等。

**字段决策**：
- `drops` 存原始物品名（不映射装备 ID），延迟映射到 `equipments` 表获取图标/装分
- `source` 区分自动扫描与手动记录
- `status` 标记待确认状态，用户确认后才转为 `confirmed`
- `is_completed` 标记 BOSS 是否全部击杀，用于区分完整/不完整副本

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

副本实例关闭时触发入库：

```
1. 用副本实例时间窗口 [start_time, end_time] 过滤 chatlog MSG_TEAM
2. 正则提取物品名: \[([^\]]+)\]\s*\d+金\s*分配给\s*\[([^\]]+)\]
3. 去重物品名列表
4. UPSERT raid_drop_records:
   - 已存在且 status='confirmed' → 不覆盖 drops
   - 已存在且 status='pending' → 覆盖 drops
   - 不存在 → 插入，status='pending'
```

### 5. 与现有系统的关系

| 系统 | 关系 |
|------|------|
| `records` 表（手动收支） | 完全独立，不外键关联 |
| `useActivePoller` | 复用，在线时触发扫描 |
| `active_detector.rs` | 复用，离线检测触发入库 |
| `equipments` 表 | 延迟映射，展示时查图标/装分 |
| `raids` / `raid_bosses` 表 | 查询副本 BOSS 总数与名称 |
| UI（收支明细页等） | 暂不改动，后续迭代增加确认入口 |

## 关键设计决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 表粒度 | 副本实例（每场一条） | 符合"记录每个副本的掉落"需求 |
| 主键 | (account_id, raid_name, start_time) | 天然幂等，用户确认 |
| ID 映射 | 仅记物品名，延迟映射 | 避免丢失陨铁/材料/旧赛季装备 |
| 数据源 | 仅 MSG_TEAM | 用户明确选择 |
| 触发 | 复用 useActivePoller | 在线时自动扫描 |
| 入库时机 | 副本实例关闭时 | 避免拍装备时间窗口截断 |
| end_time | 动态确定 | 延迟到下个非当前实例 JCL 或离线 |
| UI | 暂不改动 | 用户明确要求，后续迭代 |
| 与 RaidRecord | 完全独立 | 避免耦合 |

## 技术依赖

### 已有能力
- `encoding_rs`（Cargo.toml 已有）：JCL 文件名 GBK 解码
- `rusqlite`（已有）：读写 chatlog + raid_drop_records
- `useActivePoller`（前端已有）：在线检测 + 触发扫描
- `active_detector.rs`（已有）：离线检测
- `equipments` 表 + `jx3BoxApi.ts`（已有）：装备数据

### 新增内容
- Rust 模块 `drop_scanner.rs`：
  - JCL 文件名解析（GBK 解码 + 正则提取副本名/BOSS名/时间戳）
  - 副本实例聚类与状态机
  - chatlog MSG_TEAM 过滤与解析
  - raid_drop_records 表 UPSERT
- 数据库迁移：新建 `raid_drop_records` 表（v12 迁移）
- Tauri 命令：
  - `scan_raid_drops(account_id)`：手动触发扫描（调试用）
  - `get_raid_drops(filter)`：查询掉落记录
  - `confirm_raid_drop(account_id, raid_name, start_time)`：确认记录
  - `reject_raid_drop(account_id, raid_name, start_time)`：拒绝记录
- 前端 Hook `useDropScanner`：
  - 订阅 `useActivePoller` 的在线状态
  - 在线时调用后端扫描命令
  - 离线时触发入库

## 实施步骤

1. **数据库迁移**：新增 `raid_drop_records` 表（v12 迁移）
2. **Rust 后端**：
   - 新建 `drop_scanner.rs` 模块
   - 实现 JCL 文件名解析（复用现有 GBK 解码能力）
   - 实现副本实例状态机
   - 实现 chatlog MSG_TEAM 过滤
   - 注册 Tauri 命令
3. **前端 Hook**：
   - 新建 `useDropScanner` Hook
   - 订阅 `useActivePoller`
   - 调用扫描命令
4. **测试验证**：
   - 用实际 chatlog + JCL 样本验证解析
   - 验证副本实例聚类与时间窗口
   - 验证 UPSERT 幂等性
   - 验证离线入库时机
5. **UI（后续迭代）**：
   - 收支明细页新增"副本掉落"标签
   - 待确认记录列表与确认/拒绝入口

## 防乱码注意

- chatlog 的 `text` 字段为 UTF-8（已验证），可直接读取
- JCL 文件名为 GBK 编码（项目记忆），需用 `encoding_rs` 解码
- 数据库写入中文内容时，rusqlite 默认 UTF-8，无需特殊处理
- 日志文案遵循 AGENTS.md 中文规范
