# 自动扫描历史遗留 scanning 记录复核设计

日期：2026-08-09
状态：已确认，待实现

## 背景与问题

### 当前自动扫描的"只处理本次会话"机制

自动扫描逻辑只处理本次 JX3 进程启动后的任务，由三层过滤保证：

1. **账号活跃过滤**（`active_detector.rs:471-473`）：账号 mtime 早于 JX3 进程启动 → 判为 Idle → 不扫描
2. **JCL 文件 mtime 过滤**（`drop_scanner.rs:395-408`）：`since_ms = process_start_ms`，早于进程启动的 JCL 全部跳过
3. **CD 窗口兜底**：本周一 07:00 ~ 下周一 07:00

`process_start_ms` 来源：
- 轮询扫描：`scan_all_active_raid_drops_internal` 行 3555 → `active_result.jx3_start_time_unix * 1000`
- 单账号扫描：`scan_raid_drops` 行 3479-3483 → `runtime_status.matched_process.start_time_unix * 1000`
- 退出扫描：`useDropScanner.ts` 行 289 → `sessionStartMs`

### 历史 scanning 记录的生命周期缺口

records 表中 `status='scanning'` 记录（副本进行中，UI 锁定不可确认）的处理路径有限：

| 场景 | 处理 |
|------|------|
| 同 CD 窗口 + 同账号 + 同副本 + 同 role_id | 新扫描 UPDATE 覆盖，可升级 scanning→pending |
| 同 CD 窗口 + 已有 confirmed/rejected | 跳过新扫描 |
| 同 CD 窗口 + 已有手工 confirmed | DELETE 多余 pending 或跳过创建 |
| **跨 CD 窗口** | **不处理，一直卡住** |
| **应用被强关 / JX3 崩溃 / 电脑重启** | **不处理，一直卡住** |

**核心问题 1**：上次 JX3 会话产生的 scanning 记录在异常退出场景下会一直卡在 DB 中，用户在 UI 上无法确认（scanning 状态被锁定），只能等下次同账号同副本的扫描 UPDATE 覆盖。如果用户不再打这个副本，记录永久卡住。

**核心问题 2**：历史 pending 记录如果因上次扫描时 chatlog 未 flush、文件被删等原因**未识别到掉落和工资**（drops='[]' 且 goldIncome=0），会成为孤儿记录：
- 本次会话扫描不处理（process_start_ms 过滤掉历史 JCL）
- scanning 复核不处理（它已经是 pending 状态）
- 用户在 UI 上看到一条空的 pending 记录，无法自动补充掉落和工资

**覆盖缺口**：

| 记录类型 | 本次会话扫描 | scanning 复核（原方案） | 问题 |
|----------|-------------|------------------------|------|
| 本次会话的 scanning | ✅ 处理 | ❌ 不处理（cutoff 隔离） | 无 |
| 历史 scanning | ❌ 不处理 | ✅ 处理 | 无 |
| 历史 pending（有掉落/工资） | ❌ 不处理 | ❌ 不处理 | 无需处理 |
| **历史 pending（无掉落无工资）** | ❌ 不处理 | ❌ 不处理 | **孤儿记录，永远无法补充** |

本方案扩展复核范围，同时覆盖"历史 scanning"和"历史无掉落无工资的 pending"两类记录。

## 设计方案

### 核心思路

**不新增独立触发点，融入 30 秒轮询扫描流程**。在每次轮询的本次会话扫描之前，先复核历史 scanning 记录。

### 整体流程

```
30 秒轮询触发
  ├─ [复核阶段·新增] 处理历史 scanning 记录
  │   ├─ 查询 status='scanning' AND created_at < cutoff_ms 的记录
  │   ├─ 对每条记录，复用 scan_raids_in_range 重扫其副本实例时间范围
  │   └─ upsert 自动 UPDATE 覆盖原 scanning 记录（→pending 或保留 scanning）
  └─ [原有逻辑] 本次会话扫描
      └─ active_roles 过滤 + scan_raid_drops_with_raids
```

### 触发时机

- 30 秒轮询自然触发（`useDropScanner.ts:134`）
- 轮询只在 JX3 运行时启动，因此复核阶段**只在 JX3 运行时执行**
- 应用启动后若 JX3 已在运行，第一次轮询即处理遗留 scanning
- 应用启动时若 JX3 未运行，等用户启动 JX3 后第一次轮询处理（cutoff_ms = JX3 启动时间，所有遗留 scanning 都早于此，全部进入复核）
- 复用现有 `isScanningRef` 互斥锁，无需前端新增逻辑

## 详细设计

### 1. 筛选条件

