# 实现计划：自动扫描历史遗留 scanning 记录复核

日期：2026-08-09
关联设计：`docs/plans/2026-08-09-stale-scanning-verify-design.md`

## 实现步骤

### 步骤 1：新增 `query_stale_records_for_verify` 函数

**文件**：`src-tauri/src/mingyi/drop_scanner.rs`
**位置**：在 `scan_all_active_raid_drops_internal`（行 3542）之前
**作用**：查询 DB 中需要复核的记录（历史 scanning + 无掉落无工资的历史 pending）

```rust
struct StaleRecord {
    id: String,
    account_id: String,
    data: String,
    drops: String,
    created_at: i64,
    status: String,
}

fn query_stale_records_for_verify(conn: &Connection, cutoff_ms: i64) -> Result<Vec<StaleRecord>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, account_id, data, drops, created_at, status FROM records
         WHERE created_at < ?1 AND (
           status = 'scanning'
           OR (status = 'pending' AND drops = '[]' AND json_extract(data, '$.goldIncome') = 0)
         )"
    ).map_err(|e| format!("查询待复核记录失败: {}", e))?;

    let records = stmt.query_map(params![cutoff_ms], |row| {
        Ok(StaleRecord {
            id: row.get(0)?,
            account_id: row.get(1)?,
            data: row.get(2)?,
            drops: row.get(3)?,
            created_at: row.get(4)?,
            status: row.get(5)?,
        })
    }).map_err(|e| format!("遍历待复核记录失败: {}", e))?
      .filter_map(|r| r.ok())
      .collect();

    Ok(records)
}
```

**验证**：`cargo check` 通过；手动构造 scanning 和空 pending 记录，查询能返回正确结果。

### 步骤 2：新增 `find_next_jcl_time_after` 函数

**文件**：`src-tauri/src/mingyi/drop_scanner.rs`
**位置**：在 `scan_jcl_files`（行 345）之后
**作用**：查找账号 combat_logs 目录中 timestamp > after_ms 的最小 JCL 时间戳

```rust
fn find_next_jcl_time_after(account_dir: &Path, after_ms: i64) -> Result<Option<i64>, String> {
    let combat_logs_dir = account_dir.join("combat_logs");
    if !combat_logs_dir.exists() {
        return Ok(None);
    }

    let mut next: Option<i64> = None;
    for entry in std::fs::read_dir(&combat_logs_dir).map_err(|e| {
        format!("读取 combat_logs 目录失败: {} - {}", combat_logs_dir.display(), e)
    })? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jcl") {
            continue;
        }
        let file_name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if let Some(jcl) = parse_jcl_filename(file_name) {
            if jcl.timestamp > after_ms {
                next = Some(next.map_or(jcl.timestamp, |n| n.min(jcl.timestamp)));
            }
        }
    }
    Ok(next)
}
```

**验证**：`cargo check` 通过；单元测试构造目录 + JCL 文件，验证返回正确的最小时间戳。

### 步骤 3：在 `scan_all_active_raid_drops_internal` 插入复核阶段

**文件**：`src-tauri/src/mingyi/drop_scanner.rs`
**位置**：`scan_all_active_raid_drops_internal`（行 3542），在 `process_start_ms` 计算（行 3555）之后、`active_roles` 过滤（行 3573）之前

**改动**：

```rust
// 行 3555 之后新增复核阶段
let process_start_ms = (active_result.jx3_start_time_unix as i64) * 1000;

// ===== 新增：复核历史记录（scanning + 无掉落无工资的 pending）=====
let verify_started_at = std::time::Instant::now();
let verify_cutoff_ms = process_start_ms; // 本次 JX3 启动前的记录

let verify_result = verify_stale_records(
    verify_cutoff_ms,
    &game_dir,
    preloaded_raids.as_deref(), // 注意：preloaded_raids 需提前到此点之前加载
);

match &verify_result {
    Ok(stats) => {
        if stats.total > 0 {
            log::info!(
                "[DropScanner] 复核阶段完成：共 {} 条, 翻转/补充 {} 条, 保留原数据 {} 条, 保留原状 {} 条, 失败 {} 条, 耗时 {}ms",
                stats.total, stats.flipped, stats.preserved, stats.kept, stats.failed,
                verify_started_at.elapsed().as_millis()
            );
        }
    }
    Err(e) => {
        log::warn!("[DropScanner] 复核阶段失败，继续本次会话扫描: {}", e);
    }
}
// ===== 复核阶段结束 =====

// 原有逻辑：计算 CD 窗口、筛选 active_roles、扫描...
```

