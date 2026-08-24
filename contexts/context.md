# JX3 Raid Manager 项目上下文

> 给 AI 编码代理 / 新成员 / 跨项目协作者 看的业务背景
> 最后整理：2026-08-23 · 反映 v2.2.0 设计系统统一后的仓库现状

## 📚 上手必读（先看这几份）

| 文档 | 作用 |
|---|---|
| [`AGENTS.md`](../AGENTS.md) | AI 编码代理红线 + 前后端通信约定 + 中文防乱码 |
| [`CLAUDE.md`](../CLAUDE.md) | Claude Code 路由指南（指向本文件） |
| [`specs/design-tokens.md`](../specs/design-tokens.md) | 颜色 / 字号 / 圆角 / 字体 唯一真值源（v2.2.0 沉香褐基调） |
| [`docs/release-process.md`](../docs/release-process.md) | 发版流程 · Release Notes 写法 · 中文防乱码 |

## 项目简介

JX3 Raid Manager (剑网三副本管家) 是一个专为《剑网三》玩家设计的本地化副本数据管理工具。基于 Tauri + React 技术栈，提供安全、便捷的团本收入记录、账号管理和数据分析功能。

## 核心功能

1. **概览 (Dashboard)** —— `components/Dashboard.tsx`
   - 账户总览：总金币收入、副本记录数量、玄晶统计
   - 本期欧皇 / 败家子角色展示
   - 副本收益分布图表（Recharts）
   - 支持按 本周/本月 切换统计周期

2. **账号管理 (Account Manager)** —— `components/AccountManager.tsx` + `AddAccountModal.tsx` + `AddRoleModal.tsx` + `ImportRolesModal.tsx`
   - 管理游戏账号（添加、编辑、删除、禁用）
   - 关联角色与服务器信息，支持门派/装备分数/心法
   - 支持"自有"和"代清"两种账号类型
   - 支持手动添加和扫描游戏目录自动识别

3. **副本管理 (Raid Manager)** —— `components/RaidManager.tsx` + `RaidDetail.tsx` + `RaidLogger.tsx` + `AddRecordModal.tsx` + `BossCooldownDisplay.tsx`
   - 管理副本配置（预制 + 自定义），支持 10/25 人、普通/英雄/挑战难度
   - BOSS 单独选择与 CD 追踪
   - 角色维度查看详情：收支、掉落物、BOSS CD 状态
   - 副本刷新倒计时（周一/周五 7:00）
   - **v2.1.32 起**：赛季系统（版本 → 赛季 → 副本三层级），见 `specs/season_design.md`

4. **百战管理 (Baizhan Manager)** —— `components/BaizhanManager.tsx` + `AddBaizhanRecordModal.tsx` + `BaizhanRoleRecordsModal.tsx`
   - 记录百战异闻录收入/支出
   - 角色维度查看百战历史

5. **试炼之地 (Trial Place Manager)** —— `components/TrialPlaceManager.tsx` + `AddTrialRecordModal.tsx` + `TrialBossEquipmentStatsSection.tsx`
   - 记录试炼之地通关数据（层数、翻牌、掉落装备）
   - 装备数据来源于 JX3Box API

6. **散件老板 (Scraps Boss)** —— `components/ScrapsItemsEditor.tsx`
   - 散件老板功能识别 + 估价（见 `specs/scraps-boss-design.md`）
   - 界面优化见 `specs/scraps-boss-ui-optimization.md`

7. **收支明细 (Income Detail)** —— `components/IncomeDetail.tsx` + `RoleRecordsModal.tsx`
   - 全量收支记录列表，支持按角色/时间/类型筛选（v2.1.38 起支持"全部 / 收入 / 支出" Tab）
   - 展开查看收入/支出明细和备注

8. **玄晶统计 (Crystal Detail)** —— `components/CrystalDetail.tsx`

9. **配置 (Configuration)** —— `components/ConfigManager.tsx` + `MigrationStatus.tsx`
   - 游戏路径设置
   - 主题切换（暗色/亮色模式）
   - 数据迁移（LocalStorage → IndexedDB）
   - 自动扫描 polling 配置（v2.1.52 起历史遗留记录复核）

10. **待确认记录** —— `components/PendingRecordsPanel.tsx`
    - 自动扫描产物 · 待团长手动确认

## 技术栈

- **前端框架**: React 18 + TypeScript（strict mode）
- **构建工具**: Vite 5
- **样式**: Tailwind CSS 3.4.19（自定义 ds-* 颜色系统，详见 `specs/design-tokens.md`）
- **图标**: Lucide React
- **图表**: Recharts
- **桌面端**: Tauri 1.x（Rust backend）—— `src-tauri/`
- **数据存储**: IndexedDB（主要）+ LocalStorage（辅助/旧数据）
- **数据源**: JX3Box API（试炼之地翻牌装备）+ 茗伊插件 chatlog（副本自动扫描）