复核阶段在 `scan_all_active_raid_drops_internal` 的 JX3 运行检查通过之后、本次会话扫描之前执行。此时 JX3 必然运行，`jx3_start_time_unix` 必然存在。

```rust
// cutoff_ms：本次 JX3 启动时间（毫秒）
// 轮询只在 JX3 运行时触发，jx3_start_time_unix 必然存在
let cutoff_ms = (active_result.jx3_start_time_unix as i64) * 1000;

// 查询需要复核的记录：历史 scanning + 无掉落无工资的历史 pending
let stale_records = query(
    "SELECT id, account_id, data, created_at FROM records
     WHERE created_at < ?1 AND (
       status = 'scanning'
       OR (status = 'pending' AND drops = '[]' AND json_extract(data, '$.goldIncome') = 0)
     )",
    cutoff_ms
);
```

**复核范围说明**：

| 记录类型 | 是否复核 | 理由 |
|----------|----------|------|
| 历史 scanning（本次启动前） | ✅ | 上次会话未收尾，需判定是否已结束 |
| 历史 pending（无掉落无工资） | ✅ | 上次扫描可能因 chatlog 未 flush 等原因漏识别，需补充 |
| 历史 pending（有掉落或工资） | ❌ | 数据完整，无需复核 |
| 历史 confirmed/rejected | ❌ | 已终态，不处理 |
| 本次会话产生的记录 | ❌ | `created_at < cutoff_ms` 隔离，交给原轮询逻辑 |

**关键隔离**：`created_at < cutoff_ms` 确保本次 JX3 启动后产生的记录（用户正在打的副本）100% 交给原有轮询逻辑，不被复核干扰。

### 2. 单条记录处理

对每条需要复核的记录，复用 `scan_raid_drops_with_raids` 完整逻辑重扫其副本实例时间范围。

**时间范围来源**（从 records 表 data 字段已有数据解析，不新增字段）：
- `scan_start_ms` = `data.date`（副本开始时间，= 首个成功击杀 JCL 的文件名时间）
- `last_jcl_time` = `max(parse_jcl_filename(f).timestamp for f in data.jclFiles)`（从成功击杀 JCL 文件名解析最大时间戳）；若 `jclFiles` 为空，回退到 `data.date`

```rust
for record in stale_records {
    let data: serde_json::Value = serde_json::from_str(&record.data)?;
    let scan_start_ms = data["date"].as_i64().unwrap_or(0);

    // 从 jclFiles 解析最后一个成功击杀 JCL 的时间戳
    let last_jcl_time = data["jclFiles"]
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .filter_map(|f| f.as_str())
                .filter_map(|name| parse_jcl_filename(name).map(|j| j.timestamp))
                .max()
        })
        .unwrap_or(scan_start_ms);

    // 查找该账号在 last_jcl_time 之后的下一个 JCL 时间戳
    let account_dir = accounts_base.join(format!("{}@zhcn_hd", record.account_id));
    let next_jcl_time = find_next_jcl_time_after(&account_dir, last_jcl_time)?;

    // scan_end_ms 动态计算（见下方规则）
    let scan_end_ms = if let Some(next_t) = next_jcl_time {
        next_t
    } else {
        last_jcl_time + 2 * 3600 * 1000
    };

    // 复用 scan_raid_drops_with_raids，与定期自动扫描逻辑一致
    // - jx3_running=true：复核只在 JX3 运行时触发
    // - role_online：用 is_role_online 实际检测（基于 chatlog mtime）
    // - process_start_ms=0：绕过 mtime 过滤，必须扫到历史 JCL
    let role_online = is_role_online(&account_dir, true);
    let _ = scan_raid_drops_with_raids(
        &record.account_id,
        true,           // jx3_running
        role_online,    // 基于 chatlog mtime 实际检测
        0,              // process_start_ms=0，绕过 mtime 过滤
        scan_start_ms,
        scan_end_ms,
        preloaded_raids.as_deref(),
    );
}
```

**说明**：`jclFiles` 只包含成功击杀的 JCL（不含拉托 JCL），解析出的 `last_jcl_time` 可能比实际 `instance.last_jcl_time` 略早。但拉托 JCL 通常不产生掉落和工资，对复核判定影响可忽略。

### 2.1 按需保留原数据（防止数据丢失）

`upsert_raid_drop_record` 的 UPDATE 会用新扫描结果覆盖原记录的 `data`、`drops`、`jcl_files`。如果复核扫描因 JCL/chatlog 文件被删、解析失败等原因未识别到掉落和工资，直接覆盖会导致原记录已有的掉落和工资数据丢失。

