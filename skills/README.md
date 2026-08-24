# `skills/` AI Agent Skill 包

> 给外部 AI 工具（Claude Code / Cursor / Codex / Roo Code 等）使用的项目专属 skill manifest
> 最后整理：2026-08-23

---

## 📚 Skill 文件

| 文件 | 用途 |
|---|---|
| [`jx3-raid-manager-project/SKILL.md`](./jx3-raid-manager-project/SKILL.md) | JX3 Raid Manager 项目专属 skill · 触发词：React/Tauri 改动、发版、中文文档、账号管理、UI 规范 |

> ℹ️ 魔盒发布 skill `jx3-release-publisher` 未放在本项目内（避免项目仓库携带本机发布能力），
> 它以 Junction 形式挂在 DSH 全局技能目录：`C:\Users\lessw\.agents\skills\jx3-release-publisher`
> → 源文件 `C:\Users\lessw\.cc-switch\skills\jx3-release-publisher`。

---

## 🔀 与顶层 `CLAUDE.md` / `AGENTS.md` 的关系

```
skills/jx3-raid-manager-project/SKILL.md   ← 外部 AI 工具的 skill manifest
                              ↓ 引用
contexts/context.md          ← 业务背景(给 AI 看)
specs/design-tokens.md       ← 设计规范(给 AI 看)
docs/release-process.md      ← 发版流程(给 AI 看)
                              ↑ 同时被
CLAUDE.md                    ← Claude Code 路由指南(对话开始时读)
AGENTS.md                    ← AI 编码代理红线(项目级硬约束)
```

### 关键区别

| 文档 | 受众 | 何时读 |
|---|---|---|
| `CLAUDE.md` | **Claude Code 对话** | 每个新对话开头 |
| `AGENTS.md` | **AI 编码代理**（任何工具） | 修改代码前必读 |
| `skills/.../SKILL.md` | **外部 AI 工具的 skill 加载** | 工具启动时按 `description` 触发词加载 |

> 简单说：`CLAUDE.md` / `AGENTS.md` 是**项目内**约定；`skills/.../SKILL.md` 是**对外发布**的 skill 描述，给别的 AI 工具发现和加载。

---

## 📝 维护约定

### 新增 skill

1. 在 `skills/<skill-name>/` 下创建 `SKILL.md`
2. YAML frontmatter 必填：
   ```yaml
   ---
   name: <kebab-case>
   description: Use when <具体触发场景>...
   ---
   ```
3. 在本 README 表格里登记

### 更新 skill

- 触发词变了 → 改 `description` 字段
- 项目结构变了 → 更新 `Start Here` / `High-Value Files` 段
- 红线变了 → 与 `AGENTS.md` 同步（SSOT 原则：`AGENTS.md` 是硬约束，skill 是软描述）

### 与 `AGENTS.md` 的边界

- `AGENTS.md §4 开发规范` 是**硬约束**（代码 / 文档 lint 会检查）
- `SKILL.md Core Rules` 是**软提示**（给 AI 看，不能依赖代码 lint）
- 两者冲突时，**以 `AGENTS.md` 为准**

---

## 🧪 验证 skill 是否仍有效

```bash
# 触发词扫描:description 里的关键词是否覆盖实际工作场景
grep -E "Use when" skills/*/SKILL.md

# 引用路径是否仍然存在(防文件移动后链接断裂)
grep -oE '[a-zA-Z_-]+/[a-zA-Z_-]+\.(md|tsx|ts|json)' skills/*/SKILL.md | sort -u | while read p; do
  [ -f "$p" ] || echo "✗ 引用断裂: $p"
done
```

---

**最后整理**：2026-08-23