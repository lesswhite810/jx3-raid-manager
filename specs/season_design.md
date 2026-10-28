# 赛季数据模型设计文档

## 1. 概述

本文档描述 JX3 Raid Manager 赛季系统的数据模型设计，实现**版本 → 赛季 → 副本**的三层层级关系。

### 1.1 苍生铸世预置配置（v2.2.3）

- 新增「苍生铸世」版本与同名赛季，版本排序在「丝路风雨」之后；等级回到 50 不改变版本的先后顺序。
- 新赛季从 2026 年 10 月 29 日 7:00（Asia/Shanghai）开始，「暗影千机」在同一时刻结束。上线日期依据[官方更新公告](https://jx3.xoyo.com/launcher/update/latest.html)，7:00 沿用项目的赛季日界线。
- 「洛阳之战」静态等级为 50，配置包含 10 人普通、25 人普通、25 人英雄。等级沿用静态数据元信息，不新增数据库字段或等级筛选界面。
- 正式 BOSS 按顺序为「突利和顺 / 田承嗣 / 伊曼·寂夜 / 阿史那承庆 / 史朝义」。伊曼·逐焰属于伊曼·寂夜的分身，共用本体 CD，不单独加入 BOSS 配置或全通计数；逐焰命名的 JCL 交由既有的多 BOSS 判定处理，不为该副本新增专属分支。
- 已有数据库启动时补入新预置项，重复初始化不产生重复项；保留已有副本的启停状态与历史记录。旧赛季仅在起始日期仍匹配预置值且结束日期为空或 0 时补齐结束日期，保留用户明确设置的日期。
- 新版本插入时清除版本列表缓存，使前端及时使用新的版本排序。
- 新赛季试炼装备品级范围不写死：未配置（或配置为 0）时按「同步全部无修装备」处理，装备选择器的品级区间由实际同步到的装备数据推导，不回退旧赛季品级。后台同步在未配置范围时不传 `min_level`/`max_level`，抓取失败或结果为空时保留已有装备。已配置范围时仍以赛季配置为准；仅配置了单边、或最大值小于最小值，视为配置错误并拒绝同步。

## 2. 现状分析

### 2.1 现有数据模型

当前 `Raid` 结构体中 `version` 字段为字符串类型（如 `"3.0"`, `"4.0"`），用于标识副本所属版本。

```typescript
// types.ts - 当前结构
export interface Raid {
  name: string;
  difficulty: '普通' | '英雄' | '挑战';
  playerCount: 10 | 25;
  version?: string;  // 字符串类型，无层级关系
  isActive: boolean;
  bosses?: Boss[];
}
```

### 2.2 问题

- `version` 为自由字符串，无法建立层级关系
- 无法按赛季查询副本记录
- 历史数据无法平滑过渡

## 3. 数据模型设计

### 3.1 版本表 (game_versions)

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PRIMARY KEY | 版本唯一标识 |
| name | TEXT NOT NULL | 版本名称（如"丝路风雨"） |
| code | TEXT NOT NULL UNIQUE | 版本代号（如"4.0"） |
| start_date | INTEGER NOT NULL | 版本开始时间（Unix timestamp） |
| end_date | INTEGER | 版本结束时间（可为空，表示当前） |
| created_at | INTEGER NOT NULL | 创建时间 |

### 3.2 赛季表 (seasons)

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PRIMARY KEY | 赛季唯一标识 |
| name | TEXT NOT NULL | 赛季名称（如"第一赛季"） |
| version_id | INTEGER NOT NULL | 外键 → game_versions.id |
| start_date | INTEGER NOT NULL | 赛季开始时间（当天 7:00） |
| end_date | INTEGER | 赛季结束时间 |
| sort_order | INTEGER NOT NULL DEFAULT 0 | 排序序号 |
| created_at | INTEGER NOT NULL | 创建时间 |

**约束**: `end_date <= version.end_date`（赛季结束时间不晚于版本结束时间）

### 3.3 副本配置表 (raid_configs)

在现有结构基础上新增字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| season_id | INTEGER | 外键 → seasons.id（可空，兼容旧数据） |
| is_active | INTEGER NOT NULL DEFAULT 1 | 是否启用 |

### 3.4 副本记录表 (raid_records)

新增字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| season_id | INTEGER | 外键 → seasons.id（可空，记录时不强制） |

## 4. 层级关系

```
game_versions (版本)
    │
    └── seasons (赛季) [1:N]
            │
            └── raid_configs (副本配置) [1:N]
            │
            └── raid_records (副本记录) [1:N]
```

## 5. 功能实现

### 5.1 版本管理

- `db_get_versions()` - 获取所有版本
- `db_save_version(version_json)` - 保存版本
- `db_delete_version(id)` - 删除版本（级联删除关联赛季）

### 5.2 赛季管理

- `db_get_seasons()` - 获取所有赛季
- `db_get_seasons_by_version(version_id)` - 按版本获取赛季
- `db_save_season(season_json)` - 保存赛季
- `db_delete_season(id)` - 删除赛季
- `db_get_season_for_date(timestamp)` - 根据日期获取所属赛季

### 5.3 副本配置

- `db_get_raid_configs()` - 获取所有副本配置
- `db_get_raid_configs_by_season(season_id)` - 按赛季获取副本
- `db_save_raid_config(config_json)` - 保存副本配置

### 5.4 副本记录

- `db_add_raid_record(record_json)` - 新增记录时自动关联赛季
- `db_get_raid_records_by_season(season_id)` - 按赛季查询记录

## 6. 数据迁移策略

### 6.1 迁移时机

应用启动时检测数据库版本，若小于目标版本则执行迁移。

### 6.2 迁移步骤

1. **创建新表** - 添加 `game_versions`、`seasons` 表
2. **迁移版本数据** - 将 `raid_configs` 中 distinct version 转为 `game_versions` 记录
3. **创建默认赛季** - 为每个版本创建"第一赛季"
4. **关联副本配置** - 将 `raid_configs.season_id` 关联到对应赛季
5. **关联历史记录** - 根据记录日期自动分配到对应赛季

### 6.3 兼容性处理

- `season_id` 允许为空，回退到按 version 字符串过滤
- 前端按 version 过滤时自动转换为 season_id 查询

## 7. 前端集成

### 7.1 类型定义

```typescript
export interface GameVersion {
  id?: number;
  name: string;
  code: string;
  startDate: number;
  endDate?: number;
}

export interface Season {
  id?: number;
  name: string;
  versionId: number;
  startDate: number;
  endDate?: number;
  sortOrder: number;
}
```

### 7.2 状态管理

在 `ConfigContext` 或新建 `SeasonContext` 管理：
- 当前选中的版本/赛季
- 版本/赛季列表缓存

## 8. 性能考虑

- 赛季查询建立索引：`CREATE INDEX idx_seasons_version_id ON seasons(version_id)`
- 副本配置查询索引：`CREATE INDEX idx_raid_configs_season_id ON raid_configs(season_id)`
- 记录查询索引：`CREATE INDEX idx_raid_records_season_id ON raid_records(season_id)`

## 9. 测试计划

### 9.1 单元测试

- 赛季模型字段验证
- 版本-赛季层级关系
- 赛季时间约束校验

### 9.2 集成测试

- 版本创建 → 赛季创建 → 副本配置关联
- 按赛季查询记录正确性
- 数据迁移完整性

### 9.3 性能测试

- 1000+ 赛季数据下查询响应时间
- 大量记录（10000+）的赛季过滤性能
