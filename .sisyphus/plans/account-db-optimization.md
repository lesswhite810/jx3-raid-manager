# 优化账号数据库更新逻辑 - 只更新目录解析字段

## TL;DR

> **快速摘要**：修改账号扫描合并逻辑，智能区分"目录解析字段"和"用户编辑字段"，扫描更新时只覆盖目录解析字段，保留用户手动修改的字段（门派、装分等）。
>
> **核心修改**：
> - 目录解析字段：`accountName`, `name`, `region`, `server`
> - 用户编辑字段：`sect`, `equipmentScore`, `disabled`, `isClient`, `password`, `notes`, `hidden`, `disabled`
>
> **交付物**：
> 1. `services/directoryParser.ts` - 智能合并函数
> 2. `components/AccountManager.tsx` - 调用合并函数
>
> **预计工作量**：约 1-2 小时
> **测试验证**：本地目录扫描测试

---

## Context

### 原始请求
用户反馈：优化更新数据库逻辑，只更新目录结构解析出来的字段。

### 需求澄清

**用户已确认**：
1. ✅ `role.id` 由系统自动生成，目录解析不包含此字段
2. ✅ 合并策略：精确匹配（name + region + server 三元组）
3. ✅ 需要详细日志记录字段变更

**当前问题代码位置**：`components/AccountManager.tsx:306-370`
```typescript
// 问题：完全替换角色对象，导致用户编辑字段丢失
updatedAccountsMap.set(existingAccount.id, {
  ...existingAccount,
  roles: sortRoles(currentRoles)  // currentRoles 中已完全替换为扫描结果
});
```

**影响范围**：
- `services/directoryParser.ts` - 添加合并函数
- `components/AccountManager.tsx` - 修改合并调用
- 单元测试覆盖

---

## Work Objectives

### Core Objective
修改账号扫描合并逻辑，智能区分字段类型，更新时只覆盖目录解析字段，保留用户手动修改的字段。

### Concrete Deliverables
1. **`services/directoryParser.ts`** - 新增智能合并函数
   - `mergeRoles()` - 智能合并角色列表
   - `mergeRoleWithScanned()` - 单个角色智能合并
   - `mergeAccountWithScanned()` - 账号智能合并
   - `SCANNED_FIELDS` - 常量定义目录解析字段

2. **`components/AccountManager.tsx`** - 修改合并逻辑
   - 导入新增的合并函数
   - 替换原有的完全替换逻辑
   - 添加变更日志输出

3. **测试验证**
   - 测试目录扫描合并功能
   - 验证用户编辑字段不被覆盖

### Definition of Done
- [ ] 目录扫描后，用户手动设置的门派（sect）保留
- [ ] 目录扫描后，用户设置的装分（equipmentScore）保留
- [ ] 目录扫描后，禁用状态（disabled）保留
- [ ] 目录扫描后，密码和备注保留
- [ ] 目录解析字段（name, region, server, accountName）正确更新
- [ ] 控制台输出详细变更日志

### Must Have
- 字段分类清晰，可维护
- 日志输出详细，便于排查
- 保留所有现有功能兼容性

### Must NOT Have (Guardrails)
- ❌ 不能丢失现有的账号/角色关联关系
- ❌ 不能影响账号类型（OWN/CLIENT）的判断逻辑
- ❌ 不能破坏现有 UI 操作流程
- ❌ 不能修改数据库 schema

---

## Verification Strategy

### Test Decision
- **Infrastructure exists**: YES (Vite + React Testing Library)
- **Automated tests**: Tests after (添加合并逻辑后测试)
- **Framework**: React Testing Library

### Agent-Executed QA Scenarios (MANDATORY)

**验证工具**: Bash (npm test / Manual UI testing)

**Scenario: 扫描后用户编辑字段保留**
```bash
# 前提：已存在账号包含角色，角色有 sect="纯阳" equipmentScore=50000
# 操作：执行目录扫描
# 验证：
# 1. 扫描后角色sect仍为"纯阳"
# 2. 扫描后equipmentScore仍为50000
# 3. 控制台输出日志包含 "字段变更: sect=保留"
```

**Scenario: 扫描后目录解析字段更新**
```bash
# 前提：现有角色 region="电信区" server="梦江南"
# 操作：扫描后目录结构变化（如角色迁移到新服）
# 验证：
# 1. 角色region和server正确更新
# 2. 其他用户编辑字段保留
```

