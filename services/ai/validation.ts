/**
 * AI模块验证脚本
 * 用于验证整合后的AI模块功能完整性和正确性
 */

import { aiService } from './aiService';
import { configManager } from './configManager';
import { modelManager } from './modelManager';
import { connectionTester } from './connectionTester';
import { requestHandler } from './requestHandler';
import AIConfigAdapter from './configAdapter';

/**
 * 验证结果接口
 */
interface ValidationResult {
  module: string;
  success: boolean;
  issues: string[];
  details?: any;
}

/**
 * 验证模型管理器
 */
function validateModelManager(): ValidationResult {
  const issues: string[] = [];
  
  try {
    // 测试获取模型列表
    const models = modelManager.getModels();
    if (!models || models.length === 0) {
      issues.push('无法获取模型列表或列表为空');
    }
    
    // 测试获取当前模型
    const currentModel = modelManager.getCurrentModel();
    if (!currentModel) {
      issues.push('无法获取当前模型');
    }
    
    // 测试模型切换
    if (models.length > 1) {
      const firstModel = models[0];
      const switchResult = modelManager.setCurrentModel(firstModel.id);
      if (!switchResult) {
        issues.push('模型切换失败');
      }
      
      const switchedModel = modelManager.getCurrentModel();
      if (switchedModel?.id !== firstModel.id) {
        issues.push('模型切换后状态不一致');
      }
    }
    
    return {
      module: 'ModelManager',
      success: issues.length === 0,
      issues,
      details: {
        modelsCount: models.length,
        currentModelId: currentModel?.id,
        currentModelName: currentModel?.name
      }
    };
  } catch (error) {
    return {
      module: 'ModelManager',
      success: false,
      issues: [`验证过程中发生错误: ${error instanceof Error ? error.message : String(error)}`]
    };
  }
}

/**
 * 验证配置管理器
 */
function validateConfigManager(): ValidationResult {
  const issues: string[] = [];
  
  try {
    // 测试配置获取
    const config = configManager.getConfig();
    if (!config) {
      issues.push('无法获取配置');
    }
    
    // 测试API密钥设置
    const testApiKey = 'test-api-key-12345';
    const setKeyResult = configManager.setApiKey(testApiKey);
    if (!setKeyResult) {
      issues.push('API密钥设置失败');
    }
    
    const getKeyResult = configManager.getApiKey();
    if (getKeyResult !== testApiKey) {
      issues.push('API密钥获取不一致');
    }
    
    // 测试温度设置
    const testTemp = 0.8;
    const setTempResult = configManager.setTemperature(testTemp);
    if (!setTempResult) {
      issues.push('温度设置失败');
    }
    
    const getTempResult = configManager.getTemperature();
    if (Math.abs(getTempResult - testTemp) > 0.01) {
      issues.push('温度获取不一致');
    }
    
    // 测试代理设置
    const testProxyUrl = 'http://127.0.0.1:7890';
    const setProxyResult = configManager.setProxyUrl(testProxyUrl);
    if (!setProxyResult) {
      issues.push('代理URL设置失败');
    }
    
    const getProxyResult = configManager.getProxyUrl();
    if (getProxyResult !== testProxyUrl) {
      issues.push('代理URL获取不一致');
    }
    
    // 测试配置验证
    const validation = configManager.validateConfig();
    if (!validation.isValid) {
      issues.push(`配置验证失败: ${validation.error}`);
    }
    
    return {
      module: 'ConfigManager',
      success: issues.length === 0,
      issues,
      details: {
        config,
        validation
      }
    };
  } catch (error) {
    return {
      module: 'ConfigManager',
      success: false,
      issues: [`验证过程中发生错误: ${error instanceof Error ? error.message : String(error)}`]
    };
  }
}

/**
 * 验证连通性测试器
 */
async function validateConnectionTester(): Promise<ValidationResult> {
  const issues: string[] = [];
  
  try {
    // 测试网络连接
    const networkConnected = await connectionTester.testNetworkConnectivity();
    if (!networkConnected) {
      issues.push('网络连接测试失败');
    }
    
    // 测试模型连通性（模拟）
    const models = modelManager.getModels();
    if (models.length > 0) {
      const testModel = models[0];
      const testResult = await connectionTester.testModel(testModel, {
        apiKey: 'test-key',
        timeout: 5000
      });
      
      if (!testResult.success) {
        issues.push(`模型连通性测试失败: ${testResult.message}`);
      }
    }
    
    return {
      module: 'ConnectionTester',
      success: issues.length === 0,
      issues,
      details: {
        networkConnected,
        modelsCount: models.length
      }
    };
  } catch (error) {
    return {
      module: 'ConnectionTester',
      success: false,
      issues: [`验证过程中发生错误: ${error instanceof Error ? error.message : String(error)}`]
    };
  }
}

/**
 * 验证请求处理器
 */
