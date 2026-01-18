/**
 * AI模型配置管理模块
 * 负责AI模型的配置、API密钥管理和代理设置
 * 整合了原有的ConfigManager.tsx中的AI相关功能
 */

import { AIModel, AIRequestConfig, AIServiceConfig } from './types';
import { modelManager } from './modelManager';

// 配置存储键名
const CONFIG_KEY = 'jx3_config'; // 与原有配置保持一致

// 从utils/configUtils.ts导入的验证函数
const isValidApiKey = (apiKey: string): boolean => {
  if (!apiKey || typeof apiKey !== 'string') {
    return false;
  }
  
  return apiKey.trim().length > 0;
};

const isValidTemperature = (temperature: number): boolean => {
  return typeof temperature === 'number' && temperature >= 0 && temperature <= 1;
};

const isValidProxyUrl = (url: string): boolean => {
  if (!url || typeof url !== 'string') {
    return false;
  }
  
  try {
    new URL(url);
    return true;
  } catch {
    return false;
  }
};

/**
 * AI模型配置管理器类
 * 整合了原有ConfigManager.tsx中的AI相关功能
 */
export class AIConfigManager {
  private static instance: AIConfigManager;
  private config: AIServiceConfig;
  private listeners: Map<string, Set<Function>> = new Map(); // 事件监听器

  private constructor() {
    this.config = this.loadConfigFromStorage();
    this.initializeEventListeners();
  }

  /**
   * 获取配置管理器单例
   */
  public static getInstance(): AIConfigManager {
    if (!AIConfigManager.instance) {
      AIConfigManager.instance = new AIConfigManager();
    }
    return AIConfigManager.instance;
  }

  /**
   * 初始化事件监听器
   */
  private initializeEventListeners(): void {
    // 监听模型变化事件
    window.addEventListener('ai-model-changed', this.handleModelChange.bind(this));
    window.addEventListener('ai-config-updated', this.handleConfigUpdate.bind(this));
  }

  /**
   * 处理模型变化事件
   */
  private handleModelChange(event: Event): void {
    const customEvent = event as CustomEvent;
    const { modelId } = customEvent.detail;
    if (modelId && modelId !== this.config.model) {
      this.setModel(modelId);
    }
  }

  /**
   * 处理配置更新事件
   */
  private handleConfigUpdate(event: Event): void {
    const customEvent = event as CustomEvent;
    const { updates } = customEvent.detail;
    if (updates) {
      this.updateConfig(updates);
    }
  }

