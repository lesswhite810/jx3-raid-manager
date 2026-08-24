# `specs/` 设计规范索引

> 设计规范 / 数据模型 / 实施方案 的统一入口
> 最后整理：2026-08-23

---

## 📋 设计文档速查表

| 文档 | 行数 | 最近更新 | 状态 | 用途 |
|---|---:|---:|---:|---|
| [`design-tokens.md`](./design-tokens.md) | 249 | 2026-08-23 | 🔵 **v2.2.0 增量** | 设计 token 全表（颜色 / 字号 / 圆角 / 字体 / 阴影 / 动效） |
| [`scraps-boss-design.md`](./scraps-boss-design.md) | 395 | 2026-08-10 | ✅ 已实现 | 散件老板功能 · 业务规则 / 数据结构 / UI 设计 |
| [`scraps-boss-ui-optimization.md`](./scraps-boss-ui-optimization.md) | 230 | 2026-08-21 | 🚧 实现中 | 散件老板界面分析与优化方案（`AddRecordModal` + `PendingRecordsPanel`） |
| [`season_design.md`](./season_design.md) | 185 | 2026-04-28 | ✅ 已实现 | 赛季系统 · 版本 → 赛季 → 副本 三层层级数据模型（v2.1.32 上线） |

> 📝 **状态徽章约定**：
> - ✅ 已实现：代码已落地，文档作为架构参考
> - 🚧 实现中：部分落地，仍在迭代
> - ⚠️ 悬空：确认方案但未开始实现
> - 🔵 增量中：有近期重要改动（指向本版本）
> - 📦 已归档：超期或被新方案取代

---

## 🎯 按场景查文档

### 我想……看哪份

| 我想…… | 看哪份 |
|---|---|
| 知道某个 Tailwind 工具类的来源 | `design-tokens.md §2 几何 / 排版 token` |
| 写新组件时确认颜色 / 字号 / 阴影规范 | `design-tokens.md §1 提案 token 全表` |
| 改收入/支出相关样式 | `design-tokens.md §收入与支出` |
| 散件老板的识别规则 | `scraps-boss-design.md §3 散件清单规则` |
| 改散件老板相关弹窗 | `scraps-boss-ui-optimization.md` |
| 调整赛季 / 版本 / 副本层级 | `season_design.md` |
| 理解副本层级数据模型 | `season_design.md §3-4 数据模型设计` |

### 与其它文档的关联

```
specs/
├── design-tokens.md         ← ┐
│                             │  唯一 token 真值源
│   tailwind.config.js       ← ┘
│   index.css                ← ┘
├── scraps-boss-design.md    ← 散件核心业务
│   scraps-boss-ui-optimization.md  ← 散件交互优化
└── season_design.md         ← 副本层级架构
```

---

## 📝 文档维护约定（`AGENTS.md §11` 强化）

- **状态徽章必填**：每个 spec 文件头部加 `> 状态：✅ 已实现 / 🚧 实现中 / ⚠️ 悬空 / 🔵 增量中 / 📦 已归档`，与 release version 关联
- **代码与 spec 不同步**：发现代码已变更但 spec 未跟进 → 优先改 spec（SSOT 原则）
- **新增设计**：用 `<feature>-design.md` 命名（英文小写连字符），落地后加 `✅` 徽章
- **token 永远从 `colors_and_type.css` / `design-tokens.md` 取值**：不在代码里硬编码 hex / 像素值（`AGENTS.md §5`）

---

**最后整理**：2026-08-23 · v2.2.0 设计系统统一后的快照