## 目录结构（截至 v2.2.0）

```
jx3-raid-manager/
├── App.tsx                      # 主组件
├── index.tsx                    # 入口
├── index.html                   # HTML 模板
├── index.css                    # Tailwind 入口 + 提案 token 注入
├── constants.ts                 # 全局常量
├── types.ts                     # TypeScript 类型定义（共享）
│
├── components/                  # 36 个 React 组件
│   ├── Dashboard.tsx
│   ├── AccountManager.tsx
│   ├── AddAccountModal.tsx
│   ├── AddRoleModal.tsx
│   ├── AddBaizhanRecordModal.tsx
│   ├── AddRecordModal.tsx       # 添加副本记录
│   ├── AddTrialRecordModal.tsx
│   ├── BaizhanManager.tsx
│   ├── BaizhanRoleRecordsModal.tsx
│   ├── BossCooldownDisplay.tsx
│   ├── ConfigManager.tsx        # 471 行 · 待精简(⚠️ 见 plans/)
│   ├── CrystalDetail.tsx
│   ├── DateTimePicker.tsx
│   ├── DualRangeSlider.tsx
│   ├── ErrorBoundary.tsx
│   ├── ImportRolesModal.tsx
│   ├── IncomeDetail.tsx
│   ├── LoadingSpinner.tsx
│   ├── MigrationStatus.tsx
│   ├── PendingRecordsPanel.tsx
│   ├── RaidDetail.tsx
│   ├── RaidLogger.tsx           # 快速记录副本
│   ├── RaidManager.tsx
│   ├── RoleRecordsModal.tsx
│   ├── ScrapsItemsEditor.tsx    # 散件老板编辑
│   ├── SectIcon.tsx
│   ├── SectSelect.tsx
│   ├── SetupGuide.tsx
│   ├── ToastContainer.tsx
│   ├── TrialBossEquipmentStatsSection.tsx
│   └── TrialPlaceManager.tsx
│
├── contexts/                    # React Context
│   ├── README.md
│   ├── context.md               # 本文件
│   ├── ActivePollerContext.tsx  # 自动扫描 polling 状态
│   ├── AppConfigContext.tsx     # 应用配置
│   ├── DebugContext.tsx
│   └── ThemeContext.tsx         # 主题管理
│
├── hooks/                       # Custom Hooks
│   ├── index.ts
│   ├── useCountdown.tsx
│   └── useDropScanner.ts
│
├── services/                    # 业务逻辑（13 个文件 + 6 个测试）
│   ├── db.ts                    # IndexedDB 数据库服务
│   ├── migration.ts             # 数据迁移
│   ├── jx3BoxApi.ts             # JX3Box API 集成
│   ├── gameDirectoryScanner.ts  # 游戏目录扫描
│   ├── directoryParser.ts
│   ├── jx3BinaryParser.ts
│   ├── gkpDirectoryScanner.ts
│   ├── dropScanner.ts           # 自动扫描核心
│   ├── activeDetector.ts        # 活动检测
│   ├── appConfig.ts
│   ├── themeStorage.ts
│   ├── accountDirectoryCleanup.ts
│   ├── updater.ts               # 应用内更新
│   ├── mockInvoke.ts            # 测试 mock
│   └── *.test.ts                # 单测
│
├── utils/                       # 工具函数（19 个文件 + 8 个测试）
│   ├── cooldownManager.ts       # 副本 CD
│   ├── bossCooldownManager.ts   # BOSS CD
│   ├── recordUtils.ts
│   ├── recordLookupUtils.ts
│   ├── raidUtils.ts
│   ├── raidRoleUtils.ts
│   ├── raidVersionUtils.ts
│   ├── scrapsUtils.ts           # 散件老板
│   ├── sectConfig.ts            # 门派配置
│   ├── serverUtils.ts
│   ├── accountUtils.ts
│   ├── configUtils.ts
│   ├── pageSearchUtils.ts
│   ├── rareDropUtils.ts
│   ├── toastManager.ts
│   ├── trialFlipStats.ts
│   ├── trialRecordUtils.ts
│   ├── updaterUtils.ts
│   ├── uuid.ts
│   └── *.test.ts
│
├── data/                        # 静态数据 + JSON 源
│   ├── baizhanBosses.ts
│   ├── raidBosses.ts
│   └── kungfu_data.json         # ⚠️ 心法数据源（被 src/data/kungfuData.ts 自动生成）
│
├── src/
│   ├── data/
│   │   └── kungfuData.ts        # ⚠️ 自动生成（vite-plugin-kungfu-data）
│   └── tauri.d.ts               # Tauri 命令类型
│
├── scripts/                     # 一次性脚本（v2.2.0 设计系统统一时新增）
│   ├── patch-index-css.mjs
│   ├── patch-tailwind-config.mjs
│   ├── append-design-tokens.mjs
│   ├── unify-design-tokens.mjs  # emerald/amber → ds-success/ds-warning
│   ├── migrate-kungfu-data.mjs  # data/*.json 颜色迁移
│   ├── design-smoke-test.mjs    # 9 项烟测
│   └── add-doc-positioning.mjs  # 文档整理
│
├── specs/                       # 设计规范（详见 specs/README.md）
│   ├── README.md
│   ├── design-tokens.md         # 唯一 token 真值源（v2.2.0 沉香褐）
│   ├── scraps-boss-design.md    # 散件老板业务规则
│   ├── scraps-boss-ui-optimization.md
│   └── season_design.md         # 赛季系统
│
├── docs/                        # 流程文档（详见 docs/README.md）
│   ├── README.md
│   ├── release-process.md
│   ├── TEST_CASES.md
│   ├── user-guide.md            # 完整参考手册
│   ├── 新手指南.md              # step-by-step 入门
│   ├── design-system-meeting.md # v2.2.0 设计系统会议议程
│   ├── jx3my-plugin-analysis.md # 茗伊插件反分析（1294 行）
│   └── plans/                   # 设计计划（带状态徽章）
│       ├── 2026-04-09-kungfu-data-unification-design.md  ✅
│       ├── 2026-06-28-raid-drop-scanner-design.md         🚧
│       ├── 2026-07-13-config-ui-simplification-design.md ⚠️
│       ├── 2026-08-09-stale-scanning-verify-design.md    ✅
│       └── 2026-08-09-stale-scanning-verify-impl.md       ✅
│
├── contexts/                    # 业务上下文
│   ├── README.md
│   └── context.md               # 本文件
│
├── release-notes/               # 版本说明（50 个）
│   ├── README.md                # 索引
│   └── vX.Y.Z.md
│
├── src-tauri/                   # Tauri 后端（Rust）
│   └── tauri.conf.json
│
├── public/                      # 静态资源
│
├── postcss.config.js
├── tailwind.config.js           # 含 ds-* 主题扩展
├── vite.config.ts               # 含 vite-plugin-kungfu-data
├── package.json                 # 当前版本 2.1.54
└── AGENTS.md                    # AI 编码代理红线
```

