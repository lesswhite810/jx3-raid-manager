# JX3 Raid Manager 项目上下文

> 给 AI 编码代理 / 新成员 / 跨项目协作者 看的业务背景
> 最后整理：2026-09-11 · 反映 v2.2.2 发布后、v2.2.3 开发中的仓库现状

## 📚 上手必读（先看这几份）

| 文档 | 作用 |
|---|---|
| [`AGENTS.md`](../AGENTS.md) | AI 编码代理红线 + 前后端通信约定 + 中文防乱码 + 发版流程（第 9、10 节） |
| [`CLAUDE.md`](../CLAUDE.md) | Claude Code 路由指南（指向本文件） |
| [`specs/design-tokens.md`](../specs/design-tokens.md) | 颜色 / 字号 / 圆角 / 字体 唯一真值源（v2.2.0 沉香褐基调） |
| [`specs/season_design.md`](../specs/season_design.md) | 赛季系统三层结构与时间衔接规则 |

> ⚠️ `docs/` 目录自 2026-09-06 起整体不入库（已加入 `.gitignore`），只作本地参考。**新增文档不要写进 `docs/`**，也不要在入库文档中引用 `docs/**` 路径；发版与中文防乱码要点已内联到 [`AGENTS.md`](../AGENTS.md) 第 9、10 节。

## 项目简介

JX3 Raid Manager (剑网三副本管家) 是一个专为《剑网三》玩家设计的本地化副本数据管理工具。基于 Tauri 2 + React 技术栈，提供安全、便捷的团本收入记录、账号管理和数据分析功能。所有业务数据保存在用户本机的 SQLite 数据库中，不上传任何数据。

## 核心功能

1. **概览 (Dashboard)** —— `components/Dashboard.tsx`
   - 本期总收入 / 总支出 / 净收入，附散件估价与散件支出副信息
   - 副本记录数、玄晶等稀有掉落统计
   - 副本收益分布图表（Recharts）
   - 支持按 **本周 / 本赛季 / 全部** 切换统计周期

2. **账号管理 (Account Manager)** —— `components/AccountManager.tsx` + `AddAccountModal.tsx` + `AddRoleModal.tsx` + `ImportRolesModal.tsx`
   - 管理游戏账号（添加、编辑、删除、禁用）
   - 关联角色与服务器信息，支持门派 / 心法 / 装备分数
   - 支持"自有"和"代清"两种账号类型
   - 支持手动添加和扫描游戏目录自动识别
   - 拖动排序，排序结果持久化到数据库

3. **副本管理 (Raid Manager)** —— `components/RaidManager.tsx` + `RaidDetail.tsx` + `RaidLogger.tsx` + `AddRecordModal.tsx` + `BossCooldownDisplay.tsx`
   - 管理副本配置（预制 + 自定义），支持 10/25 人、普通/英雄/挑战难度
   - BOSS 单独选择与 CD 追踪
   - 角色维度查看详情：收支、掉落物、BOSS CD 状态
   - 副本刷新倒计时（25 人本周一 7:00，10 人本周一/周五 7:00）
   - **v2.1.32 起**：赛季系统（版本 → 赛季 → 副本三层级），见 `specs/season_design.md`
   - **v2.1.52 起**：自动扫描历史遗留记录复核（卡"进行中"、待确认无数据两类）

4. **百战管理 (Baizhan Manager)** —— `components/BaizhanManager.tsx` + `AddBaizhanRecordModal.tsx` + `BaizhanRoleRecordsModal.tsx`
   - 记录百战异闻录收入/支出
   - 角色维度查看百战历史

5. **试炼之地 (Trial Place Manager)** —— `components/TrialPlaceManager.tsx` + `AddTrialRecordModal.tsx` + `TrialRoleRecordsModal.tsx` + `TrialFlipStatsModal.tsx` + `TrialFlipDetail.tsx` + `TrialBossEquipmentStatsSection.tsx`
   - 记录试炼之地通关数据（层数、翻牌、掉落装备）
   - 装备数据来源于 JX3Box API
   - 翻牌统计：按位置统计翻牌率 / 出装备率，按 Boss 组合统计高发装备位置

