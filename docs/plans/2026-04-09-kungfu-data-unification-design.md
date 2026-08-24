# 心法数据统一方案设计

> 📦 **本计划已归档** · 归档日期：2026-08-23
> 完整内容请见 `docs/plans/archive/2026-04-09-kungfu-data-unification-design.md`

> **落地状态**：✅ 已实现（v2.1.30+ · vite-plugin-kungfu-data）
> **归档原因**：已落地超过 90 天，作为历史档案保留

---

## 关联资产

- **源数据**：`data/kungfu_data.json`
- **生成产物**：`src/data/kungfuData.ts`（由 vite-plugin-kungfu-data 自动生成）
- **业务影响**：心法 / 门派图标、简称、归属关系统一从此 JSON 派生
- **当前文档**：见 `contexts/context.md §数据源唯一性` 与 `specs/design-tokens.md`

---

**最后整理**：2026-08-23