**注意**：`preloaded_raids` 当前在行 3611-3629 加载，需要将其提前到复核阶段之前。调整顺序：先加载 `preloaded_raids`，再执行复核，再执行本次会话扫描。

### 步骤 4：新增 `verify_stale_records` 和 `verify_single_record` 函数

**文件**：`src-tauri/src/mingyi/drop_scanner.rs`
**位置**：在 `scan_all_active_raid_drops_internal` 之前
**作用**：复核历史记录的核心逻辑，含"按需保留原数据"

```rust
struct VerifyStats {
    total: usize,
    flipped: usize,           // scanning→pending 或补充了数据
    preserved: usize,         // 保留了原数据（新扫描未识别到掉落/工资）
    kept: usize,              // 保留原状（pending 空记录仍空）
    failed: usize,
}

fn verify_stale_records(
    cutoff_ms: i64,
    game_dir: &str,
    preloaded_raids: Option<&[RaidEntry]>,
) -> Result<VerifyStats, String> {
    let conn = db::init_db()?;
    let stale_records = query_stale_records_for_verify(&conn, cutoff_ms)?;
    drop(conn);

    if stale_records.is_empty() {
        return Ok(VerifyStats { total: 0, flipped: 0, preserved: 0, kept: 0, failed: 0 });
    }

    log::info!("[DropScanner] 轮询开始：发现 {} 条历史记录待复核", stale_records.len());

    let game_path = PathBuf::from(game_dir);
    let accounts_base = game_path.join(MINGYI_ACCOUNTS_BASE_PATH);

    let mut stats = VerifyStats { total: stale_records.len(), flipped: 0, preserved: 0, kept: 0, failed: 0 };

    for record in &stale_records {
        match verify_single_record(record, &accounts_base, preloaded_raids) {
            Ok(VerifyOutcome::Flipped) => stats.flipped += 1,
            Ok(VerifyOutcome::PreservedOriginal) => stats.preserved += 1,
            Ok(VerifyOutcome::Kept) => stats.kept += 1,
            Err(e) => {
                log::warn!("[DropScanner] 复核记录 [{}] 失败: {}", record.id, e);
                stats.failed += 1;
            }
        }
    }

    Ok(stats)
}

#[derive(Debug)]
enum VerifyOutcome {
    Flipped,           // scanning→pending 或 pending 补充了数据
    PreservedOriginal, // 新扫描未识别到掉落/工资，保留了原数据
    Kept,              // pending 空记录仍空，无变化
}

fn verify_single_record(
    record: &StaleRecord,
    accounts_base: &Path,
    preloaded_raids: Option<&[RaidEntry]>,
) -> Result<VerifyOutcome, String> {
    // 1. 保存原记录的 data 和 drops（用于按需保留）
    let original_data = record.data.clone();
    let original_drops = record.drops.clone();
    let original_has_data = has_drops_or_salary(&original_data, &original_drops);
    let was_scanning = record.status == "scanning";

    // 2. 解析 data 字段
    let data: serde_json::Value = serde_json::from_str(&record.data)
        .map_err(|e| format!("解析 data JSON 失败: {}", e))?;

    let scan_start_ms = data["date"].as_i64().unwrap_or(0);
    if scan_start_ms == 0 {
        return Err("data.date 为 0，无法确定扫描起始时间".to_string());
    }

    // 3. 从 jclFiles 解析 last_jcl_time
    let last_jcl_time = data["jclFiles"]
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .filter_map(|f| f.as_str())
                .filter_map(|name| parse_jcl_filename(name).map(|j| j.timestamp))
                .max()
        })
        .unwrap_or(scan_start_ms);

    // 4. 查找下一个 JCL 时间戳
    let account_dir = accounts_base.join(format!("{}@zhcn_hd", record.account_id));
    if !account_dir.exists() {
        return Err(format!("账号目录不存在: {}", account_dir.display()));
    }

    let next_jcl_time = find_next_jcl_time_after(&account_dir, last_jcl_time)?;

    // 5. 动态计算 scan_end_ms
    let scan_end_ms = if let Some(next_t) = next_jcl_time {
        next_t
    } else {
        last_jcl_time + 2 * 3600 * 1000
    };

    log::info!(
        "[DropScanner] 复核记录 [{}]: account={}, status={}, scan_range=[{},{}], next_jcl={:?}",
        record.id, record.account_id, record.status, scan_start_ms, scan_end_ms,
        next_jcl_time.map(|t| t.to_string()).unwrap_or("None".to_string())
    );

    // 6. 复用 scan_raid_drops_with_raids 重扫
    let role_online = is_role_online(&account_dir, true);
    let result = scan_raid_drops_with_raids(
        &record.account_id,
        true,           // jx3_running
        role_online,
        0,              // process_start_ms=0，绕过 mtime 过滤
        scan_start_ms,
        scan_end_ms,
        preloaded_raids,
    );

    match result {
        Ok(_instance_count) => {
            // 7. 扫描后查询新记录的 data 和 drops
            let conn = db::init_db()?;
            let (new_data, new_drops, new_status): (String, String, String) = conn.query_row(
                "SELECT data, drops, status FROM records WHERE id = ?1",
                params![&record.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).map_err(|e| format!("查询复核后记录失败: {}", e))?;

            let new_has_data = has_drops_or_salary(&new_data, &new_drops);

            // 8. 按需保留原数据
            //    新扫描未识别到掉落/工资，但原记录有 → 恢复原 data/drops，只改 status
            if !new_has_data && original_has_data {
                let now_ms = chrono::Local::now().timestamp_millis();
                // 对于 scanning 记录，翻转 status='pending'
                // 对于 pending 记录，status 保持 'pending'（恢复原数据即可）
                let target_status = if was_scanning { "pending" } else { "pending" };
                conn.execute(
                    "UPDATE records SET data = ?1, drops = ?2, status = ?3, updated_at = ?4 WHERE id = ?5",
                    params![original_data, original_drops, target_status, now_ms, &record.id],
                ).map_err(|e| format!("恢复原数据失败: {}", e))?;

                log::info!(
                    "[DropScanner] 复核记录 [{}]: 新扫描未识别到掉落/工资，保留原数据，status='{}'",
                    record.id, target_status
                );
                return Ok(VerifyOutcome::PreservedOriginal);
            }

            // 9. 新扫描有数据或原记录也无数据 → 接受 upsert 的结果
            let outcome = if was_scanning && new_status == "pending" {
                VerifyOutcome::Flipped  // scanning→pending
            } else if !was_scanning && new_has_data {
                VerifyOutcome::Flipped  // pending 补充了数据
            } else if !was_scanning && !new_has_data {
                VerifyOutcome::Kept     // pending 空记录仍空
            } else {
                VerifyOutcome::Kept     // scanning 保留 scanning
            };

            log::info!(
                "[DropScanner] 复核记录 [{}]: {} (status={})",
                record.id,
                match &outcome {
                    VerifyOutcome::Flipped => "翻转/补充",
                    VerifyOutcome::PreservedOriginal => "保留原数据",
                    VerifyOutcome::Kept => "保留原状",
                },
                new_status
            );
            Ok(outcome)
        }
        Err(e) => Err(format!("重扫失败: {}", e)),
    }
}

/// 判断记录是否有掉落或工资数据
fn has_drops_or_salary(data: &str, drops: &str) -> bool {
    let drops_nonempty = drops != "[]" && !drops.is_empty();
    let gold_income = serde_json::from_str::<serde_json::Value>(data)
        .ok()
        .and_then(|v| v["goldIncome"].as_i64())
        .unwrap_or(0);
    let gold_expense = serde_json::from_str::<serde_json::Value>(data)
        .ok()
        .and_then(|v| v["goldExpense"].as_i64())
        .unwrap_or(0);
    drops_nonempty || gold_income > 0 || gold_expense > 0
}
```