**处理策略**：扫描后比较新旧数据，新扫描未识别到掉落和工资但原记录有 → 恢复原 `data`/`drops`，只改 `status`。

| 原记录状态 | 原记录数据 | 新扫描结果 | 处理 |
|-----------|-----------|-----------|------|
| scanning | 有掉落/工资 | 识别到 | 正常覆盖（补充新数据） |
| scanning | 有掉落/工资 | 未识别到 | **恢复原 data/drops，只翻 status='pending'** |
| scanning | 无掉落/工资 | 识别到 | 正常覆盖（补充数据） |
| scanning | 无掉落/工资 | 未识别到 | 正常覆盖（都空，翻 status） |
| pending(空) | 无掉落/工资 | 识别到 | 正常覆盖（补充数据） |
| pending(空) | 无掉落/工资 | 未识别到 | 保留原状（都空，无变化） |

**实现逻辑**（在 `verify_single_record` 中）：

```rust
// 1. 扫描前保存原记录的 data 和 drops
let original_data = record.data.clone();
let original_drops = query_drops_by_id(&conn, &record.id)?;
let original_has_data = has_drops_or_salary(&original_data, &original_drops);

// 2. 调用 scan_raid_drops_with_raids 重扫（upsert 会 UPDATE 覆盖）
let _ = scan_raid_drops_with_raids(...);

// 3. 扫描后查询新记录的 data 和 drops
let new_drops = query_drops_by_id(&conn, &record.id)?;
let new_data = query_data_by_id(&conn, &record.id)?;
let new_has_data = has_drops_or_salary(&new_data, &new_drops);

// 4. 按需保留原数据
if !new_has_data && original_has_data {
    // 新扫描未识别到掉落/工资，但原记录有 → 恢复原 data/drops，只改 status
    UPDATE records SET data = original_data, drops = original_drops,
                       status = 'pending', updated_at = now WHERE id = record.id;
    log::info!("[DropScanner] 复核 [{}]: 新扫描未识别到掉落/工资，保留原数据，翻转 status='pending'", record.id);
    return Ok(VerifyOutcome::PreservedOriginal);
}

// 5. 新扫描有数据或原记录也无数据 → 接受 upsert 的结果

fn has_drops_or_salary(data: &str, drops: &str) -> bool {
    let drops_nonempty = drops != "[]" && !drops.is_empty();
    let gold_income = data["goldIncome"].as_i64().unwrap_or(0);
    let gold_expense = data["goldExpense"].as_i64().unwrap_or(0);
    drops_nonempty || gold_income > 0 || gold_expense > 0
}
```

### 3. scan_end_ms 动态计算规则

| 场景 | scan_end_ms | is_raid_complete 判定路径 | chatlog 扫描范围 |
|------|-------------|---------------------------|------------------|
| 后续有下一个 JCL（已切换副本） | `next_jcl_time` | 条件 5（副本切换，has_next_jcl） | 到切换点即可 |
| 后续无下一个 JCL（最后一个实例） | `last_jcl_time + 2h` | 条件 3/4/6（JX3 退出/离线/6h 兜底） | +2h 覆盖工资记录 |

**设计理由**：
- 有下一个 JCL 时，副本切换本身就是完成信号（条件 5），不需要 +2h 缓冲
- +2h 缓冲原本是为了兜底工资记录（chatlog 在 JX3 退出时才 flush），但副本已切换时工资记录在 last_jcl_time 附近就已发完
- 有下一个 JCL 时 scan_end_ms 更小，JCL 和 chatlog 扫描范围都更小，性能更好

### 4. 关键设计点

#### 复用自动扫描完整逻辑

- JCL 解析、聚类、掉落提取、工资计算、`is_raid_complete` 判定、`upsert_raid_drop_record` 写入全部复用
- 行为与定期自动扫描**完全一致**，只是时间窗口不同

#### UPDATE 覆盖由 upsert 自动完成

- 同 CD 窗口 + 同账号 + 同副本 + 同 role_id → `upsert_raid_drop_record` 的 UPDATE 分支（`drop_scanner.rs:2835-2850`）
- `is_raid_complete` 判定完成 → status 升级为 pending（UI 解锁可确认）
- 仍判定 scanning → 保留原状（极端情况，如 JCL 文件被删，等下次复核）

#### `process_start_ms = 0`：绕过 mtime 过滤

- 自动扫描正常流程按 `process_start_ms` 过滤掉历史 JCL（`drop_scanner.rs:395-408`）
- 复核必须扫到旧 JCL，传 0 绕过
- 时间范围本身（`scan_start_ms ~ scan_end_ms`）已限定范围