6. **散件老板 (Scraps Boss)** —— `components/ScrapsItemsEditor.tsx`
   - 散件老板功能识别 + 估价（见 `specs/scraps-boss-design.md`）
   - 界面优化见 `specs/scraps-boss-ui-optimization.md`
   - 散件支出按白名单材料的实际购买价统计，装备 / 小铁等购买只计入总支出

7. **收支明细 (Income Detail)** —— `components/IncomeDetail.tsx` + `RoleRecordsModal.tsx`
   - 全量收支记录列表，支持按角色 / 时间 / 类型筛选（"全部 / 收入 / 支出" Tab）
   - 收益记录分页加载（默认 100 条 + 加载更多）
   - 展开查看收入/支出明细和备注

8. **玄晶统计 (Crystal Detail)** —— `components/CrystalDetail.tsx`

9. **配置 (Configuration)** —— `components/ConfigManagerModal.tsx` + `components/config-panels/`（5 个分类面板）
   - 游戏路径设置（`ConfigGamePanel`）
   - 主题切换：经典 / 江湖纸笺 / 暗色（`ConfigThemePanel`，主题名常量见 `services/themeStorage.ts`）
   - 数据目录切换与迁移状态（`MigrationStatus.tsx`、`ConfigAdvancedPanel`）
   - 自动更新（`ConfigUpdatePanel` + `UpdateDialog.tsx`）
   - 意见反馈（`ConfigFeedbackPanel`）
   - 自动扫描 polling 与启动刷新装分配置

10. **待确认记录** —— `components/PendingRecordsPanel.tsx`
    - 自动扫描产物 · 待团长手动确认

11. **跨角色设置同步（v2.2.2 起）** —— `components/CharSyncModal.tsx` + `services/charSync.ts`
    - 把一个角色的界面布局 / 快捷键 / 聊天自定义 / 插件设置复制到其他角色
    - 同步前自动备份原角色设置，可一键回滚
    - 回滚标记存于 SQLite `char_sync_rollback_marks` 表（V18），前端不读 localStorage

## 技术栈

- **前端框架**: React 18 + TypeScript（strict mode）
- **构建工具**: Vite 5
- **样式**: Tailwind CSS 3.4（自定义 `ds-*` 颜色系统，详见 `specs/design-tokens.md`）
- **图标**: Lucide React
- **图表**: Recharts
- **桌面端**: **Tauri 2**（Rust 后端）—— `src-tauri/`
- **数据存储**: **SQLite**（`rusqlite` bundled，schema **V18**，v1 → v18 完整迁移链）
  - localStorage 仅保留两类用途：主题偏好、旧版（v2.1.x 之前）数据迁移的读取来源与迁移标记
  - **IndexedDB 已完全移除**（全仓库 0 处引用），不要再按 IndexedDB 设计新功能
- **数据源**: JX3Box API（试炼之地翻牌装备 + 材料成交聚合估价）+ 茗伊插件 chatlog（副本自动扫描）+ 游戏安装目录（账号 / 角色 / 装分）

## 目录结构（截至 2026-09-11）

