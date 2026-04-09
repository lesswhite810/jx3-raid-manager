# 心法数据统一方案设计

## 背景

当前项目中，门派和心法数据分散在多处：

- **Rust 后端**：
  - `get_force_name`: force_id → 门派名
  - `get_sect_by_martial`: 心法名 → 门派名
  - `build_kungfu_force_name_to_id_map`: (force_id, 心法名) → 心法ID

- **TypeScript 前端**：
  - `SECT_ICON_IDS`: 心法名 → 图标ID
  - `SECT_CONFIG`: 心法名 → 颜色/简称/名称
  - `MARTIAL_TO_SECT`: 心法名 → 门派名

问题：维护两份数据，人工同步容易出错（如"山海心决" vs "山海心诀"）。

## 设计方案

### 1. 统一数据文件

**文件**：`data/kungfu_data.json`

```json
{
  "forces": [
    {
      "id": 1,
      "name": "少林",
      "kungfus": [
        { "id": 10002, "name": "洗髓经", "color": "amber", "shortName": "和尚T" },
        { "id": 10003, "name": "易筋经", "color": "amber", "shortName": "和尚" }
      ]
    }
    // ... 其他门派
  ]
}
```

### 2. Rust 编译时嵌入

使用 `include_str!` 宏在编译时嵌入 JSON，无需运行时读取文件。

```rust
static KUNGFU_DATA: LazyLock<KungfuData> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../data/kungfu_data.json")).unwrap()
});
```

### 3. Vite 插件生成 TypeScript

**文件**：`vite-plugin-kungfu-data.ts`

- 每次构建时从 JSON 生成 TypeScript 文件
- 自动生成派生数据：MARTIAL_TO_SECT、MARTIAL_TO_ICON_ID、MARTIAL_TO_CONFIG

### 4. 生成的 TypeScript 文件

**文件**：`src/data/kungfuData.ts`（自动生成）

```typescript
export const KUNGFU_FORCES = [...];  // forces 数组
export const MARTIAL_TO_SECT = new Map([...]);  // 心法 → 门派
export const MARTIAL_TO_ICON_ID = new Map([...]); // 心法 → 图标ID
export const MARTIAL_TO_CONFIG = new Map([...]); // 心法 → 展示配置
```

## 实施步骤

1. 创建 `data/kungfu_data.json` 包含所有心法数据
2. 创建 Vite 插件 `vite-plugin-kungfu-data.ts`
3. 修改 `vite.config.ts` 集成插件
4. 修改 Rust 后端从 JSON 加载数据
5. 创建 `src/data/kungfuData.ts` 生成逻辑
6. 更新前端引用，使用新的统一数据
7. 删除重复的硬编码数据

## 删除的文件/代码

- `utils/sectConfig.ts` 中的 `SECT_ICON_IDS`、`SECT_CONFIG`、`MARTIAL_TO_SECT`
- `src-tauri/src/game_directory.rs` 中的硬编码 match 语句（`get_force_name`、`get_sect_by_martial`、`build_kungfu_force_name_to_id_map` 中的数据）
