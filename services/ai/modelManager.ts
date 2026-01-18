/**
 * AI模型管理模块
 * 负责AI模型的获取、缓存和管理
 */

import { AIModel, AIProvider, AIModelCacheStatus } from './types';

// 默认模型列表
const DEFAULT_MODELS: AIModel[] = [
  {
    id: 'glm-4.6',
    name: 'GLM-4.6',
    provider: AIProvider.ZHIPU,
    requiresProxy: false,
    maxTokens: 4096,
    description: '智谱AI GLM-4.6 模型，适合中文对话和分析',
    apiUrl: 'https://open.bigmodel.cn/api/paas/v4',
  },
  {
    id: 'glm-4.6-flash',
    name: 'GLM-4.6-Flash',
    provider: AIProvider.ZHIPU,
    requiresProxy: false,
    maxTokens: 8192,
    description: '智谱AI GLM-4.6-Flash 模型，快速响应版本',
    apiUrl: 'https://open.bigmodel.cn/api/paas/v4',
  },
  {
    id: 'gemini-1.5-flash',
    name: 'Gemini 1.5 Flash',
    provider: AIProvider.GEMINI,
    requiresProxy: true,
    maxTokens: 8192,
    description: 'Google Gemini 1.5 Flash 模型，需要代理访问',
  },
  {
    id: 'gemini-1.5-pro',
    name: 'Gemini 1.5 Pro',
    provider: AIProvider.GEMINI,
    requiresProxy: true,
    maxTokens: 4096,
    description: 'Google Gemini 1.5 Pro 模型，需要代理访问',
  },
];

// 缓存键名
const MODELS_CACHE_KEY = 'ai_models_cache';
const CACHE_EXPIRY_TIME = 24 * 60 * 60 * 1000; // 24小时

/**
 * AI模型管理器类
 */
export class AIModelManager {
  private static instance: AIModelManager;
  private models: AIModel[] = [];
  private currentModelId: string | null = null;
  private cacheStatus: AIModelCacheStatus | null = null;

  private constructor() {
    this.loadModelsFromCache();
    if (this.models.length === 0) {
      this.models = [...DEFAULT_MODELS];
      this.saveModelsToCache();
    }
  }

  /**
   * 获取模型管理器单例
   */
  public static getInstance(): AIModelManager {
    if (!AIModelManager.instance) {
      AIModelManager.instance = new AIModelManager();
    }
    return AIModelManager.instance;
  }

  /**
   * 从缓存加载模型列表
   */
  private loadModelsFromCache(): void {
    try {
      const cachedData = localStorage.getItem(MODELS_CACHE_KEY);
      if (cachedData) {
        const parsedData = JSON.parse(cachedData);
        const lastUpdated = new Date(parsedData.lastUpdated);
        const isExpired = Date.now() - lastUpdated.getTime() > CACHE_EXPIRY_TIME;
        
        this.models = parsedData.models || [];
        this.cacheStatus = {
          lastUpdated,
          models: this.models,
          isExpired
        };
      }
    } catch (error) {
      console.error('加载模型缓存失败:', error);
      this.models = [...DEFAULT_MODELS];
      this.cacheStatus = {
        lastUpdated: new Date(),
        models: this.models,
        isExpired: false
      };
    }
  }

  /**
   * 保存模型列表到缓存
   */
  private saveModelsToCache(): void {
    try {
      const now = new Date();
      const cacheData = {
        lastUpdated: now.toISOString(),
        models: this.models
      };
      
      localStorage.setItem(MODELS_CACHE_KEY, JSON.stringify(cacheData));
      this.cacheStatus = {
        lastUpdated: now,
        models: this.models,
        isExpired: false
      };
    } catch (error) {
      console.error('保存模型缓存失败:', error);
    }
  }

  /**
   * 获取所有模型列表
   * @param forceRefresh 是否强制刷新缓存
   */
  public getModels(forceRefresh: boolean = false): AIModel[] {
    if (forceRefresh || this.cacheStatus?.isExpired) {
      this.refreshModels();
    }
    return [...this.models];
  }

  /**
   * 根据ID获取模型
   */
  public getModelById(id: string): AIModel | undefined {
    return this.models.find(model => model.id === id);
  }