```
jx3-raid-manager/
├── App.tsx                      # 主组件
├── index.tsx                    # 入口
├── index.html                   # HTML 模板
├── index.css                    # Tailwind 入口 + 设计 token 注入
├── constants.ts                 # 全局常量
├── types.ts                     # TypeScript 类型定义（共享）
│
├── components/                  # 39 个 React 组件 + config-panels/ 5 个
│   ├── Dashboard.tsx
│   ├── AccountManager.tsx       # 1852 行 · 全仓最大组件
│   ├── AddAccountModal.tsx
│   ├── AddRoleModal.tsx
│   ├── AddRecordModal.tsx       # 添加副本记录
│   ├── AddBaizhanRecordModal.tsx
│   ├── AddTrialRecordModal.tsx
│   ├── BaizhanManager.tsx
│   ├── BaizhanRoleRecordsModal.tsx
│   ├── BossCooldownDisplay.tsx
│   ├── CharSyncModal.tsx        # 跨角色设置同步（v2.2.2）
│   ├── ConfigManager.tsx
│   ├── ConfigManagerModal.tsx   # 设置弹窗外壳（分类导航）
│   ├── config-panels/           # ConfigGamePanel / ConfigThemePanel /
│   │                            # ConfigUpdatePanel / ConfigAdvancedPanel / ConfigFeedbackPanel
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
│   ├── ThemeSelector.tsx
│   ├── ToastContainer.tsx
│   ├── TrialBossEquipmentStatsSection.tsx
│   ├── TrialFlipDetail.tsx
│   ├── TrialFlipStatsModal.tsx
│   ├── TrialPlaceManager.tsx
│   ├── TrialRoleRecordsModal.tsx
│   ├── UpdateDialog.tsx
│   └── WindowControls.tsx
│
├── contexts/                    # React Context
│   ├── README.md
│   ├── context.md               # 本文件
│   ├── ActivePollerContext.tsx  # 自动扫描 polling 状态
│   ├── AppConfigContext.tsx     # 应用配置
│   ├── DebugContext.tsx
│   └── ThemeContext.tsx         # 主题管理
│
├── hooks/                       # 自定义 Hooks
│   ├── index.ts
│   ├── useCountdown.tsx
│   └── useDropScanner.ts
│
├── services/                    # 业务逻辑（15 个 + 1 个测试）
│   ├── db.ts                    # ⭐ IPC 唯一出口 · 724 行 · 28 处 invoke
│   ├── charSync.ts              # 跨角色设置同步
│   ├── migration.ts             # localStorage 旧数据 → SQLite 迁移
│   ├── dropScanner.ts           # 自动扫描核心
│   ├── activeDetector.ts        # 活跃检测
│   ├── gameDirectoryScanner.ts  # 游戏目录扫描
│   ├── directoryParser.ts
│   ├── jx3BinaryParser.ts
│   ├── gkpDirectoryScanner.ts
│   ├── jx3BoxApi.ts             # JX3Box API 集成
│   ├── appConfig.ts
│   ├── themeStorage.ts
│   ├── accountDirectoryCleanup.ts
│   ├── updater.ts               # 应用内更新
│   ├── mockInvoke.ts            # 测试 mock
│   └── accountDirectoryCleanup.test.ts
│
├── utils/                       # 工具函数（20 个 + 12 个测试）
│   ├── cooldownManager.ts       # 副本 CD
│   ├── bossCooldownManager.ts   # BOSS CD
│   ├── goldFormat.ts            # 金额「砖 / 金」格式化
│   ├── scrapsUtils.ts           # 散件老板
│   ├── trialFlipStats.ts        # 试炼翻牌统计
│   ├── trialRecordUtils.ts
│   ├── recordUtils.ts
│   ├── recordLookupUtils.ts
│   ├── raidUtils.ts
│   ├── raidRoleUtils.ts
│   ├── raidVersionUtils.ts
│   ├── rareDropUtils.ts
│   ├── sectConfig.ts            # 门派配置
│   ├── serverUtils.ts
│   ├── accountUtils.ts
│   ├── configUtils.ts
│   ├── pageSearchUtils.ts
│   ├── toastManager.ts
│   ├── updaterUtils.ts
│   └── uuid.ts
│
├── data/                        # 静态数据 + JSON 源
│   ├── baizhanBosses.ts
│   ├── raidBosses.ts
│   └── kungfu_data.json         # ⚠️ 心法数据唯一源（自动生成 src/data/kungfuData.ts）
│
├── src/
│   ├── data/
│   │   └── kungfuData.ts        # ⚠️ 自动生成（vite-plugin-kungfu-data），勿手改
│   └── tauri.d.ts               # Tauri 命令类型
│
├── scripts/                     # 构建 / 发版 / 一次性维护脚本（36 个）
│   ├── run-tauri.mjs            # npm run tauri 入口（快速本地 release 配置）
│   ├── update-release-notes.mjs # npm run release:notes
│   ├── prepare-next-version.mjs # npm run version:prepare
│   └── …                        # 其余为设计系统统一、数据迁移等一次性脚本
│
├── specs/                       # 设计规范（5 份，入库）
│   ├── README.md
│   ├── design-tokens.md         # 唯一 token 真值源（v2.2.0 沉香褐）
│   ├── scraps-boss-design.md    # 散件老板业务规则
│   ├── scraps-boss-ui-optimization.md
│   └── season_design.md         # 赛季系统
│
├── release-notes/               # 版本说明（53 个 + README 索引）
│   ├── README.md                # 索引
│   └── vX.Y.Z.md
│
├── src-tauri/                   # Tauri 2 后端（Rust）
│   ├── Cargo.toml               # 版本需与 package.json / tauri.conf.json 同步
│   ├── tauri.conf.json
│   ├── nsis/                    # 安装器定制（hooks.nsh / SimpChinese.nsh）
│   └── src/
│       ├── main.rs              # 317 行 · 只做插件装配与命令注册
│       ├── db.rs                # ⭐ 5523 行 · 约 80 个 db_* 命令 + schema V18
│       ├── db/
│       │   ├── migration.rs     # 迁移调度
│       │   ├── migrations/      # v1.rs … v18.rs
│       │   ├── upgrade_tests.rs # 升级测试（依赖 APPDATA 环境变量）
│       │   └── static_*.json    # 预制副本 / 赛季 / 版本数据
│       ├── mingyi/              # 茗伊插件 chatlog 解析（自动扫描核心）
│       ├── char_sync.rs         # 跨角色设置同步
│       ├── char_sync_rollback_marks.rs
│       ├── gkp_parser.rs        # GKP 记录解析
│       ├── jx3_process.rs       # 游戏进程检测（含多开）
│       ├── equip_sync.rs        # 装分同步
│       ├── game_directory.rs    # 游戏目录定位
│       ├── kungfu_data.rs       # 心法数据
│       ├── drop_table.rs        # 掉落表
│       ├── runtime_mode.rs      # 安装版 / 便携版区分
│       ├── updater.rs           # 自动更新
│       └── app_config.rs
│
├── public/                      # 静态资源
│
├── postcss.config.js
├── tailwind.config.js           # 含 ds-* 主题扩展
├── vite.config.ts               # 含 vite-plugin-kungfu-data
├── package.json                 # 当前开发版 2.2.3
└── AGENTS.md                    # AI 编码代理红线
```

