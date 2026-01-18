# AI模块迁移指南

本指南帮助开发者从现有的`aiService.ts`迁移到新的模块化AI系统。

## 迁移步骤

### 1. 更新导入语句

**旧代码：**
```typescript
import { analyzeRaidWithAI, AIServiceConfig } from '../services/aiService';
```

**新代码：**
```typescript
import { aiService } from '../services/ai';
// 或者导入特定模块
import { modelManager, configManager } from '../services/ai';
```

### 2. 更新AI分析调用

**旧代码：**
```typescript
const result = await analyzeRaidWithAI(records, accounts, config);
```

**新代码：**
```typescript
const result = await aiService.analyzeRaidData(records);
```

### 3. 更新配置管理

**旧代码：**
```typescript
const config: AIServiceConfig = {
  apiKey: 'your-key',
  model: 'glm-4.6',
  temperature: 0.7,
  proxyUrl: 'http://127.0.0.1:7890',
  proxyEnabled: true
};
```

**新代码：**
```typescript
// 设置API密钥
aiService.setApiKey('your-key');

// 切换模型
aiService.switchModel('glm-4.6');

// 更新其他配置
aiService.updateConfig({
  temperature: 0.7,
  proxyUrl: 'http://127.0.0.1:7890',
  proxyEnabled: true
});
```

### 4. 更新连通性测试

**旧代码：**
```typescript
import { testAIConnection } from '../services/aiService';
const result = await testAIConnection(config);
```

**新代码：**
```typescript
const result = await aiService.testConnection();
```

## 组件迁移示例

### Dashboard组件迁移

**旧代码：**
```typescript
import { analyzeRaidWithAI } from '../services/aiService';

const handleAIAnalysis = async () => {
  if (!config.ai.apiKey || !config.ai.model) {
    setAnalysis('请先在配置中设置AI功能');
    return;
  }
  
  const aiConfig = {
    apiKey: config.ai.apiKey,
    model: config.ai.model,
    temperature: config.ai.temperature,
    proxyUrl: config.ai.proxyUrl,
    proxyEnabled: config.ai.proxyEnabled
  };
  
  const result = await analyzeRaidWithAI(safeRecords, safeAccounts, aiConfig);
  
  if (result.success) {
    setAnalysis(result.content || '分析完成');
  } else {
    setAnalysis(result.error || '分析失败');
  }
};
```

**新代码：**
```typescript
import { aiService } from '../services/ai';

const handleAIAnalysis = async () => {
  // 验证配置
  const validation = aiService.validateConfig();
  if (!validation.isValid) {
    setAnalysis(validation.error || '配置无效');
    return;
  }
  
  // 分析数据
  const result = await aiService.analyzeRaidData(safeRecords);
  
  if (result.success) {
    setAnalysis(result.content || '分析完成');
  } else {
    setAnalysis(result.error || '分析失败');
  }
};
```

### ConfigManager组件迁移

**旧代码：**
```typescript
import { testAIConnection, supportedModels } from '../services/aiService';

const handleTestAIConnection = async () => {
  if (!config.ai.apiKey) {
    setTestResult({ success: false, message: '请先配置API密钥' });
    return;
  }

  setTestingAI(true);
  
  await new Promise(resolve => setTimeout(resolve, 1500));
  
  setTestingAI(false);
  const selectedModel = supportedModels.find(m => m.id === config.ai.model);
  if (selectedModel?.requiresProxy && !config.ai.proxyEnabled) {
    setTestResult({
      success: false,
      message: `${selectedModel.name}需要代理才能访问，请启用代理并配置有效的代理服务器`
    });
  } else {
    setTestResult({ success: true, message: `${selectedModel?.name || 'AI'} API连接成功` });
  }
};
```

**新代码：**
```typescript
import { aiService } from '../services/ai';

const handleTestAIConnection = async () => {
  setTestingAI(true);
  setTestResult(null);
  
  try {
    const result = await aiService.testConnection();
    setTestResult(result);
  } catch (error) {
    setTestResult({
      success: false,
      message: '测试过程中发生错误'
    });
  } finally {
    setTestingAI(false);
  }
};
```

## 渐进式迁移策略

### 阶段1：保持兼容性

1. 保留旧的`aiService.ts`文件
2. 在旧文件中添加新模块的适配器
3. 逐步迁移组件

```typescript
// 旧aiService.ts中的适配器
import { aiService as newAIService } from './ai';

export const analyzeRaidWithAI = async (records, accounts, config) => {
  // 设置配置
  newAIService.setApiKey(config.apiKey);
  newAIService.switchModel(config.model);
  newAIService.updateConfig({
    temperature: config.temperature,
    proxyUrl: config.proxyUrl,
    proxyEnabled: config.proxyEnabled
  });
  
  // 调用新方法
  return newAIService.analyzeRaidData(records);
};
```

### 阶段2：并行运行

1. 新旧代码同时存在
2. 通过特性开关控制使用哪个版本
3. 对比结果确保一致性

```typescript
const useNewAIService = featureFlags.useNewAIService;

const result = useNewAIService
  ? await newAIService.analyzeRaidData(records)
  : await analyzeRaidWithAI(records, accounts, config);
```

### 阶段3：完全迁移

1. 移除旧的`aiService.ts`文件
2. 更新所有导入
3. 清理不再需要的代码

## 注意事项

1. **API密钥处理**：
   - 新模块中API密钥是单独设置的
   - 不再需要在每次请求时传递

2. **模型切换**：
   - 新模块中模型切换是持久化的
   - 不需要手动管理模型状态

3. **错误处理**：
   - 新模块提供了更详细的错误类型
   - 可以根据错误类型进行不同的处理

4. **代理设置**：
   - 新模块会根据模型自动判断是否需要代理
   - 简化了代理配置逻辑

## 常见问题

### Q: 如何处理现有的配置结构？

A: 新模块提供了`updateConfig`方法，可以接受部分配置更新：

```typescript
// 旧方式
const config = { ...oldConfig, apiKey: newKey };

// 新方式
aiService.updateConfig({ apiKey: newKey });
```

### Q: 如何获取模型列表？

A: 新模块提供了更丰富的模型管理功能：

```typescript
// 获取所有模型
const models = aiService.getModels();

// 获取特定提供商的模型
import { modelManager } from '../services/ai';
const zhipuModels = modelManager.getModelsByProvider(AIProvider.ZHIPU);
```

### Q: 如何处理流式响应？

A: 新模块提供了流式响应支持：

```typescript
await aiService.analyzeRaidDataStream(
  records,
  (chunk) => {
    console.log('收到数据块:', chunk);
  }
);
```

## 测试迁移

在完成迁移后，运行以下测试确保功能正常：

```typescript
import { runAllTests } from '../services/ai/tests';

// 运行所有测试
runAllTests();
```

## 回滚计划

如果新模块出现问题，可以快速回滚：

1. 恢复旧的`aiService.ts`文件
2. 更新导入语句
3. 重新部署应用

新模块设计为与旧接口兼容，因此回滚应该是无痛的。