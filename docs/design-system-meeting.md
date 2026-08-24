# 设计系统统一 · 团队评审会议议程

> 时间：2026-08-23 · 时长：60 分钟
> 主持人：[待定]
> 必到：设计负责人 / 前端主程 / Release Manager / 产品代表
> 背景文档：`tmp/design-zip/AUDIT-REPORT.md` · `tmp/design-zip/DESIGN.md` · `specs/design-tokens.md`

---

## 0. 会议目的（5 分钟）

让全员理解「以设计文档为权威」的具体含义，并通过 4 项关键决策。每项决策有 3 种选项（A/B/C），附推荐结论。

## 1. 当前进度（10 分钟）

### 1.1 已完成

| Step | 内容 | 验证 |
|---|---|---|
| 1.1 | `--primary-base` RGB 改为沉香褐 (`168 99 63`) | 169 处 `bg-primary` 自动变沉香褐；构建通过 |
| 1.2 | `emerald-*` / `amber-*` → `ds-success` / `ds-warning` 语义重命名 | **764 处** 跨 29 个文件 + 1 个数据 JSON（`data/kungfu_data.json`） |
| 1.3 | `preview/applied-surfaces.html` + `brand-assets.html` 5 处反模式修复 | 8px 圆角 → 2px，linear-gradient → 单色 + 描边 |
| 1.4 | `README.md §5.3` 审计脚本误报修复 | `grep -rE '^\s*--role-'` 仅匹配实际变量定义 |
| 2.1 | 9 条手动清单重跑 | **9/9 通过** |
| 2.2 | 综合烟测脚本 | `scripts/design-smoke-test.mjs` · 16 通过 / 4 已知约束 |

### 1.2 量化改动

- 修改文件数：32 个（30 个 `.tsx`/`.ts` + 1 个 `data/*.json` + 1 个 `index.css`）+ 2 个 `preview/*.html`
- 代码改动行：~700 行 Tailwind 类名替换
- 新增 CSS 变量：36 个（提案 §3 完整覆盖）
- 新增 Tailwind 工具类：`ds-*` × 14 个 + `rounded-card/tag` × 2 + `font-display/body/han/pingfang/mono` × 6 + `text-ds-xs..4xl` × 8 + `p-ds-1..8` × 8 + `shadow-ds-flat/stack/modal` × 3 + `duration-ds-fast/base/slow` × 3
- 删除：`bg-emerald-*` / `text-emerald-*` / `bg-amber-*` / `text-amber-*` 引用 = 0

---

## 2. 决策项（30 分钟 · 每项 5–8 分钟 + 表决）

### 决策 1 · 4 处 WCAG AA 对比度限制（紧急）

烟测发现 4 处 semantic color 在白底 + 正文 ≤18px 时不达 AA 4.5:1：

| Token | 实测 | 要求 | 影响场景 |
|---|---|---|---|
| `--success` vs 白底 | 4.07:1 | ≥4.5:1 | 收入卡内金额 / 收入勾选文字 |
| `--warning` vs 白底 | 3.60:1 | ≥4.5:1 | 支出卡内金额 / 警告文字 |
| `--info` vs 白底 | 4.30:1 | ≥4.5:1 | 副标题 / 链接文字 |
| `--accent` vs 米杏底 | 4.35:1 | ≥4.5:1 | 暖底色上的主操作按钮 |

**提案原文 §12.3 说**：≥4.5:1；§12.3 注：「暗主题 accent 必须比光主题**提亮 ≥ 1 个亮度档**」—— 提案在亮侧的低饱和度设计与 AA 正文标准存在张力。

#### 选项
- **A · 接受约束**（推荐短期）—— 限定 4 处语义色**仅用于 ≥18px 大字号 / Tag / 弱提示**；正文场景使用 `*-strong` 派生 token 或 `--fg` 主文本色
- **B · 提升饱和度** —— 调整 hex 至 `#4d7560 / #927237 / #506c80`，损失 12% 视觉降饱和度优势但通过 AA
- **C · 加 `*-strong` 派生 token** —— 新增 `--success-strong` / `--warning-strong` / `--info-strong` 用于正文（最佳实践）