## ⚠️ 重要注意事项

### Tailwind CSS 自定义颜色冲突
`tailwind.config.js` 中定义了 `colors.base` 等自定义颜色，这会导致 Tailwind 生成 `text-base` 颜色工具类，**覆盖**内置的 `text-base`（font-size: 1rem）。

**规则**：当需要 1rem 字号时，使用 `text-[1rem]` 而非 `text-base`。

### 设计规范
收入/支出的图标与颜色已有统一规范（v2.2.0 沉香褐基调），详见 `specs/design-tokens.md`。
任何**新增**颜色 / 字号 / 圆角 必须从设计 token 取值，**禁止**在组件里硬编码 hex / 像素值。

### 数据源唯一性
- `data/kungfu_data.json` 是心法数据的**唯一源** —— 修改后运行 `npm run build` 由 `vite-plugin-kungfu-data` 自动生成 `src/data/kungfuData.ts`
- 切勿手动改 `src/data/kungfuData.ts`，下次 build 会被覆盖

### 配色迁移（v2.2.0 已完成）
- `emerald-*` / `amber-*` → `ds-success` / `ds-warning`（764 处迁移完成，残留 0）
- 主操作色 `--primary-base` → 沉香褐 (`168 99 63`)
- 旧 `bg-primary` 类名继续可用（自动映射）

### 中文防乱码（`AGENTS.md §10`）
- 不用 PowerShell 写中文文件名
- 文件统一 CRLF 行尾
- Node 脚本用 `fs.writeFileSync` 显式 UTF-8

## 关键业务逻辑

- **副本 CD**：25 人本周一 7:00 刷新，10 人本周一/周五 7:00 刷新（`utils/cooldownManager.ts`）
- **BOSS CD**：见 `utils/bossCooldownManager.ts` + `components/BossCooldownDisplay.tsx`
- **数据迁移**：启动时检查 LocalStorage 旧数据，迁移至 IndexedDB（`services/migration.ts`）
- **自动扫描**：polling 读取 chatlog → `services/dropScanner.ts` → `components/PendingRecordsPanel.tsx`
- **历史遗留记录复核**：v2.1.52 起，每次扫描开始时复核「进行中」和「无掉落无工资的待确认」（`contexts/ActivePollerContext.tsx`）
- **自动解析**：扫描剑网三目录解析账号/角色（`services/gameDirectoryScanner.ts`）
- **JX3Box API**：获取装备数据用于试炼之地翻牌展示（`services/jx3BoxApi.ts`）
- **赛季系统**：版本 → 赛季 → 副本 三层层级，详见 `specs/season_design.md`
- **散件老板**：副本中散件识别 + 估价，详见 `specs/scraps-boss-design.md`