## ⚠️ 重要注意事项

### Tailwind CSS 自定义颜色冲突
`tailwind.config.js` 中定义了 `colors.base` 等自定义颜色，这会导致 Tailwind 生成 `text-base` 颜色工具类，**覆盖**内置的 `text-base`（font-size: 1rem）。

**规则**：当需要 1rem 字号时，使用 `text-[1rem]` 而非 `text-base`。

### 设计规范
收入/支出的图标与颜色已有统一规范（v2.2.0 沉香褐基调），详见 `specs/design-tokens.md`。
任何**新增**颜色 / 字号 / 圆角 必须从设计 token 取值，**禁止**在组件里硬编码 hex / 像素值。

### 心法数据源唯一性
- `data/kungfu_data.json` 是心法数据的**唯一源** —— 修改后运行 `npm run build` 由 `vite-plugin-kungfu-data` 自动生成 `src/data/kungfuData.ts`
- 切勿手动改 `src/data/kungfuData.ts`，下次 build 会被覆盖

### 数据层唯一性
- **业务数据只写 SQLite**，通过 `services/db.ts` 的 `invoke` 调用后端 `db_*` 命令，不要在前端另起存储
- 升 schema 版本时四处必须同步：`src-tauri/src/db.rs` 的 `CURRENT_SCHEMA_VERSION`、`db/migration.rs`、`db/migrations/mod.rs`、`db/upgrade_tests.rs`，并新增 `db/migrations/vN.rs`
- localStorage 只允许用于主题偏好等纯 UI 状态，不得承载业务数据

