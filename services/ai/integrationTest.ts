/**
 * AI模块集成测试脚本
 * 测试整合后的AI模块是否正常工作
 */

import { aiService } from './index';

/**
 * 测试AI模块基本功能
 */
async function testAIModule() {
  console.log('=== 测试AI模块基本功能 ===');
  
  try {
    // 初始化服务
    aiService.initialize();
    console.log('✅ AI服务初始化成功');
    
    // 获取模型列表
    const models = aiService.getModels();
    console.log(`✅ 获取到 ${models.length} 个模型`);
    
    // 获取当前模型
    const currentModel = aiService.getCurrentModel();
    console.log(`✅ 当前模型: ${currentModel?.name || '未设置'}`);
    
    // 获取配置
    const config = aiService.getConfig();
    console.log(`✅ 获取配置成功，API密钥状态: ${config.apiKey ? '已设置' : '未设置'}`);
    
    // 设置测试API密钥
    aiService.setApiKey('test-api-key');
    console.log('✅ 设置测试API密钥成功');
    
    // 设置测试温度
    aiService.setTemperature(0.8);
    console.log('✅ 设置测试温度成功');
    
    // 设置测试代理
    aiService.setProxyUrl('http://127.0.0.1:7890');
    aiService.setProxyEnabled(true);
    console.log('✅ 设置测试代理成功');
    
    // 验证配置
    const validation = aiService.validateConfig();
    if (validation.isValid) {
      console.log('✅ 配置验证通过');
    } else {
      console.log(`❌ 配置验证失败: ${validation.error}`);
    }
    
    // 测试连接（模拟）
    console.log('开始测试连接...');
    const testResult = await aiService.testConnection();
    if (testResult.success) {
      console.log(`✅ 连接测试成功: ${testResult.message}`);
    } else {
      console.log(`❌ 连接测试失败: ${testResult.message}`);
    }
    
    console.log('\n=== AI模块测试完成 ===');
    return true;
  } catch (error) {
    console.error('❌ AI模块测试失败:', error);
    return false;
  }
}

/**
 * 测试ConfigManager组件集成
 */
function testConfigManagerIntegration() {
  console.log('\n=== 测试ConfigManager组件集成 ===');
  
  try {
    // 由于ConfigManager是React组件，我们只检查文件是否存在
    const fs = require('fs');
    const path = require('path');
    const configManagerPath = path.join(__dirname, '..', '..', 'components', 'ConfigManager.tsx');
    
    if (fs.existsSync(configManagerPath)) {
      console.log('✅ ConfigManager组件文件存在');
    } else {
      console.log('❌ ConfigManager组件文件不存在');
      return false;
    }
    
    // 检查组件是否使用了新的AI模块
    const componentSource = fs.readFileSync(configManagerPath, 'utf8');
    if (componentSource.includes('aiService')) {
      console.log('✅ ConfigManager组件已集成新的AI模块');
    } else {
      console.log('❌ ConfigManager组件未集成新的AI模块');
      return false;
    }
    
    console.log('✅ ConfigManager组件集成测试通过');
    return true;
  } catch (error) {
    console.error('❌ ConfigManager组件集成测试失败:', error);
    return false;
  }
}

/**
 * 运行所有测试
 */
export async function runAllIntegrationTests() {
  console.log('开始AI模块集成测试...\n');
  
  const results = [];
  
  // 测试AI模块
  results.push(await testAIModule());
  
  // 测试ConfigManager组件集成
  results.push(testConfigManagerIntegration());
  
  // 统计结果
  const successCount = results.filter(result => result).length;
  const totalCount = results.length;
  
  console.log('\n=== 测试结果汇总 ===');
  console.log(`总测试数: ${totalCount}`);
  console.log(`成功测试数: ${successCount}`);
  console.log(`失败测试数: ${totalCount - successCount}`);
  console.log(`成功率: ${((successCount / totalCount) * 100).toFixed(1)}%`);
  
  if (successCount === totalCount) {
    console.log('\n🎉 所有测试通过！AI模块整合成功！');
  } else {
    console.log('\n⚠️ 部分测试失败，请检查相关模块');
  }
  
  return successCount === totalCount;
}

// 如果直接运行此文件，则执行测试
if (typeof window !== 'undefined') {
  // 浏览器环境
  (window as any).runAIIntegrationTests = runAllIntegrationTests;
  console.log('在浏览器控制台中运行 runAIIntegrationTests() 来执行所有测试');
} else if (typeof global !== 'undefined') {
  // Node.js环境
  runAllIntegrationTests();
}