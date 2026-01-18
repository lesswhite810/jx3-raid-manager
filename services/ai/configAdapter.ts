/**
 * AI配置适配器
 * 提供从旧ConfigManager.tsx到新AI模块的平滑迁移
 */

import { aiService } from './aiService';
import { configManager } from './configManager';
import { modelManager } from './modelManager';

/**
 * 旧式AI配置接口
 * 兼容原有的ConfigManager.tsx中的AI配置结构
 */
export interface LegacyAIConfig {
  enabled: boolean;
  apiKey: string;
  model: string;
  temperature: number;
  proxyUrl: string;
  proxyEnabled: boolean;
}

/**
 * AI配置适配器类
 * 提供旧接口到新模块的适配
 */
export class AIConfigAdapter {
  /**
   * 将旧式配置转换为新模块配置
   */
  public static adaptLegacyConfig(legacyConfig: LegacyAIConfig): void {
    // 设置API密钥
    if (legacyConfig.apiKey) {
      configManager.setApiKey(legacyConfig.apiKey);
    }
    
    // 切换模型
    if (legacyConfig.model) {
      configManager.switchModel(legacyConfig.model);
    }
    
    // 设置温度
    if (typeof legacyConfig.temperature === 'number') {
      configManager.setTemperature(legacyConfig.temperature);
    }
    
    // 设置代理
    if (typeof legacyConfig.proxyEnabled === 'boolean') {
      configManager.setProxyEnabled(legacyConfig.proxyEnabled);
    }
    
    if (legacyConfig.proxyUrl) {
      configManager.setProxyUrl(legacyConfig.proxyUrl);
    }
  }

  /**
   * 从新模块配置转换为旧式配置
   */
  public static convertToLegacyConfig(): LegacyAIConfig {
    const config = configManager.getConfig();
    
    return {
      enabled: !!config.apiKey,
      apiKey: config.apiKey,
      model: config.model,
      temperature: config.temperature,
      proxyUrl: config.proxyUrl || '',
      proxyEnabled: config.proxyEnabled || false,
    };
  }

  /**
   * 适配旧的analyzeRaidWithAI函数
   */
  public static async analyzeRaidWithAI(
    records: any[],
    _accounts: any[],
    config: LegacyAIConfig
  ): Promise<{ success: boolean; content?: string; error?: string }> {
    // 转换配置
    this.adaptLegacyConfig(config);
    
    // 调用新方法
    const result = await aiService.analyzeRaidData(records);
    
    // 转换返回格式
    return {
      success: result.success,
      content: result.content,
      error: result.error,
    };
  }

  /**
   * 适配旧的testAIConnection函数
   */
  public static async testAIConnection(
    config: LegacyAIConfig
  ): Promise<{ success: boolean; message: string }> {
    // 转换配置
    this.adaptLegacyConfig(config);
    
    // 调用新方法
    return configManager.testConnection();
  }

  /**
   * 适配旧的validateConfig函数
   */
  public static validateConfig(config: LegacyAIConfig): { isValid: boolean; error?: string } {
    // 转换配置
    this.adaptLegacyConfig(config);
    
    // 调用新方法
    return configManager.validateConfig();
  }

  /**
   * 适配旧的saveConfigToStorage函数
   */
  public static saveConfigToStorage(config: LegacyAIConfig): void {
    // 转换配置
    this.adaptLegacyConfig(config);
    
    // 新模块会自动保存，无需额外操作
  }

  /**
   * 适配旧的loadConfigFromStorage函数
   */
  public static loadConfigFromStorage(): LegacyAIConfig {
    // 从新模块获取配置
    return this.convertToLegacyConfig();
  }

  /**
   * 适配旧的getModelProvider函数
   */
  public static getModelProvider(modelId: string): string {
    const model = modelManager.getModelById(modelId);
    return model ? model.provider : '未知';
  }

  /**
   * 适配旧的modelRequiresProxy函数
   */
  public static modelRequiresProxy(modelId: string): boolean {
    const model = modelManager.getModelById(modelId);
    return model ? model.requiresProxy : false;
  }

  /**
   * 适配旧的getProxyWarning函数
   */
  public static getProxyWarning(modelId: string): string {
    const model = modelManager.getModelById(modelId);
    if (!model) {
      return '未知的模型';
    }
    
    if (model.requiresProxy) {
      return `${model.name}需要代理才能访问，请启用代理并配置有效的代理服务器`;
    }
    
    return '';
  }

