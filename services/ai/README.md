# AI功能处理模块文档

## 概述

AI功能处理模块是一个独立、模块化的AI服务系统，专为剑网三副本管家应用设计。该模块采用高内聚低耦合的设计原则，提供了完整的AI模型管理、配置、连通性测试和请求处理功能。

## 模块结构

```
services/ai/
├── types.ts              # 类型定义
├── modelManager.ts       # 模型管理
├── configManager.ts      # 配置管理（整合了ConfigManager.tsx的AI功能）
├── connectionTester.ts    # 连通性测试
├── requestHandler.ts     # 请求处理
├── aiService.ts         # 统一服务接口
├── configAdapter.ts     # 配置适配器（用于平滑迁移）
├── tests.ts             # 测试文件
├── examples.ts          # 使用示例
└── index.ts             # 模块导出
```

## 核心功能

### 1. 模型管理功能

#### 功能描述
- 获取模型列表（优先从缓存读取）
- 获取当前已选择模型
- 模型切换和管理
- 模型缓存管理

#### 主要类和方法
```typescript
class AIModelManager {
  // 获取所有模型
  getModels(forceRefresh?: boolean): AIModel[]
  
  // 获取当前模型
  getCurrentModel(): AIModel | null
  
  // 设置当前模型
  setCurrentModel(modelId: string): boolean
  
  // 刷新模型列表
  refreshModels(): Promise<AIModel[]>
}
```

#### 使用示例
```typescript
import { modelManager } from './services/ai';

// 获取所有模型
const models = modelManager.getModels();

// 获取当前模型
const currentModel = modelManager.getCurrentModel();

// 切换模型
modelManager.setCurrentModel('glm-4.6');
```

### 2. 模型配置功能

#### 功能描述
- 模型切换接口
- API密钥管理
- 代理信息管理（区分国内外模型）
- 配置验证和持久化

#### 主要类和方法
```typescript
class AIConfigManager {
  // 获取配置
  getConfig(): AIServiceConfig
  
  // 更新配置
  updateConfig(updates: Partial<AIServiceConfig>): void
  
  // 设置API密钥
  setApiKey(apiKey: string): void
  
  // 切换模型
  switchModel(modelId: string): boolean
  
  // 验证配置
  validateConfig(): { isValid: boolean; error?: string }
}
```

#### 使用示例
```typescript
import { configManager } from './services/ai';

// 设置API密钥
configManager.setApiKey('your-api-key');

// 切换模型
configManager.switchModel('glm-4.6');

// 验证配置
const validation = configManager.validateConfig();
if (!validation.isValid) {
  console.error('配置无效:', validation.error);
}
```

### 3. 连通性测试功能

#### 功能描述
- 测试当前模型的连通性
- 发送测试请求并验证响应
- 提供详细的测试结果（网络状态、认证状态等）

#### 主要类和方法
```typescript
class AIConnectionTester {
  // 测试当前模型
  testCurrentModel(): Promise<AIConnectionTestResult>
  
  // 测试指定模型
  testModel(model: AIModel, config?: any): Promise<AIConnectionTestResult>
  
  // 批量测试模型
  testMultipleModels(models: AIModel[]): Promise<AIConnectionTestResult[]>
}
```

#### 使用示例
```typescript
import { connectionTester } from './services/ai';

// 测试当前模型
const result = await connectionTester.testCurrentModel();
if (result.success) {
  console.log('连接成功，延迟:', result.latency, 'ms');
} else {
  console.error('连接失败:', result.message);
}
```

### 4. AI请求处理功能

#### 功能描述
- 接收prompt参数
- 自动使用当前模型配置
- prompt格式化与请求参数拼接
- API响应处理和解析
- 错误处理机制

#### 主要类和方法
```typescript
class AIRequestHandler {
  // 发送AI请求
  sendRequest(params: AIRequestParams): Promise<AIResponse>
  
  // 格式化prompt
  formatPrompt(template: string, variables: Record<string, any>): string
  
  // 构建副本分析prompt
  buildRaidAnalysisPrompt(data: any): string
  
  // 流式请求处理
  sendStreamRequest(params: AIRequestParams, onChunk: Function): Promise<AIResponse>
}
```

#### 使用示例
```typescript
import { requestHandler } from './services/ai';

// 发送请求
const response = await requestHandler.sendRequest({
  prompt: '分析以下数据',
  systemPrompt: '你是一个数据分析助手',
  temperature: 0.7,
  maxTokens: 1000
});

if (response.success) {
  console.log('AI响应:', response.content);
} else {
  console.error('请求失败:', response.error);
}
```

## 统一服务接口

为了简化使用，模块提供了统一的服务接口：

```typescript
import { aiService } from './services/ai';

// 初始化服务
aiService.initialize();

// 获取模型列表
const models = aiService.getModels();

// 切换模型
aiService.switchModel('glm-4.6');

// 设置API密钥
aiService.setApiKey('your-api-key');

// 测试连接
const testResult = await aiService.testConnection();

// 分析副本数据
const analysis = await aiService.analyzeRaidData(records);
```

## 类型定义

模块提供了完整的类型定义，包括：

