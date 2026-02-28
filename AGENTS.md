# AGENTS.md

本文档为 AI 编码代理提供 JX3 Raid Manager 代码库的开发指南。

## 1. 核心指令 & 开发向导

> **⚠️ 业务与架构全景指引**
> 本应用的核心业务逻辑、深层目录结构拆解已被分离至专用上下文文件中。
> 如需寻找“JX3 Raid Manager 的详细模块作用”、“架构技术栈数据流”，请**必须查阅**：
> 👉 `contexts/context.md`

### 1.1 构建与开发命令

```bash
# 开发模式 (前端 + Tauri)
npm run tauri dev

# 生产构建
npm run tauri build

# 类型检查 (跑通即可确认 TypeScript 无错误)
npm run build
```

## 2. API 测试与后台通信规范

### 2.1 前后端通信约定
- **模式**: 通过 Tauri `invoke` 进行请求/响应通信
- **端侧封装**: 位于 `services/db.ts`，前端统一使用 `camelCase` (如 `getAccounts`) 以隐藏系统实现
- **Rust 后端**: 命令皆为 `snake_case` (如 `db_get_accounts`)

### 2.2 测试与运行预留
- **后台 API 测试方法**: 若开发功能遭遇 Tauri 无法调用，在启动应用后可通过 MCP Bridge (端口 9223) 执行下面格式的联调脚本进行测试：

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

*(附注：完整的 API 返回与用例细节记录于 `docs/TEST_CASES.md`)*

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

> **⚠️ 核心 UI 规约指引**
> 本项目已采用全面扁平化、极简风格的 UI 规范，关于 Tailwind 语义色 (`emerald`, `amber` 等)、容器阴影、组件交互状态的具体使用准则，请**必须查阅**并在编写前遵循：
> 👉 `specs/design-tokens.md`

### 基础原则（摘要）
- **语义化命名**：严格使用语义色 (`bg-surface`, `text-muted` 等)，避免原始色值 (`bg-white` 等)。
- **一致性**：各类状态标签激活色系跨功能统一为交互绿（详见规范表）。
- **去冗余**：克制使用图标，杜绝复杂的线性渐变与深邃的卡片悬浮阴影。

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