  /**
   * 添加事件监听器
   */
  public addEventListener(event: string, callback: Function): void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(callback);
  }

  /**
   * 移除事件监听器
   */
  public removeEventListener(event: string, callback: Function): void {
    const eventListeners = this.listeners.get(event);
    if (eventListeners) {
      eventListeners.delete(callback);
    }
  }

  /**
   * 触发事件
   */
  private emitEvent(event: string, data?: any): void {
    const eventListeners = this.listeners.get(event);
    if (eventListeners) {
      eventListeners.forEach(callback => {
        try {
          callback(data);
        } catch (error) {
          console.error(`事件处理器错误 (${event}):`, error);
        }
      });
    }
  }

  /**
   * 从本地存储加载配置
   * 兼容原有的配置格式
   */
  private loadConfigFromStorage(): AIServiceConfig {
    try {
      const savedConfig = localStorage.getItem(CONFIG_KEY);
      if (savedConfig) {
        const parsedConfig = JSON.parse(savedConfig);
        
        // 兼容原有配置格式
        if (parsedConfig.ai) {
          return {
            enabled: parsedConfig.ai.enabled || false,
            apiKey: parsedConfig.ai.apiKey || '',
            model: parsedConfig.ai.model || 'glm-4.6',
            temperature: parsedConfig.ai.temperature || 0.7,
            proxyUrl: parsedConfig.ai.proxyUrl || '',
            proxyEnabled: parsedConfig.ai.proxyEnabled || false,
            maxTokens: parsedConfig.ai.maxTokens || 2048,
            timeout: parsedConfig.ai.timeout || 30000,
          };
        }
      }
    } catch (error) {
      console.error('加载AI配置失败:', error);
    }
    
    // 返回默认配置
    return {
      enabled: false,
      apiKey: '',
      model: modelManager.getCurrentModel()?.id || 'glm-4.6',
      temperature: 0.7,
      proxyUrl: '',
      proxyEnabled: false,
      maxTokens: 2048,
      timeout: 30000,
    };
  }

  /**
   * 保存配置到本地存储
   * 兼容原有的配置格式
   */
  private saveConfigToStorage(): void {
    try {
      // 获取完整配置
      const fullConfig = this.getFullConfig();
      localStorage.setItem(CONFIG_KEY, JSON.stringify(fullConfig));
      this.emitEvent('config-saved', this.config);
    } catch (error) {
      console.error('保存AI配置失败:', error);
      this.emitEvent('config-save-error', error);
    }
  }

  /**
   * 获取完整配置（兼容原有格式）
   */
  private getFullConfig(): any {
    // 获取现有配置
    let existingConfig: any = {};
    try {
      const saved = localStorage.getItem(CONFIG_KEY);
      if (saved) {
        existingConfig = JSON.parse(saved);
      }
    } catch (error) {
      console.error('读取现有配置失败:', error);
    }
    
    // 合并AI配置
    return {
      ...existingConfig,
      ai: {
        ...existingConfig.ai,
        enabled: this.config.enabled,
        apiKey: this.config.apiKey,
        model: this.config.model,
        temperature: this.config.temperature,
        proxyUrl: this.config.proxyUrl,
        proxyEnabled: this.config.proxyEnabled,
        maxTokens: this.config.maxTokens,
        timeout: this.config.timeout,
      }
    };
  }

  /**
   * 获取当前配置
   */
  public getConfig(): AIServiceConfig {
    return { ...this.config };
  }

  /**
   * 获取请求配置
   */
  public getRequestConfig(): AIRequestConfig {
    const currentModel = modelManager.getCurrentModel();
    if (!currentModel) {
      throw new Error('未选择AI模型');
    }
    
    return {
      apiKey: this.config.apiKey,
      model: this.config.model,
      temperature: this.config.temperature,
      maxTokens: this.config.maxTokens,
      proxyUrl: this.config.proxyUrl,
      proxyEnabled: this.config.proxyEnabled, // 直接使用用户设置的代理状态
      timeout: this.config.timeout,
    };
  }

  /**
   * 更新配置
   */
  public updateConfig(updates: Partial<AIServiceConfig>): void {
    const oldConfig = { ...this.config };
    this.config = { ...this.config, ...updates };
    
    // 验证更新后的配置
    const validation = this.validateConfig();
    if (!validation.isValid) {
      console.error('配置更新失败:', validation.error);
      this.config = oldConfig; // 回滚
      this.emitEvent('config-validation-error', validation.error);
      return;
    }
    
    this.saveConfigToStorage();
    this.emitEvent('config-updated', updates);
  }

  /**
   * 设置API密钥
   * 兼容原有的验证逻辑
   */
  public setApiKey(apiKey: string): boolean {
    if (!isValidApiKey(apiKey)) {
      this.emitEvent('api-key-error', 'API密钥格式无效');
      return false;
    }
    
    this.config.apiKey = apiKey.trim();
    this.saveConfigToStorage();
    this.emitEvent('api-key-updated', this.config.apiKey);
    return true;
  }

  /**
   * 获取API密钥
   */
  public getApiKey(): string {
    return this.config.apiKey;
  }

  /**
   * 验证API密钥
   */
  public validateApiKey(apiKey?: string): boolean {
    const keyToValidate = apiKey !== undefined ? apiKey : this.config.apiKey;
    return isValidApiKey(keyToValidate);
  }

  /**
   * 设置模型ID
   * 兼容原有的模型切换逻辑
   */
  public setModel(modelId: string): boolean {
    const model = modelManager.getModelById(modelId);
    if (!model) {
      console.error(`未找到ID为 ${modelId} 的模型`);
      this.emitEvent('model-error', `未找到ID为 ${modelId} 的模型`);
      return false;
    }
    
    const oldModelId = this.config.model;
    this.config.model = modelId;
    
    // 如果新模型需要代理但未配置，则发出警告
    if (model.requiresProxy && !this.config.proxyEnabled) {
      this.emitEvent('proxy-required', {
        modelName: model.name,
        message: `${model.name}需要代理才能访问，请启用代理并配置有效的代理服务器`
      });
    }
    
    modelManager.setCurrentModel(modelId);
    this.saveConfigToStorage();
    this.emitEvent('model-changed', { oldModelId, newModelId: modelId, model });
    return true;
  }

  /**
   * 获取当前模型ID
   */
  public getModel(): string {
    return this.config.model;
  }

  /**
   * 获取当前模型对象
   */
  public getCurrentModel(): AIModel | null {
    return modelManager.getCurrentModel();
  }

  /**
   * 设置温度参数
   * 兼容原有的验证逻辑
   */
  public setTemperature(temperature: number): boolean {
    if (!isValidTemperature(temperature)) {
      this.emitEvent('temperature-error', '温度参数必须在0-1之间');
      return false;
    }
    
    const oldTemperature = this.config.temperature;
    this.config.temperature = temperature;
    this.saveConfigToStorage();
    this.emitEvent('temperature-updated', { oldTemperature, newTemperature: temperature });
    return true;
  }

  /**
   * 获取温度参数
   */
  public getTemperature(): number {
    return this.config.temperature;
  }

  /**
   * 设置最大令牌数
   */
  public setMaxTokens(maxTokens: number): boolean {
    // 限制最大令牌数范围在1-32768之间
    const clampedMaxTokens = Math.max(1, Math.min(32768, maxTokens));
    const oldMaxTokens = this.config.maxTokens;
    
    this.config.maxTokens = clampedMaxTokens;
    this.saveConfigToStorage();
    this.emitEvent('max-tokens-updated', { oldMaxTokens, newMaxTokens: clampedMaxTokens });
    return true;
  }

  /**
   * 获取最大令牌数
   */
  public getMaxTokens(): number {
    return this.config.maxTokens || 2048;
  }

  /**
   * 设置代理URL
   * 兼容原有的验证逻辑
   */
  public setProxyUrl(proxyUrl: string): boolean {
    if (proxyUrl && !isValidProxyUrl(proxyUrl)) {
      this.emitEvent('proxy-url-error', '代理URL格式无效');
      return false;
    }
    
    const oldProxyUrl = this.config.proxyUrl;
    this.config.proxyUrl = proxyUrl ? proxyUrl.trim() : '';
    this.saveConfigToStorage();
    this.emitEvent('proxy-url-updated', { oldProxyUrl, newProxyUrl: this.config.proxyUrl });
    return true;
  }

  /**
   * 获取代理URL
   */
  public getProxyUrl(): string {
    return this.config.proxyUrl || '';
  }

  /**
   * 设置代理启用状态
   */
  public setProxyEnabled(enabled: boolean): void {
    const oldProxyEnabled = this.config.proxyEnabled;
    this.config.proxyEnabled = enabled;
    this.saveConfigToStorage();
    this.emitEvent('proxy-enabled-updated', { oldProxyEnabled, newProxyEnabled: enabled });
    
    // 检查当前模型是否需要代理
    const currentModel = modelManager.getCurrentModel();
    if (currentModel && currentModel.requiresProxy && !enabled) {
      this.emitEvent('proxy-warning', {
        modelName: currentModel.name,
        message: `${currentModel.name}需要代理才能正常工作`
      });
    }
  }

  /**
   * 获取代理启用状态
   */
  public isProxyEnabled(): boolean {
    return this.config.proxyEnabled || false;
  }

  /**
   * 设置请求超时时间
   */
  public setTimeout(timeout: number): boolean {
    // 限制超时时间范围在5秒-5分钟之间
    const clampedTimeout = Math.max(5000, Math.min(300000, timeout));
    const oldTimeout = this.config.timeout;
    
    this.config.timeout = clampedTimeout;
    this.saveConfigToStorage();
    this.emitEvent('timeout-updated', { oldTimeout, newTimeout: clampedTimeout });
    return true;
  }

  /**
   * 获取请求超时时间
   */
  public getTimeout(): number {
    return this.config.timeout || 30000;
  }

  /**
   * 切换模型
   * 兼容原有的模型切换逻辑
   */
  public switchModel(modelId: string): boolean {
    if (this.setModel(modelId)) {
      this.emitEvent('model-switched', { modelId });
      return true;
    }
    return false;
  }

  /**
   * 获取模型代理需求状态
   */
  public doesModelRequireProxy(): boolean {
    const model = modelManager.getCurrentModel();
    return model ? model.requiresProxy : false;
  }

  /**
   * 获取有效的代理URL
   */
  public getEffectiveProxyUrl(): string | null {
    const model = modelManager.getCurrentModel();
    if (!model || !model.requiresProxy || !this.config.proxyEnabled) {
      return null;
    }
    
    return this.config.proxyUrl || null;
  }

  /**
   * 验证配置是否完整
   * 兼容原有的验证逻辑
   */
  public validateConfig(): { isValid: boolean; error?: string } {
    const model = modelManager.getCurrentModel();
    if (!model) {
      return { isValid: false, error: '未选择AI模型' };
    }
    
    if (!this.config.apiKey.trim()) {
      return { isValid: false, error: 'API密钥未配置' };
    }
    
    if (model.requiresProxy && (!this.config.proxyEnabled || !this.config.proxyUrl?.trim())) {
      return { 
        isValid: false, 
        error: `模型 ${model.name} 需要代理，但未配置代理或未启用代理` 
      };
    }
    
    if (!isValidTemperature(this.config.temperature)) {
      return { isValid: false, error: '温度参数必须在0-1之间' };
    }
    
    if (this.config.maxTokens && (this.config.maxTokens < 1 || this.config.maxTokens > 32768)) {
      return { isValid: false, error: '最大令牌数必须在1-32768之间' };
    }
    
    return { isValid: true };
  }

  /**
   * 重置配置为默认值
   */
  public resetToDefaults(): void {
    const oldConfig = { ...this.config };
    
    this.config = {
      enabled: false,
      apiKey: '',
      model: modelManager.getCurrentModel()?.id || 'glm-4.6',
      temperature: 0.7,
      proxyUrl: '',
      proxyEnabled: false,
      maxTokens: 2048,
      timeout: 30000,
    };
    
    this.saveConfigToStorage();
    this.emitEvent('config-reset', { oldConfig, newConfig: this.config });
  }

  /**
   * 导出配置
   */
  public exportConfig(): string {
    return JSON.stringify(this.config, null, 2);
  }

  /**
   * 导入配置
   */
  public importConfig(configJson: string): boolean {
    try {
      const importedConfig = JSON.parse(configJson);
      
      // 验证导入的配置
      const tempConfig = { ...this.config, ...importedConfig };
      const validation = this.validateConfigWithModel(tempConfig.model);
      
      if (!validation.isValid) {
        console.error('导入的配置无效:', validation.error);
        this.emitEvent('config-import-error', validation.error);
        return false;
      }
      
      const oldConfig = { ...this.config };
      this.config = tempConfig;
      this.saveConfigToStorage();
      this.emitEvent('config-imported', { oldConfig, newConfig: this.config });
      return true;
    } catch (error) {
      console.error('导入配置失败:', error);
      this.emitEvent('config-import-error', error);
      return false;
    }
  }

  /**
   * 使用指定模型验证配置
   */
  private validateConfigWithModel(modelId: string): { isValid: boolean; error?: string } {
    const model = modelManager.getModelById(modelId);
    if (!model) {
      return { isValid: false, error: '指定的模型不存在' };
    }
    
    if (!this.config.apiKey.trim()) {
      return { isValid: false, error: 'API密钥未配置' };
    }
    
    if (model.requiresProxy && (!this.config.proxyEnabled || !this.config.proxyUrl?.trim())) {
      return { isValid: false, error: `模型 ${model.name} 需要代理，但未配置代理或未启用代理` };
    }
    
    return { isValid: true };
  }

  /**
   * 获取配置摘要
   * 兼容原有的配置摘要功能
   */
  public getConfigSummary(): string {
    const model = modelManager.getCurrentModel();
    const aiStatus = this.config.apiKey ? '已配置' : '未配置';
    const modelInfo = model ? `${model.name} (${this.config.temperature.toFixed(1)})` : '未设置';
    const proxyStatus = this.config.proxyEnabled ? `已启用 (${this.config.proxyUrl})` : '未启用';
    
    return `AI: ${aiStatus}, 模型: ${modelInfo}, 代理: ${proxyStatus}`;
  }

  /**
   * 测试AI连接
   * 兼容原有的测试逻辑
   */
  public async testConnection(): Promise<{ success: boolean; message: string }> {
    console.log('[配置管理器] 开始测试AI连接');
    
    if (!this.config.apiKey) {
      console.log('[配置管理器] API密钥未配置');
      return { success: false, message: '请先配置API密钥' };
    }

    const model = modelManager.getCurrentModel();
    if (!model) {
      console.log('[配置管理器] 未选择AI模型');
      return { success: false, message: '未选择AI模型' };
    }

    console.log('[配置管理器] 使用connectionTester进行真实测试', {
      model: model.name,
      proxyEnabled: this.config.proxyEnabled,
      proxyUrl: this.config.proxyUrl
    });

    // 使用connectionTester进行真实测试
    const { connectionTester } = await import('./connectionTester');
    const testResult = await connectionTester.testModel(model, this.config);
    
    console.log('[配置管理器] 连接测试结果', testResult);
    
    return {
      success: testResult.success,
      message: testResult.message || (testResult.success ? `${model.name} API连接成功` : `${model.name} API连接失败`)
    };
  }

  /**
   * 销毁配置管理器
   */
  public destroy(): void {
    window.removeEventListener('ai-model-changed', this.handleModelChange.bind(this));
    window.removeEventListener('ai-config-updated', this.handleConfigUpdate.bind(this));
    this.listeners.clear();
  }
}

// 导出单例实例
export const configManager = AIConfigManager.getInstance();