#### 复核失败不阻塞本次扫描

- 单条复核失败用 `let _ =` 忽略，记日志即可
- 复核阶段整体完成后再进入本次会话扫描

### 5. JX3 运行状态对判定的影响

复核只在 JX3 运行时触发（轮询触发），因此 `jx3_running` 恒为 true：

| 场景 | cutoff_ms | is_raid_complete 主路径 |
|------|-----------|-------------------------|
| JX3 运行中（唯一场景） | 本次启动时间 | 条件 5（切换）或条件 6（6h 兜底） |

**说明**：JX3 未运行时不触发复核（无轮询）。遗留 scanning 等待用户下次启动 JX3，第一次轮询即处理。

## 性能考量

- 历史 scanning 记录通常很少（个位数），每条只扫一个副本实例的 JCL（几 MB）
- 30 秒轮询间隔足够吸收额外开销
- 历史记录被翻转后，下次轮询 `query` 查不到，无额外开销
- 若某条记录反复无法翻转（极端情况），每次轮询都会重扫，但有 30 秒间隔限流

## 日志

```rust
log::info!("[DropScanner] 轮询开始：发现 {} 条历史记录待复核", stale_records.len());
// 每条复核后
log::info!("[DropScanner] 复核记录 [{}]: 翻转/补充 | 保留原数据 | 保留原状", record.id);
log::info!("[DropScanner] 复核阶段完成：共 {} 条, 翻转/补充 {} 条, 保留原数据 {} 条, 保留原状 {} 条, 失败 {} 条", ...);
```

前端 `useDropScanner.ts` 的 `[polling]` 日志无需改动，自然反映复核 + 本次扫描的总耗时和副本实例数。

## 代码改动范围

### Rust 后端（`src-tauri/src/mingyi/drop_scanner.rs`）

1. 修改 `scan_all_active_raid_drops_internal`：在本次会话扫描前插入复核阶段
2. 新增 `query_stale_records_for_verify(conn, cutoff_ms)`：查询历史 scanning + 无掉落无工资的 pending 记录（返回 id、account_id、data、drops、created_at、status）
3. 新增 `find_next_jcl_time_after(account_dir, after_ms)`：遍历 combat_logs 目录，用 `parse_jcl_filename` 解析文件名，返回 timestamp > after_ms 的最小时间戳
4. 新增 `verify_stale_records(cutoff_ms, game_dir, preloaded_raids)`：复核入口函数
5. 新增 `verify_single_record(record, accounts_base, preloaded_raids)`：单条记录复核，含"按需保留原数据"逻辑
6. 新增 `has_drops_or_salary(data, drops)`：判断记录是否有掉落或工资数据
7. 复用已有函数：`scan_raid_drops_with_raids`、`is_role_online`、`parse_jcl_filename`、`calculate_cd_window`

### 前端（`hooks/useDropScanner.ts`）

- **无需改动**，复用现有轮询触发和 `isScanningRef` 互斥锁

## 与现有逻辑的边界

| 场景 | 处理逻辑 |
|------|----------|
| 本次 JX3 会话产生的 scanning | 原有轮询扫描（`created_at >= cutoff_ms`，不进入复核） |
| 历史遗留的 scanning（本次启动前） | **本方案**：复核重扫，UPDATE 覆盖 |
| 历史 confirmed/rejected 记录 | 不受影响（只查 status='scanning'） |
| 跨 CD 窗口的 scanning | 自然处理：重扫时间范围内若有 JCL 会判定完成；无 JCL 则保留 |

## 验证方法

1. 构造遗留 scanning 记录：手动 INSERT 一条 `status='scanning'`、`created_at` 早于当前 JX3 启动时间的记录
2. 启动应用 + JX3，等待 30 秒轮询触发
3. 检查日志：应出现 `[DropScanner] 轮询开始：发现 1 条历史 scanning 记录待复核`
4. 检查 DB：该记录 status 应升级为 pending（或保留 scanning 并有合理原因）
5. 检查 UI：pending 记录可确认，scanning 记录仍锁定

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| 复核扫描与本次会话扫描并发 | 复用 `isScanningRef` 互斥锁，串行执行 |
| 复核扫描耗时过长影响轮询响应 | 历史 scanning 记录数量少（个位数），每条只扫一个副本实例，开销可控 |
| JCL 文件被删导致复核无法判定 | 保留 scanning 状态，下次复核重试，不报错 |
| next_jcl_time 查找增加目录遍历 | 复用 `scan_jcl_files` 的目录 mtime 快速跳过逻辑；只查时间戳不做完整解析 |