- `AIModel` - AI模型信息
- `AIRequestConfig` - AI请求配置
- `AIRequestParams` - AI请求参数
- `AIResponse` - AI响应结果
- `AIConnectionTestResult` - 连通性测试结果
- `AIServiceConfig` - AI服务配置
- `AIErrorType` - AI错误类型枚举

## 扩展性

模块设计具有良好的扩展性：

1. **添加新模型提供商**：
   - 在`AIProvider`枚举中添加新提供商
   - 在`requestHandler.ts`中实现对应的请求处理方法
   - 在`connectionTester.ts`中实现对应的测试方法

2. **添加新模型**：
   - 使用`modelManager.addModel()`方法添加新模型
   - 或修改`DEFAULT_MODELS`数组

3. **自定义请求处理**：
   - 扩展`AIRequestHandler`类
   - 添加新的请求处理方法

## 错误处理

模块提供了完善的错误处理机制：

1. **错误类型分类**：
   - 网络错误
   - 认证错误
   - 速率限制错误
   - 请求错误
   - 超时错误
   - 未知错误

2. **错误恢复**：
   - 自动重试机制（可配置）
   - 降级处理（使用备用模型）
   - 错误日志记录

## 最佳实践

1. **初始化**：
   ```typescript
   // 在应用启动时初始化AI服务
   import { aiService } from './services/ai';
   aiService.initialize();
   ```

2. **配置验证**：
   ```typescript
   // 在使用前验证配置
   const validation = aiService.validateConfig();
   if (!validation.isValid) {
     // 处理配置错误
   }
   ```

3. **错误处理**：
   ```typescript
   // 始终检查响应的success字段
   const response = await aiService.sendRequest(params);
   if (!response.success) {
     // 处理错误
     console.error('AI请求失败:', response.error);
   }
   ```

4. **资源清理**：
   ```typescript
   // 在应用关闭时清理资源
   // 模块会自动处理大部分清理工作
   ```

## 迁移指南

如果要从现有的`aiService.ts`迁移到新模块：

1. 替换导入：
   ```typescript
   // 旧代码
   import { analyzeRaidWithAI, AIServiceConfig } from '../services/aiService';
   
   // 新代码
   import { aiService } from '../services/ai';
   ```

2. 更新调用方式：
   ```typescript
   // 旧代码
   const result = await analyzeRaidWithAI(records, accounts, config);
   
   // 新代码
   const result = await aiService.analyzeRaidData(records);
   ```

3. 配置管理：
   ```typescript
   // 旧代码
   const config: AIServiceConfig = { ... };
   
   // 新代码
   aiService.updateConfig({ ... });
   aiService.setApiKey('your-api-key');
   ```

## 配置管理整合

### 从ConfigManager.tsx整合的功能

新的配置管理模块已整合了原有ConfigManager.tsx中的所有AI相关功能，包括：

1. **AI配置管理**：
   - API密钥设置和验证
   - 模型选择和切换
   - 温度参数调整
   - 代理设置管理
   - 配置验证和持久化

2. **事件系统**：
   - 配置变更事件监听
   - 模型切换事件通知
   - 错误和警告事件处理

3. **兼容性适配**：
   - 提供configAdapter.ts用于平滑迁移
   - 保持与原有代码接口兼容
   - 支持渐进式迁移策略

### 配置管理器增强功能

相比原有的ConfigManager.tsx，新的configManager.ts提供了以下增强：

1. **更好的类型安全**：
   - 完整的TypeScript类型定义
   - 严格的参数验证
   - 类型错误提示

2. **更灵活的事件系统**：
   - 支持多个事件监听器
   - 事件参数结构化
   - 错误处理机制

3. **更完善的验证逻辑**：
   - 实时配置验证
   - 详细的错误信息
   - 自动回滚机制

4. **更好的性能**：
   - 配置变更批处理
   - 减少不必要的存储操作
   - 事件驱动更新

### 使用配置适配器迁移

为了平滑迁移，可以使用configAdapter.ts：

```typescript
import AIConfigAdapter from './services/ai/configAdapter';

// 迁移现有配置
const legacyConfig = {
  enabled: true,
  apiKey: 'your-api-key',
  model: 'glm-4.6',
  temperature: 0.7,
  proxyUrl: 'http://127.0.0.1:7890',
  proxyEnabled: true
};

// 应用配置到新模块
AIConfigAdapter.migrateConfig(legacyConfig);

// 验证迁移结果
const validation = AIConfigAdapter.validateMigration();
if (validation.success) {
  console.log('迁移成功');
} else {
  console.log('迁移问题:', validation.issues);
}
```

## 注意事项

1. **API密钥安全**：
   - API密钥仅存储在本地
   - 不会发送到任何第三方服务器

2. **代理设置**：
   - 国内模型（如GLM）不需要代理
   - 国外模型（如Gemini）需要配置代理

3. **性能优化**：
   - 模型列表会缓存24小时
   - 请求支持超时设置
   - 大文件分析会限制记录数量

4. **兼容性**：
   - 模块与现有代码兼容
   - 可以逐步迁移，无需一次性替换所有代码