**验证**：`cargo check` 通过；构造遗留 scanning 记录和无掉落无工资的 pending 记录，启动 JX3，等待 30 秒轮询，检查日志和 DB 状态变化。

### 步骤 5：调整 `preloaded_raids` 加载顺序

**文件**：`src-tauri/src/mingyi/drop_scanner.rs`
**位置**：`scan_all_active_raid_drops_internal`（行 3542）

**改动**：将 `preloaded_raids` 加载（行 3611-3629）提前到复核阶段之前。

原顺序：
1. 获取 active_result
2. 计算 process_start_ms、CD 窗口
3. 筛选 active_roles
4. 加载 preloaded_raids
5. 遍历 active_roles 扫描

新顺序：
1. 获取 active_result
2. 计算 process_start_ms、CD 窗口
3. **加载 preloaded_raids**（提前）
4. **复核历史 scanning 记录**（新增）
5. 筛选 active_roles
6. 遍历 active_roles 扫描

### 步骤 6：编译验证与测试

```bash
cd src-tauri && cargo build
```

**验证项**：
1. `cargo build` 无告警无错误
2. `cargo test` 已有测试不被破坏
3. 手动测试 - 历史 scanning 复核：
   - 构造遗留 scanning 记录（`status='scanning'`, `created_at` 早于 JX3 启动时间）
   - 启动 JX3，等待 30 秒轮询
   - 检查日志出现 `[DropScanner] 轮询开始：发现 N 条历史记录待复核`
   - 检查 DB 中该记录 status 升级为 pending（或保留 scanning 并有合理原因）
   - 检查 UI 中 pending 记录可确认
