# `docs/` 目录索引

> 流程文档 · 测试文档 · 计划文档 · 外部分析文档 的统一入口
> 最后整理：2026-08-23

---

## 📋 文档速查表

### 流程文档（持续维护）

| 文档 | 行数 | 最近更新 | 用途 |
|---|---:|---:|---|
| [`release-process.md`](./release-process.md) | 229 | 2026-08-06 | Release Notes 写法、中文防乱码、发版前 checklist、v2.1.24 问题总结 |
| [`TEST_CASES.md`](./TEST_CASES.md) | 369 | 2026-02-27 | 后台 API 测试 / 代码修复验证测试 / 数据一致性测试 / 执行方式 |
| [`design-system-meeting.md`](./design-system-meeting.md) | 130 | 2026-08-23 | 设计系统统一 · 团队评审会议议程（4 项决策） |

### 用户文档（面向最终用户）

| 文档 | 行数 | 最近更新 | 用途 |
|---|---:|---:|---|
| [`新手指南.md`](./新手指南.md) | 133 | 2026-04-08 | **step-by-step** · 下载→配置→添加账号→记录副本→常见问题 |
| [`user-guide.md`](./user-guide.md) | 370 | 2026-03-28 | **完整参考** · 功能介绍 / 配置项 / 游戏内设置 / 快捷键 / 高级技巧 |

> 📝 两份用户文档**双轨并存**：
> - 想"3 分钟跑起来" → 看 [`新手指南.md`](./新手指南.md)
> - 想"查某个具体功能" → 看 [`user-guide.md`](./user-guide.md)

### 外部分析文档

| 文档 | 行数 | 最近更新 | 用途 |
|---|---:|---:|---|
| [`jx3my-plugin-analysis.md`](./jx3my-plugin-analysis.md) | 1294 | 2026-08-23 | 茗伊插件反分析 · 仓库结构 / 数据格式 / 公共 API / UI 模块清单（已加 TOC） |

> 这份文档面向做"集成/对接"工作的高级工程师；普通使用者不需要读。

---

## 📁 `docs/plans/` 设计与实施计划

> 每个文件头部都有状态徽章（✅ 已落地 · 🚧 实现中 · ⚠️ 悬空 · 📦 已归档）

| 计划 | 日期 | 状态 | 关联版本 |
|---|---:|---|---|
| [`2026-04-09-kungfu-data-unification-design.md`](./plans/2026-04-09-kungfu-data-unification-design.md) | 2026-04-09 | 📦 已归档 | 心法数据统一 · 完整内容见 [archive](./plans/archive/2026-04-09-kungfu-data-unification-design.md) |
| [`2026-06-28-raid-drop-scanner-design.md`](./plans/2026-06-28-raid-drop-scanner-design.md) | 2026-06-28 | 🚧 实现中 | 副本掉落自动扫描（B 阶段） |
| [`2026-07-13-config-ui-simplification-design.md`](./plans/2026-07-13-config-ui-simplification-design.md) | 2026-07-13 | ⚠️ 悬空 | 系统配置界面精简 · 原 plan 标"已确认，待实现" |
| [`2026-08-09-stale-scanning-verify-design.md`](./plans/2026-08-09-stale-scanning-verify-design.md) | 2026-08-09 | ✅ 已落地 | 自动扫描历史遗留记录复核 · v2.1.52 上线 |
| [`2026-08-09-stale-scanning-verify-impl.md`](./plans/2026-08-09-stale-scanning-verify-impl.md) | 2026-08-09 | ✅ 已落地 | 同上的具体实现步骤 |

### 归档目录

> 落地超过 90 天且无维护价值的 plan 自动归档到 [`docs/plans/archive/`](./plans/archive/)

| 已归档 | 归档时间 |
|---|---|
| [`2026-04-09-kungfu-data-unification-design.md`](./plans/archive/2026-04-09-kungfu-data-unification-design.md) | 2026-08-23 |

---

## 🔍 找文档的常用入口

| 我想…… | 看哪份 |
|---|---|
| 了解如何发版 | `release-process.md` |
| 学习怎么用应用 | `新手指南.md`（10 分钟上手） / `user-guide.md`（完整功能） |
| 看某个功能的测试用例 | `TEST_CASES.md` |
| 查具体设计规范 | `../specs/design-tokens.md` |
| 查业务上下文 | `../contexts/context.md` |
| 反向研究茗伊插件 | `jx3my-plugin-analysis.md` |
| 找某次发版的完整笔记 | `../release-notes/vX.Y.Z.md` |
| 跟进某个悬空的设计 | `plans/` 目录下带 ⚠️ 的文件 |

---

## 🔍 找文档的常用入口

| 我想…… | 看哪份 |
|---|---|
| 了解如何发版 | `release-process.md` |
| 学习怎么用应用 | `新手指南.md`（10 分钟上手） / `user-guide.md`（完整功能） |
| 看某个功能的测试用例 | `TEST_CASES.md` |
| 查具体设计规范 | `../specs/design-tokens.md` |
| 查业务上下文 | `../contexts/context.md` |
| 反向研究茗伊插件 | `jx3my-plugin-analysis.md` |
| 找某次发版的完整笔记 | `../release-notes/vX.Y.Z.md` |
| 跟进某个悬空的设计 | `plans/` 目录下带 ⚠️ 的文件 |

---

## 📝 文档维护约定（`AGENTS.md §11` 强化）

- **写新文档**：先确认不在 `AGENTS.md` / `CLAUDE.md` 路由表里登记 → 同步更新本文档索引
- **改大文件**：超过 200 行的文件，章节标题要带 emoji 标记（## ❓ / ## ⚠️ / ## 📝 等），便于扫读
- **归档计划**：落地的设计 → `plans/` 保留 1 版本 → 超 90 天的 → 移到 `plans/archive/`
- **不让文档过期**：每季度 review 一次 `plans/` 状态徽章；过期超 90 天的 plan 标 📦 已归档

---

**最后整理**：2026-08-23 · 详见 `docs/design-system-meeting.md`