  /**
   * 创建配置变更监听器
   * 兼容旧的事件系统
   */
  public static createConfigChangeListener(
    callback: (config: LegacyAIConfig) => void
  ): () => void {
    const handleConfigChange = () => {
      const legacyConfig = this.convertToLegacyConfig();
      callback(legacyConfig);
    };
    
    // 监听配置变化
    configManager.addEventListener('config-updated', handleConfigChange);
    configManager.addEventListener('model-changed', handleConfigChange);
    configManager.addEventListener('api-key-updated', handleConfigChange);
    configManager.addEventListener('temperature-updated', handleConfigChange);
    configManager.addEventListener('proxy-enabled-updated', handleConfigChange);
    configManager.addEventListener('proxy-url-updated', handleConfigChange);
    
    // 返回清理函数
    return () => {
      configManager.removeEventListener('config-updated', handleConfigChange);
      configManager.removeEventListener('model-changed', handleConfigChange);
      configManager.removeEventListener('api-key-updated', handleConfigChange);
      configManager.removeEventListener('temperature-updated', handleConfigChange);
      configManager.removeEventListener('proxy-enabled-updated', handleConfigChange);
      configManager.removeEventListener('proxy-url-updated', handleConfigChange);
    };
  }

  /**
   * 创建模型变更监听器
   */
  public static createModelChangeListener(
    callback: (modelId: string, model: any) => void
  ): () => void {
    const handleModelChange = (data: any) => {
      if (data && data.newModelId) {
        const model = modelManager.getModelById(data.newModelId);
        if (model) {
          callback(data.newModelId, model);
        }
      }
    };
    
    configManager.addEventListener('model-changed', handleModelChange);
    
    return () => {
      configManager.removeEventListener('model-changed', handleModelChange);
    };
  }

  /**
   * 批量迁移配置
   */
  public static migrateConfig(legacyConfig: LegacyAIConfig): void {
    console.log('开始迁移AI配置...');
    
    try {
      // 备份当前配置
      const backup = this.convertToLegacyConfig();
      localStorage.setItem('ai_config_backup', JSON.stringify(backup));
      
      // 应用新配置
      this.adaptLegacyConfig(legacyConfig);
      
      console.log('AI配置迁移成功');
    } catch (error) {
      console.error('AI配置迁移失败:', error);
      
      // 恢复备份
      try {
        const backup = localStorage.getItem('ai_config_backup');
        if (backup) {
          const backupConfig = JSON.parse(backup);
          this.adaptLegacyConfig(backupConfig);
        }
      } catch (restoreError) {
        console.error('恢复配置备份失败:', restoreError);
      }
    }
  }

  /**
   * 恢复迁移前的配置
   */
  public static restoreConfig(): boolean {
    try {
      const backup = localStorage.getItem('ai_config_backup');
      if (!backup) {
        console.warn('未找到配置备份');
        return false;
      }
      
      const backupConfig = JSON.parse(backup);
      this.adaptLegacyConfig(backupConfig);
      
      console.log('配置恢复成功');
      return true;
    } catch (error) {
      console.error('配置恢复失败:', error);
      return false;
    }
  }

  /**
   * 清理迁移备份
   */
  public static cleanupBackup(): void {
    try {
      localStorage.removeItem('ai_config_backup');
      console.log('配置备份已清理');
    } catch (error) {
      console.error('清理配置备份失败:', error);
    }
  }

  /**
   * 验证迁移结果
   */
  public static validateMigration(): { success: boolean; issues: string[] } {
    const issues: string[] = [];
    
    try {
      // 检查新模块是否正常工作
      const config = configManager.getConfig();
      const validation = configManager.validateConfig();
      
      if (!validation.isValid) {
        issues.push(`配置验证失败: ${validation.error}`);
      }
      
      // 检查模型是否正确加载
      const model = modelManager.getCurrentModel();
      if (!model) {
        issues.push('模型加载失败');
      }
      
      // 检查API密钥是否正确设置
      if (!config.apiKey.trim()) {
        issues.push('API密钥未设置');
      }
      
      console.log('迁移验证结果:', { success: issues.length === 0, issues });
      return { success: issues.length === 0, issues };
    } catch (error) {
      const errorMessage = `验证过程中发生错误: ${error instanceof Error ? error.message : String(error)}`;
      issues.push(errorMessage);
      console.error(errorMessage);
      return { success: false, issues };
    }
  }
}

// 导出适配器
export default AIConfigAdapter;