  /**
   * 根据提供商获取模型列表
   */
  public getModelsByProvider(provider: AIProvider): AIModel[] {
    return this.models.filter(model => model.provider === provider);
  }

  /**
   * 获取当前选中的模型
   */
  public getCurrentModel(): AIModel | null {
    if (!this.currentModelId) {
      // 如果没有选中模型，返回第一个可用模型
      if (this.models.length > 0) {
        this.currentModelId = this.models[0].id;
        this.saveCurrentModelToStorage();
      }
    }
    
    return this.currentModelId ? this.getModelById(this.currentModelId) || null : null;
  }

  /**
   * 设置当前选中的模型
   */
  public setCurrentModel(modelId: string): boolean {
    const model = this.getModelById(modelId);
    if (!model) {
      console.error(`未找到ID为 ${modelId} 的模型`);
      return false;
    }
    
    this.currentModelId = modelId;
    this.saveCurrentModelToStorage();
    return true;
  }

  /**
   * 保存当前模型ID到本地存储
   */
  private saveCurrentModelToStorage(): void {
    if (this.currentModelId) {
      localStorage.setItem('ai_current_model', this.currentModelId);
    }
  }

  /**
   * 从本地存储加载当前模型ID
   */
  private loadCurrentModelFromStorage(): void {
    try {
      const savedModelId = localStorage.getItem('ai_current_model');
      if (savedModelId) {
        this.currentModelId = savedModelId;
      }
    } catch (error) {
      console.error('加载当前模型ID失败:', error);
    }
  }

  /**
   * 刷新模型列表
   */
  public async refreshModels(): Promise<AIModel[]> {
    try {
      // 这里可以添加从远程API获取模型列表的逻辑
      // 目前使用默认模型列表
      this.models = [...DEFAULT_MODELS];
      this.saveModelsToCache();
      return [...this.models];
    } catch (error) {
      console.error('刷新模型列表失败:', error);
      return this.models;
    }
  }

  /**
   * 添加新模型
   */
  public addModel(model: AIModel): boolean {
    // 检查是否已存在相同ID的模型
    if (this.models.some(m => m.id === model.id)) {
      console.error(`已存在ID为 ${model.id} 的模型`);
      return false;
    }
    
    this.models.push(model);
    this.saveModelsToCache();
    return true;
  }

  /**
   * 更新模型信息
   */
  public updateModel(id: string, updates: Partial<AIModel>): boolean {
    const index = this.models.findIndex(model => model.id === id);
    if (index === -1) {
      console.error(`未找到ID为 ${id} 的模型`);
      return false;
    }
    
    this.models[index] = { ...this.models[index], ...updates };
    this.saveModelsToCache();
    return true;
  }

  /**
   * 删除模型
   */
  public removeModel(id: string): boolean {
    const index = this.models.findIndex(model => model.id === id);
    if (index === -1) {
      console.error(`未找到ID为 ${id} 的模型`);
      return false;
    }
    
    this.models.splice(index, 1);
    this.saveModelsToCache();
    
    // 如果删除的是当前模型，则切换到第一个可用模型
    if (this.currentModelId === id) {
      this.currentModelId = this.models.length > 0 ? this.models[0].id : null;
      this.saveCurrentModelToStorage();
    }
    
    return true;
  }

  /**
   * 获取缓存状态
   */
  public getCacheStatus(): AIModelCacheStatus | null {
    return this.cacheStatus;
  }

  /**
   * 清除缓存
   */
  public clearCache(): void {
    try {
      localStorage.removeItem(MODELS_CACHE_KEY);
      this.models = [...DEFAULT_MODELS];
      this.cacheStatus = {
        lastUpdated: new Date(),
        models: this.models,
        isExpired: false
      };
    } catch (error) {
      console.error('清除模型缓存失败:', error);
    }
  }

  /**
   * 初始化模型管理器
   */
  public initialize(): void {
    this.loadCurrentModelFromStorage();
    
    // 如果当前模型ID不存在于模型列表中，则重置为第一个模型
    if (this.currentModelId && !this.getModelById(this.currentModelId)) {
      this.currentModelId = this.models.length > 0 ? this.models[0].id : null;
      this.saveCurrentModelToStorage();
    }
  }
}

// 导出单例实例
export const modelManager = AIModelManager.getInstance();

// 初始化模型管理器
modelManager.initialize();