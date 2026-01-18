/**
 * AI配置管理器测试文件
 * 测试从ConfigManager.tsx整合的AI相关功能
 */

import { configManager } from './configManager';
import { modelManager } from './modelManager';

/**
 * 测试配置管理器的基本功能
 */
export function testConfigManagerBasics() {
  console.log('=== 测试配置管理器基本功能 ===');
  
  // 获取当前配置
  const config = configManager.getConfig();
  console.log('当前配置:', config);
  
  // 测试API密钥设置
  const apiKeyResult = configManager.setApiKey('test-api-key-12345');
  console.log('设置API密钥结果:', apiKeyResult);
  console.log('设置后的API密钥:', configManager.getApiKey());
  
  // 测试温度设置
  const tempResult = configManager.setTemperature(0.8);
  console.log('设置温度结果:', tempResult);
  console.log('设置后的温度:', configManager.getTemperature());
  
  // 测试最大令牌数设置
  const tokensResult = configManager.setMaxTokens(4096);
  console.log('设置最大令牌数结果:', tokensResult);
  console.log('设置后的最大令牌数:', configManager.getMaxTokens());
  
  // 测试代理URL设置
  const proxyUrlResult = configManager.setProxyUrl('http://127.0.0.1:7890');
  console.log('设置代理URL结果:', proxyUrlResult);
  console.log('设置后的代理URL:', configManager.getProxyUrl());
  
  // 测试代理启用状态
  configManager.setProxyEnabled(true);
  console.log('设置代理启用状态后:', configManager.isProxyEnabled());
  
  // 测试配置验证
  const validation = configManager.validateConfig();
  console.log('配置验证结果:', validation);
  
  console.log('基本功能测试完成\n');
}

/**
 * 测试模型切换功能
 */
export function testModelSwitching() {
  console.log('=== 测试模型切换功能 ===');
  
  // 获取所有模型
  const models = modelManager.getModels();
  console.log('可用模型数量:', models.length);
  
  if (models.length > 1) {
    const firstModel = models[0];
    const secondModel = models[1];
    
    // 切换到第一个模型
    const switchResult1 = configManager.switchModel(firstModel.id);
    console.log(`切换到${firstModel.name}结果:`, switchResult1);
    console.log('当前模型:', configManager.getCurrentModel()?.name);
    
    // 切换到第二个模型
    const switchResult2 = configManager.switchModel(secondModel.id);
    console.log(`切换到${secondModel.name}结果:`, switchResult2);
    console.log('当前模型:', configManager.getCurrentModel()?.name);
  } else {
    console.log('模型数量不足，跳过模型切换测试');
  }
  
  console.log('模型切换测试完成\n');
}

/**
 * 测试配置导入导出功能
 */
export function testConfigImportExport() {
  console.log('=== 测试配置导入导出功能 ===');
  
  // 导出当前配置
  const exportedConfig = configManager.exportConfig();
  console.log('导出的配置:', exportedConfig);
  
  // 重置配置
  configManager.resetToDefaults();
  console.log('重置后的配置:', configManager.getConfig());
  
  // 导入之前导出的配置
  const importResult = configManager.importConfig(exportedConfig);
  console.log('导入配置结果:', importResult);
  console.log('导入后的配置:', configManager.getConfig());
  
  console.log('配置导入导出测试完成\n');
}

/**
 * 测试事件系统
 */
export function testEventSystem() {
  console.log('=== 测试事件系统 ===');
  
  let eventTriggered = false;
  
  // 添加事件监听器
  const handleConfigUpdate = (data: any) => {
    console.log('收到配置更新事件:', data);
    eventTriggered = true;
  };
  
  configManager.addEventListener('config-updated', handleConfigUpdate);
  
  // 更新配置
  configManager.updateConfig({ temperature: 0.9 });
  
  // 等待事件处理
  setTimeout(() => {
    console.log('事件是否触发:', eventTriggered);
    
    // 移除事件监听器
    configManager.removeEventListener('config-updated', handleConfigUpdate);
    
    console.log('事件系统测试完成\n');
  }, 100);
}

/**
 * 测试配置验证功能
 */