function validateRequestHandler(): ValidationResult {
  const issues: string[] = [];
  
  try {
    // 测试prompt格式化
    const template = '你好，{{name}}！';
    const variables = { name: '世界' };
    const formattedPrompt = requestHandler.formatPrompt(template, variables);
    
    if (formattedPrompt !== '你好，世界！') {
      issues.push('prompt格式化失败');
    }
    
    // 测试副本分析prompt构建
    const testData = {
      totalRuns: 10,
      totalGold: 200000,
      xuanjingCount: 2,
      clearedCount: 8,
      clearRate: '80.0',
      xuanjingRate: '20.0',
      avgGoldPerRun: 20000,
      topRaids: [
        { name: '25人英雄冰心诀', gold: 100000 },
        { name: '25人英雄元鲸客栈', gold: 100000 }
      ],
      xuanjingRaids: ['25人英雄冰心诀']
    };
    
    const analysisPrompt = requestHandler.buildRaidAnalysisPrompt(testData);
    if (!analysisPrompt || analysisPrompt.length === 0) {
      issues.push('副本分析prompt构建失败');
    }
    
    return {
      module: 'RequestHandler',
      success: issues.length === 0,
      issues,
      details: {
        formattedPrompt,
        analysisPromptLength: analysisPrompt?.length || 0
      }
    };
  } catch (error) {
    return {
      module: 'RequestHandler',
      success: false,
      issues: [`验证过程中发生错误: ${error instanceof Error ? error.message : String(error)}`]
    };
  }
}

/**
 * 验证AI服务统一接口
 */
function validateAIService(): ValidationResult {
  const issues: string[] = [];
  
  try {
    // 测试服务初始化
    aiService.initialize();
    
    // 测试模型获取
    const models = aiService.getModels();
    if (!models || models.length === 0) {
      issues.push('无法通过AI服务获取模型列表');
    }
    
    // 测试配置获取
    const config = aiService.getConfig();
    if (!config) {
      issues.push('无法通过AI服务获取配置');
    }
    
    // 测试配置验证
    const validation = aiService.validateConfig();
    if (!validation.isValid) {
      issues.push(`AI服务配置验证失败: ${validation.error}`);
    }
    
    return {
      module: 'AIService',
      success: issues.length === 0,
      issues,
      details: {
        modelsCount: models.length,
        config,
        validation
      }
    };
  } catch (error) {
    return {
      module: 'AIService',
      success: false,
      issues: [`验证过程中发生错误: ${error instanceof Error ? error.message : String(error)}`]
    };
  }
}

/**
 * 验证配置适配器
 */
function validateConfigAdapter(): ValidationResult {
  const issues: string[] = [];
  
  try {
    // 测试旧式配置转换
    const legacyConfig = {
      enabled: true,
      apiKey: 'test-api-key',
      model: 'glm-4.6',
      temperature: 0.7,
      proxyUrl: 'http://127.0.0.1:7890',
      proxyEnabled: true
    };
    
    AIConfigAdapter.adaptLegacyConfig(legacyConfig);
    
    // 验证转换结果
    const convertedConfig = AIConfigAdapter.convertToLegacyConfig();
    
    if (convertedConfig.apiKey !== legacyConfig.apiKey) {
      issues.push('API密钥转换不一致');
    }
    
    if (Math.abs(convertedConfig.temperature - legacyConfig.temperature) > 0.01) {
      issues.push('温度转换不一致');
    }
    
    // 测试迁移验证
    const migrationValidation = AIConfigAdapter.validateMigration();
    if (!migrationValidation.success) {
      issues.push('迁移验证失败');
      issues.push(...migrationValidation.issues);
    }
    
    return {
      module: 'ConfigAdapter',
      success: issues.length === 0,
      issues,
      details: {
        legacyConfig,
        convertedConfig,
        migrationValidation
      }
    };
  } catch (error) {
    return {
      module: 'ConfigAdapter',
      success: false,
      issues: [`验证过程中发生错误: ${error instanceof Error ? error.message : String(error)}`]
    };
  }
}

/**
 * 运行所有验证
 */
export async function runAllValidations(): Promise<ValidationResult[]> {
  console.log('开始AI模块验证...\n');
  
  const results: ValidationResult[] = [];
  
  // 验证各个模块
  results.push(validateModelManager());
  results.push(validateConfigManager());
  results.push(await validateConnectionTester());
  results.push(validateRequestHandler());
  results.push(validateAIService());
  results.push(validateConfigAdapter());
  
  // 统计结果
  const totalIssues = results.reduce((sum, result) => sum + result.issues.length, 0);
  const successCount = results.filter(result => result.success).length;
  
  console.log('\n=== 验证结果汇总 ===');
  console.log(`总模块数: ${results.length}`);
  console.log(`成功模块数: ${successCount}`);
  console.log(`失败模块数: ${results.length - successCount}`);
  console.log(`总问题数: ${totalIssues}`);
  
  // 详细结果
  results.forEach(result => {
    console.log(`\n--- ${result.module} ---`);
    console.log(`状态: ${result.success ? '✅ 成功' : '❌ 失败'}`);
    if (result.issues.length > 0) {
      console.log('问题列表:');
      result.issues.forEach(issue => console.log(`  - ${issue}`));
    }
    if (result.details) {
      console.log('详细信息:', result.details);
    }
  });
  
  console.log('\n=== 验证完成 ===');
  
  return results;
}

// 如果直接运行此文件，则执行验证
if (typeof window !== 'undefined') {
  // 浏览器环境
  (window as any).runAIValidations = runAllValidations;
  console.log('在浏览器控制台中运行 runAIValidations() 来执行所有验证');
} else if (typeof global !== 'undefined') {
  // Node.js环境
  runAllValidations();
}