**Scenario: 精确匹配合并**
```bash
# 前提：两个账号下分别有名为"测试角色"但在不同区服的同名角色
# 操作：扫描目录
# 验证：
# 1. 两个角色都保留，作为独立角色
# 2. 不发生错误合并
```

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (立即开始):
├── Task 1: 添加字段分类常量和类型定义
└── Task 2: 实现 mergeRoleWithScanned 函数

Wave 2 (After Wave 1):
├── Task 3: 实现 mergeRoles 函数
└── Task 4: 实现 mergeAccountWithScanned 函数

Wave 3 (After Wave 2):
├── Task 5: 修改 AccountManager.tsx 调用合并函数
└── Task 6: 添加详细变更日志

Wave 4 (After Wave 3):
└── Task 7: 本地测试验证
```

### Dependency Matrix

| Task | Depends On | Blocks | Can Parallelize With |
|------|------------|--------|---------------------|
| 1 | None | 2 | None |
| 2 | 1 | 3 | None |
| 3 | 2 | 4 | None |
| 4 | 3 | 5 | None |
| 5 | 4 | 6 | None |
| 6 | 5 | 7 | None |
| 7 | 6 | None | None |

---

## TODOs

> Implementation + Test = ONE Task. Never separate.

- [ ] 1. 添加字段分类常量和类型定义

  **What to do**:
  - 在 `services/directoryParser.ts` 中定义 `SCANNED_FIELDS` 常量
  - 定义用户编辑字段列表 `USER_EDITED_FIELDS`
  - 导出类型 `ScannedFields` 和 `UserEditedFields`

  **Must NOT do**:
  - 不修改现有的 `ParsedAccount` 接口
  - 不删除现有函数

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 简单的类型定义和常量添加
  - **Skills**: `[]`
  - **Skills Evaluated but Omitted**:
    - N/A - 简单任务

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1
  - **Blocks**: Task 2
  - **Blocked By**: None (can start immediately)

  **References**:

  **Pattern References** (existing code to follow):
  - `services/directoryParser.ts:17-24` - `ParsedAccount` 接口定义模式

  **Test References** (testing patterns to follow):
  - N/A - 此任务不需要测试

  **Acceptance Criteria**:
  - [ ] `SCANNED_FIELDS` 常量定义目录解析字段：`accountName`, `name`, `region`, `server`
  - [ ] `USER_EDITED_FIELDS` 数组定义用户编辑字段
  - [ ] 类型定义通过 TypeScript 编译检查

  **Evidence to Capture**:
  - [ ] TypeScript 编译输出无错误

  **Commit**: YES (with Task 2)
  - Message: `feat(account): 优化账号合并逻辑 - 字段分类`
  - Files: `services/directoryParser.ts`

---

- [ ] 2. 实现 mergeRoleWithScanned 函数

  **What to do**:
  - 接收 `existingRole: Role` 和 `scannedRole: { name, region, server }`
  - 只更新 `name`, `region`, `server` 字段
  - 保留 `sect`, `equipmentScore`, `disabled`, `isClient`, `id` 等字段
  - 添加日志记录变更情况

  **Must NOT do**:
  - 不修改 `role.id` 字段
  - 不修改账号级别的字段

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 简单函数实现
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1
  - **Blocks**: Task 3
  - **Blocked By**: Task 1

  **References**:

  **Pattern References** (existing code to follow):
  - `services/directoryParser.ts:38-62` - `convertToSystemAccounts` 函数模式
  - `types.ts:11-20` - `Role` 接口定义

  **Acceptance Criteria**:

  **If Tests Enabled (tests after)**:
  - [ ] 测试用例：合并时 sect 字段保留
  - [ ] 测试用例：合并时 equipmentScore 字段保留
  - [ ] 测试用例：合并时 name/region/server 更新

  **Agent-Executed QA Scenarios**:

  \`\`\`
  Scenario: mergeRoleWithScanned 保留用户编辑字段
    Tool: Bash (TypeScript compilation + manual verification)
    Preconditions: TypeScript compilation succeeds
    Steps:
      1. tsc --noEmit services/directoryParser.ts
      2. Verify no compilation errors
      3. Manually verify function logic:
         - Input: existingRole with sect="纯阳", equipmentScore=50000
         - scannedRole with name="少年白了发", region="电信区", server="梦江南"
         - Output: role with sect="纯阳", equipmentScore=50000, and updated name/region/server
    Expected Result: TypeScript compiles, fields correctly merged
    Evidence: Compilation output

  Scenario: 字段变更日志输出
    Tool: Bash (console.log verification)
    Preconditions: Function implementation complete
    Steps:
      1. Call mergeRoleWithScanned with test data
      2. Verify console.log outputs: "保留字段: sect, equipmentScore"
      3. Verify console.log outputs: "更新字段: name, region, server"
    Expected Result: Detailed logging as required
    Evidence: Console output
  \`\`\`

  **Evidence to Capture**:
  - [ ] TypeScript 编译输出
  - [ ] 单元测试通过

  **Commit**: YES (with Task 1)
  - Message: `feat(account): 实现 mergeRoleWithScanned 智能合并函数`
  - Files: `services/directoryParser.ts`

---

- [ ] 3. 实现 mergeRoles 智能合并角色列表函数

  **What to do**:
  - 接收 `existingRoles: Role[]` 和 `scannedRoles: { name, region, server }[]`
  - 对每个扫描的角色，在现有列表中查找精确匹配（name + region + server）
  - 匹配成功：调用 `mergeRoleWithScanned` 合并
  - 匹配失败：作为新角色添加
  - 返回合并后的角色列表

  **Must NOT do**:
  - 不修改不存在的角色的字段
  - 不使用模糊匹配

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 数组处理和匹配逻辑
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 4
  - **Blocked By**: Task 2

  **References**:

  **Pattern References** (existing code to follow):
  - `components/AccountManager.ts:316-360` - 现有的合并逻辑（参考但需要替换）

  **Acceptance Criteria**:

  **Agent-Executed QA Scenarios**:

  \`\`\`
  Scenario: 精确匹配角色合并
    Tool: Bash
    Preconditions: mergeRoles function complete
    Steps:
      1. existingRoles: [{id:"1", name:"角色A", region:"电信区", server:"梦江南", sect:"纯阳"}]
      2. scannedRoles: [{name:"角色A", region:"电信区", server:"梦江南"}]
      3. Call mergeRoles(existingRoles, scannedRoles)
      4. Verify result has 1 role with id="1", sect="纯阳"
    Expected Result: Role merged correctly, fields preserved
    Evidence: Function return value

  Scenario: 新角色添加
    Tool: Bash
    Preconditions: mergeRoles function complete
    Steps:
      1. existingRoles: [{id:"1", name:"角色A", region:"电信区", server:"梦江南"}]
      2. scannedRoles: [{name:"角色B", region:"电信区", server:"梦江南"}]
      3. Call mergeRoles(existingRoles, scannedRoles)
      4. Verify result has 2 roles (original + new)
    Expected Result: New role added
    Evidence: Function return value

  Scenario: 同名不同区服角色
    Tool: Bash
    Preconditions: mergeRoles function complete
    Steps:
      1. existingRoles: [{id:"1", name:"测试号", region:"电信区", server:"梦江南"}]
      2. scannedRoles: [{name:"测试号", region:"电信区", server:"绝代天骄"}]
      3. Call mergeRoles(existingRoles, scannedRoles)
      4. Verify result has 2 roles (both preserved)
    Expected Result: Two separate roles for same name different server
    Evidence: Function return value
  \`\`\`

  **Evidence to Capture**:
  - [ ] 单元测试通过

  **Commit**: YES (with Task 4)
  - Message: `feat(account): 实现 mergeRoles 智能合并函数`
  - Files: `services/directoryParser.ts`

---

- [ ] 4. 实现 mergeAccountWithScanned 账号智能合并函数

  **What to do**:
  - 接收 `existingAccount: Account` 和 `scannedAccount: ParsedAccount`
  - 更新 `accountName` 字段
  - 调用 `mergeRoles` 合并角色列表
  - 保留所有账号级别的用户编辑字段

  **Must NOT do**:
  - 不修改账号 ID
  - 不修改账号类型（type）

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 简单包装函数
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 5
  - **Blocked By**: Task 3

  **References**:

  **Pattern References** (existing code to follow):
  - `services/directoryParser.ts:38-62` - `convertToSystemAccounts` 模式
  - `types.ts:22-32` - `Account` 接口

  **Acceptance Criteria**:
  - [ ] `accountName` 正确更新
  - [ ] `password`, `notes` 等用户编辑字段保留
  - [ ] `type` (OWN/CLIENT) 保留

  **Commit**: YES (with Task 3)
  - Message: `feat(account): 实现 mergeAccountWithScanned 账号合并函数`
  - Files: `services/directoryParser.ts`

---

- [ ] 5. 修改 AccountManager.tsx 调用合并函数

  **What to do**:
  - 导入 `mergeAccountWithScanned` 函数
  - 修改 `handleUseConfigDirectory` 中的合并逻辑
  - 用新的智能合并函数替换原有的完全替换逻辑

  **Must NOT do**:
  - 不改变 UI 交互流程
  - 不改变 toast 消息内容

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 修改现有代码逻辑
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 6
  - **Blocked By**: Task 4

  **References**:

  **Pattern References** (existing code to follow):
  - `components/AccountManager.ts:279-393` - `handleUseConfigDirectory` 完整函数
  - `components/AccountManager.ts:5` - 现有 import 语句

  **Acceptance Criteria**:
  - [ ] 导入语句正确添加
  - [ ] 合并逻辑替换为调用 `mergeAccountWithScanned`
  - [ ] TypeScript 编译通过

  **Commit**: YES (with Task 6)
  - Message: `refactor(account): 使用智能合并函数替换完全替换逻辑`
  - Files: `components/AccountManager.tsx`

---

- [ ] 6. 添加详细变更日志

  **What to do**:
  - 在合并过程中添加详细日志输出
  - 记录保留的字段和更新的字段
  - 记录新增角色和删除角色情况
  - 日志格式： `[账号扫描] [INFO] 保留字段: sect, equipmentScore`

  **Must NOT do**:
  - 不添加敏感信息到日志
  - 不影响程序性能

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 添加日志语句
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 7
  - **Blocked By**: Task 5

  **References**:

  **Pattern References** (existing code to follow):
  - `services/gameDirectoryScanner.ts` - 现有的日志模式

  **Acceptance Criteria**:
  - [ ] 控制台输出包含变更日志
  - [ ] 日志信息清晰易懂

  **Commit**: YES (with Task 5)
  - Message: `feat(account): 添加账号合并详细日志`
  - Files: `services/directoryParser.ts`

---

- [ ] 7. 本地测试验证

  **What to do**:
  - 执行目录扫描功能
  - 验证用户编辑字段保留
  - 验证目录解析字段更新
  - 验证日志输出

  **Must NOT do**:
  - 不修改生产环境数据（使用测试账号）

  **Recommended Agent Profile**:
  - **Category**: `visual-engineering`
    - Reason: 需要手动验证 UI 和功能
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 4
  - **Blocks**: None
  - **Blocked By**: Task 6

  **Acceptance Criteria**:

  **Agent-Executed QA Scenarios**:

  \`\`\`
  Scenario: 完整流程测试 - 用户字段保留
    Tool: Manual UI Testing
    Preconditions: 本地开发环境运行
    Steps:
      1. 打开账号管理页面
      2. 编辑角色，设置 sect="万花", equipmentScore=60000
      3. 点击"目录解析"按钮
      4. 扫描完成后，检查角色信息
      5. 验证 sect 仍为 "万花"
      6. 验证 equipmentScore 仍为 60000
      7. 验证控制台日志包含 "保留字段: sect, equipmentScore"
    Expected Result: 所有用户编辑字段保留
    Evidence: Screenshot of UI showing preserved fields

  Scenario: 完整流程测试 - 目录字段更新
    Tool: Manual UI Testing
    Preconditions: 本地开发环境运行
    Steps:
      1. 现有角色 region="电信区", server="梦江南"
      2. 修改游戏目录，模拟角色迁移
      3. 执行目录扫描
      4. 验证角色 region/server 已更新
      5. 验证其他字段保留
      6. 验证控制台日志包含 "更新字段: region, server"
    Expected Result: 目录解析字段正确更新
    Evidence: Screenshot of updated fields
  \`\`\`

  **Evidence to Capture**:
  - [ ] 测试截图
  - [ ] 控制台日志截图

  **Commit**: NO
  - Message: N/A
  - Files: N/A

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| 1-2 | `feat(account): 添加字段分类和 mergeRoleWithScanned 函数` | `services/directoryParser.ts` | TypeScript 编译 |
| 3-4 | `feat(account): 实现 mergeRoles 和 mergeAccountWithScanned 函数` | `services/directoryParser.ts` | 单元测试 |
| 5-6 | `refactor(account): 使用智能合并函数并添加日志` | `components/AccountManager.tsx` | TypeScript 编译 |

---

## Success Criteria

### Verification Commands
```bash
# TypeScript 编译检查
npm run build

# 启动开发环境测试
npm run dev
```

### Final Checklist
- [ ] 所有 Must Have 条件满足
- [ ] 所有 Must NOT Have 条件未触发
- [ ] TypeScript 编译无错误
- [ ] 用户编辑字段保留（sect, equipmentScore）
- [ ] 目录解析字段更新（name, region, server）
- [ ] 详细日志输出正确