### 配色迁移（v2.2.0 已完成）
- `emerald-*` / `amber-*` → `ds-success` / `ds-warning`（764 处迁移完成，残留 0）
- 主操作色 `--primary-base` → 沉香褐 (`168 99 63`)
- 旧 `bg-primary` 类名继续可用（自动映射）

### 中文防乱码（`AGENTS.md §10`）
- 不要用 PowerShell 直接写中文文件内容
- 不要把中文 Release Notes 直接作为 GitHub CLI 参数
- 写中文内容优先使用 Node 脚本并显式指定 `utf8`
- 更新 GitHub Release Notes 统一使用 `npm run release:notes -- <tag> <notes-file>`

### 行尾规范
`.gitattributes` 统一为 `* text=auto eol=lf`（根治 Windows `core.autocrlf=true` 带来的纯 CRLF 噪音 diff）。
如果遇到「`git status` 显示 M 但 `git diff` 内容为空」的文件，直接 `git checkout -- <file>` 恢复。

## 关键业务逻辑

- **副本 CD**：25 人本周一 7:00 刷新，10 人本周一/周五 7:00 刷新（`utils/cooldownManager.ts`）
- **BOSS CD**：见 `utils/bossCooldownManager.ts` + `components/BossCooldownDisplay.tsx`
- **数据迁移**：启动时检查 localStorage 旧数据，迁移至 SQLite（`services/migration.ts` + 后端 `db_is_local_storage_migrated`）
- **自动扫描**：polling 读取茗伊 chatlog → `src-tauri/src/mingyi/` + `services/dropScanner.ts` → `components/PendingRecordsPanel.tsx`
- **历史遗留记录复核**：v2.1.52 起，每次扫描开始时复核「进行中」和「无掉落无工资的待确认」（`contexts/ActivePollerContext.tsx`）
- **自动解析**：扫描剑网三目录解析账号 / 角色（`services/gameDirectoryScanner.ts` + `src-tauri/src/game_directory.rs`）
- **JX3Box API**：试炼之地翻牌装备数据 + 材料成交聚合估价（`services/jx3BoxApi.ts`）
- **赛季系统**：版本 → 赛季 → 副本 三层层级，赛季起止时间连续衔接，详见 `specs/season_design.md`
- **散件老板**：副本中散件识别 + 估价，详见 `specs/scraps-boss-design.md`
- **跨角色设置同步**：同步 / 备份 / 回滚，回滚标记存 SQLite，详见 `src-tauri/src/char_sync.rs`
- **金额显示**：录入与存储一律以「金」为单位，展示时 ≥ 10000 金按「xx砖xx金」格式化（`utils/goldFormat.ts`）

## 常见排查提示

- **CI「Cargo 检查」步骤自 2026-08-22 起连续失败**，横跨多个成功发布的版本，是既有环境问题（疑为 `ci-` 前缀 rust-cache 损坏），**不是代码问题**。以 Release 工作流和本地 `cargo check --release` 为准。
- **跑 Rust `db/upgrade_tests.rs` 前必须手动设置 `APPDATA`**（Git Bash 用 `export APPDATA="C:\\Users\\<用户>\\AppData\\Roaming"`），否则 14 个用例会因 `NotPresent` 全部 panic，与代码无关。
- **判断上次发布版本**要三方交叉验证：GitHub Releases API、`git ls-remote --tags origin`、本地 `git tag`。`gitee` 只是镜像远端，常因代理不可达，不能作为发布依据。
