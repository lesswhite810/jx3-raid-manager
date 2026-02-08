# 账号数据库更新逻辑优化

## 问题分析

**当前行为**：`handleUseConfigDirectory` 扫描账号后，对于匹配的角色会**完全替换**整个角色对象。

**影响**：用户手动修改的字段（如门派 `sect`、装分 `equipmentScore`）会在扫描后被覆盖。

## 字段分类

| 字段类型 | 字段名 | 来源 | 更新时行为 |
|---------|-------|------|----------|
| **目录解析字段** | `role.name` | 游戏目录 | ✅ 覆盖 |
| | `role.region` | 游戏目录 | ✅ 覆盖 |
| | `role.server` | 游戏目录 | ✅ 覆盖 |
| | `account.accountName` | 游戏目录 | ✅ 覆盖 |
| **用户编辑字段** | `role.sect` | 用户手动设置 | ❌ 保留 |
| | `role.equipmentScore` | 用户手动设置 | ❌ 保留 |
| | `role.disabled` | 用户操作 | ❌ 保留 |
| | `role.isClient` | 用户操作 | ❌ 保留 |
| | `account.password` | 用户输入 | ❌ 保留 |
| | `account.notes` | 用户输入 | ❌ 保留 |
| | `account.hidden` | 用户操作 | ❌ 保留 |
| | `account.disabled` | 用户操作 | ❌ 保留 |
| **系统字段** | `role.id` | UUID | ❌ 保留 |
| | `account.id` | UUID | ❌ 保留 |

## 优化方案

### 方案：在 `directoryParser.ts` 添加智能合并函数

```typescript
/**
 * 智能合并扫描的角色与现有角色
 * 只更新目录解析的字段，保留用户编辑的字段
 */
export const mergeRoleWithScanned = (
  existingRole: Role,
  scannedRole: { name: string; region: string; server: string }
): Role => {
  return {
    ...existingRole,
    name: scannedRole.name,
    region: scannedRole.region,
    server: scannedRole.server,
    // 用户编辑的字段保持不变：
    // sect, equipmentScore, disabled, isClient, id
  };
};

/**
 * 智能合并扫描的账号与现有账号
 */
export const mergeAccountWithScanned = (
  existingAccount: Account,
  scannedAccount: ParsedAccount
): Account => {
  return {
    ...existingAccount,
    accountName: scannedAccount.accountName,
    roles: mergeRoles(existingAccount.roles, scannedAccount.roles)
  };
};
```

### 需要修改的代码位置

1. **`services/directoryParser.ts`** - 添加智能合并函数
2. **`components/AccountManager.tsx:306-370`** - 修改合并逻辑

## 确认后的决策

1. ✅ **角色 ID 处理**：保留现有 ID（目录解析不包含 `role.id`，它是自动生成的）
2. ✅ **合并策略**：精确匹配（name + region + server 三元组）
3. ✅ **变更日志**：需要详细日志
