# JX3 Raid Manager 项目上下文

## 项目简介
JX3 Raid Manager (剑网三副本管家) 是一个专为《剑网三》玩家设计的本地化副本数据管理工具。基于 Tauri + React 技术栈，提供安全、便捷的团本收入记录、账号管理和数据分析功能。

## 核心功能
1.  **概览 (Dashboard)**
    - 账户总览：总金币收入、副本记录数量、玄晶统计
    - 本期欧皇 / 败家子角色展示
    - 副本收益分布图表（Recharts）
    - 支持按 本周/本月 切换统计周期

2.  **账号管理 (Account Manager)**
    - 管理游戏账号（添加、编辑、删除、禁用）
    - 关联角色与服务器信息，支持门派/装备分数
    - 支持"自有"和"代清"两种账号类型
    - 支持手动添加和扫描游戏目录自动识别

3.  **副本管理 (Raid Manager)**
    - 管理副本配置（预制 + 自定义），支持 10/25 人、普通/英雄/挑战难度
    - BOSS 单独选择与 CD 追踪
    - 角色维度查看详情：收支、掉落物、BOSS CD 状态
    - 副本刷新倒计时（周一/周五 7:00）

4.  **百战管理 (Baizhan Manager)**
    - 记录百战异闻录收入/支出
    - 角色维度查看百战历史

5.  **试炼之地 (Trial Place Manager)**
    - 记录试炼之地通关数据（层数、翻牌、掉落装备）
    - 装备数据来源于 JX3Box API

6.  **收支明细 (Income Detail)**
    - 全量收支记录列表，支持按角色/时间/类型筛选
    - 展开查看收入/支出明细和备注

7.  **配置 (Configuration)**
    - 游戏路径设置
    - 主题切换（暗色/亮色模式）
    - 数据迁移（LocalStorage → IndexedDB）

## 技术栈
- **前端框架**: React 18, TypeScript
- **构建工具**: Vite
- **样式**: Tailwind CSS（自定义颜色系统）
- **图标**: Lucide React
- **图表**: Recharts
- **桌面端**: Tauri (Rust backend)
- **数据存储**: IndexedDB (主要), LocalStorage (辅助/旧数据)

## 目录结构
```
├── components/        # React 组件（26 个）
│   ├── Dashboard.tsx          # 数据概览
│   ├── AccountManager.tsx     # 账号管理
│   ├── RaidManager.tsx        # 副本管理
│   ├── RaidDetail.tsx         # 副本详情（角色列表+CD追踪）
│   ├── RaidLogger.tsx         # 快速记录副本
│   ├── BaizhanManager.tsx     # 百战管理
│   ├── TrialPlaceManager.tsx  # 试炼之地
│   ├── IncomeDetail.tsx       # 收支明细
│   ├── CrystalDetail.tsx      # 玄晶统计
│   ├── ConfigManager.tsx      # 配置管理
│   └── ...Modal.tsx           # 各类弹窗组件
├── contexts/          # React Context
│   ├── context.md             # 项目上下文（本文件）
│   └── ThemeContext.tsx        # 主题管理
├── data/              # 静态数据
│   ├── staticRaids.ts         # 预制副本配置
│   ├── raidBosses.ts          # BOSS 配置
│   └── baizhanBosses.ts       # 百战 BOSS 配置
├── hooks/             # Custom Hooks
├── services/          # 业务逻辑
│   ├── db.ts                  # IndexedDB 数据库服务
│   ├── migration.ts           # 数据迁移
│   ├── jx3BoxApi.ts           # JX3Box API 集成
│   ├── gameDirectoryScanner.ts
│   └── ai/                    # AI 辅助服务
├── specs/             # 技术规范
│   └── design-tokens.md       # 设计规范（颜色、图标约定）
├── utils/             # 工具函数
│   ├── cooldownManager.ts     # 副本 CD 计算
│   ├── bossCooldownManager.ts # BOSS CD 计算
│   ├── recordUtils.ts         # 记录去重/格式化
│   └── toastManager.ts        # Toast 通知
├── types.ts           # TypeScript 类型定义
└── src-tauri/         # Tauri 后端
```

## ⚠️ 重要注意事项

### Tailwind CSS 自定义颜色冲突
`tailwind.config.js` 中定义了 `colors.base` 等自定义颜色，这会导致 Tailwind 生成 `text-base` 颜色工具类，**覆盖**内置的 `text-base`（font-size: 1rem）。

**规则**：当需要 1rem 字号时，使用 `text-[1rem]` 而非 `text-base`。

### 设计规范
收入/支出的图标与颜色已有统一规范，详见 `specs/design-tokens.md`。

## 关键业务逻辑
- **副本 CD**: 25 人本周一 7:00 刷新，10 人本周一/周五 7:00 刷新（`utils/cooldownManager.ts`）
- **数据迁移**: 启动时检查 LocalStorage 旧数据，迁移至 IndexedDB（`services/migration.ts`）
- **目录扫描**: 扫描剑网三目录解析账号/角色（`services/gameDirectoryScanner.ts`）
- **JX3Box API**: 获取装备数据用于试炼之地翻牌展示（`services/jx3BoxApi.ts`）