export function testConfigValidation() {
  console.log('=== 测试配置验证功能 ===');
  
  // 测试有效配置
  configManager.setApiKey('valid-api-key');
  configManager.setTemperature(0.7);
  configManager.setProxyEnabled(false);
  
  let validation = configManager.validateConfig();
  console.log('有效配置验证结果:', validation);
  
  // 测试无效API密钥
  configManager.setApiKey('');
  validation = configManager.validateConfig();
  console.log('无效API密钥验证结果:', validation);
  
  // 恢复有效API密钥
  configManager.setApiKey('valid-api-key');
  
  // 测试无效温度
  configManager.setTemperature(1.5);
  validation = configManager.validateConfig();
  console.log('无效温度验证结果:', validation);
  
  // 恢复有效温度
  configManager.setTemperature(0.7);
  
  // 测试需要代理的模型
  const models = modelManager.getModels();
  const proxyModel = models.find(m => m.requiresProxy);
  
  if (proxyModel) {
    configManager.switchModel(proxyModel.id);
    configManager.setProxyEnabled(false);
    
    validation = configManager.validateConfig();
    console.log('需要代理但未启用代理的验证结果:', validation);
    
    // 启用代理
    configManager.setProxyEnabled(true);
    validation = configManager.validateConfig();
    console.log('启用代理后的验证结果:', validation);
  } else {
    console.log('没有需要代理的模型，跳过代理测试');
  }
  
  console.log('配置验证测试完成\n');
}

/**
 * 测试连接测试功能
 */
export async function testConnectionTest() {
  console.log('=== 测试连接测试功能 ===');
  
  // 测试无API密钥的情况
  configManager.setApiKey('');
  let testResult = await configManager.testConnection();
  console.log('无API密钥的测试结果:', testResult);
  
  // 设置有效的API密钥
  configManager.setApiKey('test-api-key');
  testResult = await configManager.testConnection();
  console.log('有API密钥的测试结果:', testResult);
  
  // 测试需要代理的模型
  const models = modelManager.getModels();
  const proxyModel = models.find(m => m.requiresProxy);
  
  if (proxyModel) {
    configManager.switchModel(proxyModel.id);
    configManager.setProxyEnabled(false);
    
    testResult = await configManager.testConnection();
    console.log('需要代理但未启用代理的测试结果:', testResult);
    
    // 启用代理
    configManager.setProxyEnabled(true);
    testResult = await configManager.testConnection();
    console.log('启用代理后的测试结果:', testResult);
  } else {
    console.log('没有需要代理的模型，跳过代理测试');
  }
  
  console.log('连接测试功能测试完成\n');
}

/**
 * 测试配置摘要功能
 */
export function testConfigSummary() {
  console.log('=== 测试配置摘要功能 ===');
  
  // 设置一些配置
  configManager.setApiKey('test-api-key');
  configManager.setTemperature(0.8);
  configManager.setProxyEnabled(true);
  configManager.setProxyUrl('http://127.0.0.1:7890');
  
  // 获取配置摘要
  const summary = configManager.getConfigSummary();
  console.log('配置摘要:', summary);
  
  console.log('配置摘要测试完成\n');
}

/**
 * 测试性能优化
 */
export function testPerformanceOptimization() {
  console.log('=== 测试性能优化 ===');
  
  const startTime = performance.now();
  
  // 执行多次配置操作
  for (let i = 0; i < 1000; i++) {
    configManager.setTemperature(Math.random());
    configManager.getTemperature();
    configManager.validateConfig();
  }
  
  const endTime = performance.now();
  const duration = endTime - startTime;
  
  console.log(`执行1000次配置操作耗时: ${duration.toFixed(2)}ms`);
  console.log(`平均每次操作耗时: ${(duration / 1000).toFixed(2)}ms`);
  
  console.log('性能优化测试完成\n');
}

/**
 * 运行所有测试
 */
export async function runAllConfigManagerTests() {
  try {
    console.log('开始AI配置管理器测试...\n');
    
    testConfigManagerBasics();
    testModelSwitching();
    testConfigImportExport();
    testEventSystem();
    testConfigValidation();
    await testConnectionTest();
    testConfigSummary();
    testPerformanceOptimization();
    
    console.log('所有AI配置管理器测试完成！');
  } catch (error) {
    console.error('测试过程中发生错误:', error);
  }
}

// 如果直接运行此文件，则执行测试
if (typeof window !== 'undefined') {
  // 浏览器环境
  (window as any).runConfigManagerTests = runAllConfigManagerTests;
  console.log('在浏览器控制台中运行 runConfigManagerTests() 来执行所有测试');
} else if (typeof global !== 'undefined') {
  // Node.js环境
  runAllConfigManagerTests();
}