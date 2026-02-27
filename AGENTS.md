# AGENTS.md

本文档为 AI 编码代理提供 JX3 Raid Manager 代码库的开发指南。

## 1. 项目概述

JX3 Raid Manager (剑网三副本管家) 是一个本地化副本数据管理桌面应用。

| 层级 | 技术栈 |
|------|--------|
| 前端 | React 18 + TypeScript + Vite + Tailwind CSS |
| 后端 | Rust (Tauri) |
| 数据库 | SQLite (由 Rust 管理) |
| 图标 | Lucide React |

## 2. 构建与开发命令

```bash
# 安装依赖
npm install

# 开发模式 (前端 + Tauri)
npm run tauri dev

# 仅前端开发 (浏览器预览)
npm run dev

# 生产构建
npm run tauri build

# 类型检查
npm run build   # 执行 tsc && vite build
```

### 测试
- **单元测试**: 使用 Vitest (`npm run test`)
- **后台 API 测试**: 通过 MCP Bridge 连接运行中的应用进行测试
- **手动验证**: 通过 `npm run tauri dev` 在真实环境中测试 Tauri IPC 调用

#### 后台 API 测试方法

启动应用后，通过 MCP Bridge (端口 9223) 执行测试脚本：

```javascript
(async () => {
  try {
    const result = await window.__TAURI__.core.invoke('db_get_accounts_structured');
    return JSON.stringify({ success: true, data: JSON.parse(result) });
  } catch (e) {
    return JSON.stringify({ success: false, error: e.message });
  }
})()
```

#### API 测试用例清单

| 类别 | API | 说明 | 状态 |
|------|-----|------|------|
| 版本管理 | `db_get_version_info` | 获取数据库版本 | ✅ |
| 账号管理 | `db_get_accounts_structured` | 获取账号列表（不含角色） | ✅ |
| 账号管理 | `db_get_accounts_with_roles` | 获取账号列表（含角色，LEFT JOIN 优化） | ✅ |
| 角色管理 | `db_get_all_roles` | 获取所有角色 | ✅ |
| 角色管理 | `db_get_roles_by_account` | 按账号获取角色 | ✅ |
| 副本记录 | `db_get_records`, `db_add_record`, `db_delete_record` | 副本记录 CRUD | ✅ |
| 配置管理 | `db_get_config`, `db_save_config` | 应用配置 | ✅ |
| 试炼记录 | `db_get_trial_records`, `db_add_trial_record` | 试炼之地记录 | ✅ |
| 百战记录 | `db_get_baizhan_records`, `db_add_baizhan_record` | 百战记录 | ✅ |
| 收藏功能 | `db_get_favorite_raids`, `db_add_favorite_raid` | 副本收藏 | ✅ |

#### 已知问题修复记录 (v2.0.0)

1. **`db_get_accounts_structured` 字段索引错误**
   - 问题：SELECT 字段顺序与 `row.get()` 索引不匹配
   - 修复：正确映射 hidden/disabled/password/notes 字段索引

2. **`db_get_accounts_with_roles` 查询优化**
   - 优化：从 2 次独立查询改为 1 次 LEFT JOIN 查询

详细测试文档见 `docs/TEST_CASES.md`

## 3. 架构与数据流

### 前后端通信
- **模式**: 通过 Tauri `invoke` 进行请求/响应通信
- **前端封装**: `services/db.ts` 封装所有 Rust 命令
- **命名约定**:
  - 前端方法: `camelCase` (如 `getAccounts`)
  - Rust 命令: `snake_case` (如 `db_get_accounts`)

### 数据持久化
- **主数据源**: 本地 SQLite 数据库 (Rust 管理)
- **前端状态**:
  - `services/db.ts`: 数据访问层
  - `contexts/ThemeContext.tsx`: 全局状态
  - 组件内部: `useState` 管理临时 UI 状态

### 文件系统访问
- 使用 `@tauri-apps/api/fs` 扫描游戏目录
- `services/gameDirectoryScanner.ts`: 解析 `userdata` 和 `interface` 目录

## 4. 代码风格指南

### TypeScript 规范

```typescript
// ✅ 推荐: 使用 interface 定义对象类型
interface Role {
  id: string;
  name: string;
  server: string;
}

// ✅ 推荐: 使用 enum 定义固定值集合
export enum AccountType {
  OWN = 'OWN',
  CLIENT = 'CLIENT'
}

// ❌ 禁止: 类型错误抑制
// @ts-ignore        // 禁止
as any              // 禁止
// @ts-expect-error  // 禁止
```

**tsconfig.json 配置**:
- `strict: true` - 严格模式
- `noUnusedLocals: true` - 禁止未使用变量
- `noUnusedParameters: true` - 禁止未使用参数

### React 组件规范

```typescript
// ✅ 推荐: 函数组件 + 命名导出
interface DashboardProps {
  records: RaidRecord[];
  accounts: Account[];
  onShowDetail: () => void;
}

export const Dashboard: React.FC<DashboardProps> = ({ 
  records, 
  accounts, 
  onShowDetail 
}) => {
  // 数组安全检查
  const safeRecords = Array.isArray(records) ? records : [];
  
  // 使用 useMemo 优化性能
  const stats = useMemo(() => {
    return safeRecords.reduce((acc, r) => acc + r.goldIncome, 0);
  }, [safeRecords]);
  
  return (
    <div className="bg-surface rounded-xl p-5">
      {/* ... */}
    </div>
  );
};
```