4. 手动测试 - 无掉落无工资的 pending 补充：
   - 构造空 pending 记录（`status='pending'`, `drops='[]'`, `goldIncome=0`, `created_at` 早于 JX3 启动时间）
   - 启动 JX3，等待 30 秒轮询
   - 检查 DB 中该记录是否补充了掉落/工资（若 JCL/chatlog 可识别）
5. 手动测试 - 按需保留原数据：
   - 构造有掉落的 scanning 记录，删除部分 JCL/chatlog 文件使新扫描无法识别掉落
   - 启动 JX3，等待 30 秒轮询
   - 检查 DB 中该记录的 drops 是否保留原数据，status 是否翻转为 pending

### 步骤 7：更新 README 和 Release Notes

- `README.md`：在自动扫描相关章节补充"历史记录复核"说明
- `release-notes/v<next>.md`：新增"自动扫描历史遗留记录复核"条目

## 风险检查清单

- [ ] 复核阶段失败不影响本次会话扫描（`verify_result` 用 `match` 处理，不 `?` 返回）
- [ ] 单条记录复核失败不阻塞其他记录（`verify_single_record` 返回 Err 只记日志）
- [ ] `preloaded_raids` 提前加载不影响原有逻辑（只是顺序调整，所有权不变）
- [ ] `find_next_jcl_time_after` 目录不存在时返回 `Ok(None)` 不报错
- [ ] `data.date` 为 0 时 `verify_single_record` 返回 Err 跳过该记录
- [ ] 不修改 records 表 data 字段 JSON 结构（完全从已有字段解析）
- [ ] 前端 `useDropScanner.ts` 无需改动
- [ ] 按需保留原数据：新扫描未识别到掉落/工资但原记录有时，恢复原 data/drops
- [ ] 空 pending 记录复核后仍空时，保留原状不报错
- [ ] 复核查询使用 `drops = '[]'` 精确匹配空数组字符串（与 INSERT 时序列化格式一致）

## 不做的事

- 不新增 Tauri 命令（复核在后端 `scan_all_active_raid_drops_internal` 内部完成，前端无感知）
- 不修改 records 表 schema
- 不修改 data 字段 JSON 结构
- 不处理 JX3 未运行时的复核（等用户启动 JX3 后第一次轮询处理）
- 不新增前端 UI（复用现有 pending/scanning 状态展示）