**建议**：C 方案。既遵守提案色调、又过 AA。改动量：+3 个 token。

**表决**：__________

### 决策 2 · 标签语义升级

`tag-success` / `tag-warning` 这些`.tsx` 命名空间是否也升级为 `tag-ds-success` 等？目前提案 §4.2 已批准 `tag-{tone}` 短名。

#### 选项
- **A · 维持现状** —— 保留 `tag-success`；`bg-ds-success-soft` 是颜色 token，`tag-success` 是组件别名
- **B · 升级别名** —— 把 `bg-tag-success` / `text-tag-success` 加入 token 命名空间

**建议**：A。提案 §4.2 短名规范明确。

**表决**：__________

### 决策 3 · 字体配置 fallback 优先级

`tailwind.config.js fontFamily.sans` 仍默认为 Inter 链。`AGENTS.md §9` 明确禁止 Inter 作**展示字体**。

#### 选项
- **A · 保留 sans 默认**（当前） —— Inter 仅在 `font-sans` 显式调用时使用，新代码用 `font-display/body/mono`
- **B · sans 默认改为 PingFang SC** —— 影响范围大（无名 `font-sans` 调用全部受影响）
- **C · 把 sans 别名指向 font-body** —— 等价 B 但明确语义

**建议**：A。最小改动，AGENTS.md §9 已通过 `font-display/body` 规范覆盖。

**表决**：__________

### 决策 4 · 反向兼容窗口期

新的 `bg-ds-accent` 已经替代 `bg-primary` 245+ 处的视觉等价语义，但**类名层面**仍有 169 处 `bg-primary` 引用（语义上是「primary」不是「accent」）。团队是否要：

#### 选项
- **A · 半年后再次正则统一类名** —— 把 169 处 `bg-primary` 全部改名 `bg-ds-accent`（破坏性更新，需发 v3.0.0）
- **B · 永久保留双轨** —— `primary` 作兼容别名继续工作，新代码用 `ds-accent`
- **C · 触发条件改名** —— 当旧组件被重构到 50% 时一次性改名

**建议**：B。v2.x 一直保留；v3.0 重大重构时统一收口。

**表决**：__________

---

## 3. 后续任务（10 分钟 · 信息同步）

### 3.1 短期（一周内）

- [ ] 决策 1 选定方案 → 修 index.css → 重跑烟测
- [ ] 写 `docs/release-notes/v2.2.0.md` 草稿（CLAUDE.md §6 格式）
- [ ] 通知设计下游：Figma / 截图工具如有引用旧 `bg-emerald-*` 应同步更新

### 3.2 中期（一个月内）

- [ ] 把 `DsSuccessCard` / `DsWarningCard` 等推广模板写进 `specs/design-tokens.md §4`
- [ ] 给 `tailwind.config.js` 加 `"renderer"` 字段防止 4.x 升级时命名冲突
- [ ] 把 `tmp/design-zip/` 移到正式位置 `docs/design-system/` 或 `specs/design-system/`

### 3.3 长期（一季度内）

- [ ] 复审决策 4B：是否要升 v3.0 把 `primary` 完全移除
- [ ] 视觉回归测试 — `npm run tauri dev` 看每个主要页面对比（Dashboard / IncomeDetail / PendingRecordsPanel）
- [ ] Release Notes 走 GitHub Actions 自动写入

---

## 4. 关闭（5 分钟）

主持人确认：
1. 4 项决策的最终选项（填表）
2. 决策 1 / 决策 4 触发后续 PR 的 assignee
3. 下次会议日期（建议一周后）

会议记录归档到：`docs/meetings/2026-08-23-design-system.md`