### Hooks 规范

```typescript
// hooks/useCountdown.ts
export const useCountdown = (
  targetTime: number | Date,
  options: UseCountdownOptions = {}
): UseCountdownReturn => {
  // 使用 useRef 存储回调引用
  const onCompleteRef = useRef(onComplete);
  
  useEffect(() => {
    onCompleteRef.current = onComplete;
  }, [onComplete]);
  
  // 返回值使用 interface 定义
  return { days, hours, minutes, seconds, isExpired };
};

// hooks/index.ts - 桶式导出
export { useCountdown, CountdownDisplay } from './useCountdown';
```

### 命名约定

| 类型 | 约定 | 示例 |
|------|------|------|
| 组件 | PascalCase | `Dashboard.tsx`, `AddRecordModal.tsx` |
| Hooks | camelCase + use前缀 | `useCountdown.ts` |
| 工具函数 | camelCase | `formatCurrency.ts`, `sortAccounts.ts` |
| 常量 | UPPER_SNAKE_CASE | `DEFAULT_DURATION` |
| Rust 命令 | snake_case | `db_get_accounts` |
| CSS 变量 | kebab-case | `--bg-surface` |

## 5. 样式规范 (Tailwind CSS)

### 颜色语义化
使用语义化颜色名称，而非原始颜色值：

```tsx
// ✅ 推荐
<div className="bg-base text-main border-base">
<div className="bg-surface text-muted">

// ❌ 避免
<div className="bg-white text-slate-900">
```

### CSS 变量系统
```css
:root {
  --bg-base: 255 255 255;
  --bg-surface: 248 250 252;
  --text-main: 15 23 42;
  --text-muted: 100 116 139;
  --primary-base: 124 58 237;
}

[data-theme="dark"] {
  --bg-base: 15 23 42;
  --text-main: 226 232 240;
}
```

### 暗色模式
- 通过 `[data-theme="dark"]` 选择器支持
- 由 `ThemeContext` 自动管理主题切换

## 6. 错误处理

### IPC/异步操作
```typescript
// ✅ 推荐: 完整的错误处理
try {
  await db.saveRecord(data);
  toast.success('保存成功');
} catch (error) {
  console.error('Failed to save record:', error);
  toast.error('保存失败，请重试');
}
```

### Toast 用户反馈
```typescript
import { toast } from '../utils/toastManager';

// 便捷方法
toast.success('操作成功');
toast.error('操作失败');
toast.warning('请注意');
toast.info('提示信息');

// 带自定义时长
toast.success('保存成功', 5000);
```

## 7. 目录结构

```
├── components/     # React 组件 (Dashboard, Modals 等)
├── contexts/       # React Context (ThemeContext)
├── hooks/          # 自定义 Hooks (index.ts 桶式导出)
├── services/       # 业务逻辑层 (db.ts, scanner 等)
├── utils/          # 纯工具函数 (uuid, toast 等)
├── data/           # 静态数据文件
├── constants.ts    # 全局常量
├── types.ts        # TypeScript 类型定义
├── App.tsx         # 根组件
├── index.tsx       # 入口文件
└── src-tauri/      # Rust 后端代码
```

## 8. Git 工作流

- **提交信息**: 使用中文 (项目为中文本地化)
- **格式**: `类型: 描述`
- **示例**:
  - `feat: 新增副本收益统计图表`
  - `fix: 修复自动扫描路径错误`
  - `refactor: 重构账号管理组件`

## 9. 代理专用规则

1. **先分析后修改**: 修改逻辑前，检查 `services/db.ts` 和 `src-tauri/` 中的 Rust 代码
2. **安全重构**: 修改 `db.ts` 时，确保对应的 Rust 命令存在或正在添加
3. **禁止臆造**: 不要臆造不存在的 Tauri 命令
4. **本地化**: 所有 UI 文本必须使用简体中文
5. **类型安全**: 严禁使用 `as any` 或 `@ts-ignore` 绕过类型检查
6. **文档同步**: 新增功能、修复 Bug、版本更新时，必须同步更新 `README.md` 中的更新日志

## 10. 常见代码模式

### 数据加载
```typescript
useEffect(() => {
  const loadData = async () => {
    try {
      const data = await db.getAccounts();
      setAccounts(data);
    } catch (error) {
      console.error('加载数据失败:', error);
    }
  };
  loadData();
}, []);
```

### 条件渲染
```typescript
// 加载状态
if (!isInitialized) {
  return <LoadingSpinner text="正在加载..." />;
}

// 空状态
if (records.length === 0) {
  return <EmptyState message="暂无记录" />;
}
```

### 列表渲染
```typescript
// 始终进行数组安全检查
const safeRecords = Array.isArray(records) ? records : [];

return safeRecords.map(record => (
  <RecordCard key={record.id} record={record} />
));
```
