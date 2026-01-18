/**
 * AI服务统一接口
 * 提供简化的API供外部调用
 */

import { AIResponse, AIRequestParams, AIConnectionTestResult, AIServiceConfig } from './types';
import { modelManager } from './modelManager';
import { configManager } from './configManager';
import { connectionTester } from './connectionTester';
import { requestHandler } from './requestHandler';

/**
 * AI服务类
 * 提供统一的AI功能接口
 */
export class AIService {
  private static instance: AIService;

  private constructor() {}

  /**
   * 获取AI服务单例
   */
  public static getInstance(): AIService {
    if (!AIService.instance) {
      AIService.instance = new AIService();
    }
    return AIService.instance;
  }

  /**
   * 初始化AI服务
   */
  public initialize(): void {
    modelManager.initialize();
  }

  /**
   * 获取所有可用模型
   */
  public getModels() {
    return modelManager.getModels();
  }

  /**
   * 获取当前选中的模型
   */
  public getCurrentModel() {
    return modelManager.getCurrentModel();
  }

  /**
   * 切换模型
   */
  public switchModel(modelId: string): boolean {
    return configManager.switchModel(modelId);
  }

  /**
   * 获取当前配置
   */
  public getConfig(): AIServiceConfig {
    return configManager.getConfig();
  }

  /**
   * 更新配置
   */
  public updateConfig(updates: Partial<AIServiceConfig>): void {
    configManager.updateConfig(updates);
  }

  /**
   * 设置API密钥
   */
  public setApiKey(apiKey: string): void {
    configManager.setApiKey(apiKey);
  }

  /**
   * 设置温度
   */
  public setTemperature(temperature: number): void {
    configManager.setTemperature(temperature);
  }

  /**
   * 设置代理URL
   */
  public setProxyUrl(proxyUrl: string): void {
    configManager.setProxyUrl(proxyUrl);
  }

  /**
   * 设置代理启用状态
   */
  public setProxyEnabled(enabled: boolean): void {
    configManager.setProxyEnabled(enabled);
  }

  /**
   * 测试当前模型的连通性
   */
  public async testConnection(): Promise<AIConnectionTestResult> {
    return connectionTester.testCurrentModel();
  }

  /**
   * 发送AI请求
   */
  public async sendRequest(params: AIRequestParams): Promise<AIResponse> {
    return requestHandler.sendRequest(params);
  }

  /**
   * 分析副本数据
   */
  public async analyzeRaidData(records: any[]): Promise<AIResponse> {
    // 准备数据
    const recentRecords = records.slice(0, 50);
    const totalGold = recentRecords.reduce((acc, r) => acc + r.goldIncome, 0);
    const xuanjingCount = recentRecords.filter(r => r.hasXuanjing).length;
    const clearedCount = recentRecords.filter(r => r.isCleared).length;
    const raidCount = recentRecords.length;

    const goldByRaid: Record<string, number> = {};
    const xuanjingRaids: string[] = [];

    for (const record of recentRecords) {
      if (!goldByRaid[record.raidName]) {
        goldByRaid[record.raidName] = 0;
      }
      goldByRaid[record.raidName] += record.goldIncome;
      if (record.hasXuanjing && !xuanjingRaids.includes(record.raidName)) {
        xuanjingRaids.push(record.raidName);
      }
    }

    const topRaids = Object.entries(goldByRaid)
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(([name, gold]) => ({ name, gold }));

    const data = {
      totalRuns: raidCount,
      totalGold,
      xuanjingCount,
      clearedCount,
      clearRate: raidCount > 0 ? (clearedCount / raidCount * 100).toFixed(1) : 0,
      xuanjingRate: raidCount > 0 ? (xuanjingCount / raidCount * 100).toFixed(1) : 0,
      avgGoldPerRun: raidCount > 0 ? Math.round(totalGold / raidCount) : 0,
      topRaids,
      xuanjingRaids,
    };

    // 构建prompt
    const prompt = requestHandler.buildRaidAnalysisPrompt(data);
    
    // 发送请求
    return this.sendRequest({
      prompt,
      systemPrompt: '你是剑网三副本数据分析助手，专门分析金团数据并提供简洁有趣的中文总结。使用游戏术语，风格轻松幽默。',
    });
  }

  /**
   * 流式分析副本数据
   */
  public async analyzeRaidDataStream(
    records: any[],
    onChunk: (chunk: string) => void
  ): Promise<AIResponse> {
    // 准备数据（同上）
    const recentRecords = records.slice(0, 50);
    const totalGold = recentRecords.reduce((acc, r) => acc + r.goldIncome, 0);
    const xuanjingCount = recentRecords.filter(r => r.hasXuanjing).length;
    const clearedCount = recentRecords.filter(r => r.isCleared).length;
    const raidCount = recentRecords.length;

    const goldByRaid: Record<string, number> = {};
    const xuanjingRaids: string[] = [];

    for (const record of recentRecords) {
      if (!goldByRaid[record.raidName]) {
        goldByRaid[record.raidName] = 0;
      }
      goldByRaid[record.raidName] += record.goldIncome;
      if (record.hasXuanjing && !xuanjingRaids.includes(record.raidName)) {
        xuanjingRaids.push(record.raidName);
      }
    }

    const topRaids = Object.entries(goldByRaid)
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(([name, gold]) => ({ name, gold }));

    const data = {
      totalRuns: raidCount,
      totalGold,
      xuanjingCount,
      clearedCount,
      clearRate: raidCount > 0 ? (clearedCount / raidCount * 100).toFixed(1) : 0,
      xuanjingRate: raidCount > 0 ? (xuanjingCount / raidCount * 100).toFixed(1) : 0,
      avgGoldPerRun: raidCount > 0 ? Math.round(totalGold / raidCount) : 0,
      topRaids,
      xuanjingRaids,
    };

    // 构建prompt
    const prompt = requestHandler.buildRaidAnalysisPrompt(data);
    
    // 发送流式请求
    return requestHandler.sendStreamRequest(
      {
        prompt,
        systemPrompt: '你是剑网三副本数据分析助手，专门分析金团数据并提供简洁有趣的中文总结。使用游戏术语，风格轻松幽默。',
      },
      onChunk
    );
  }

  /**
   * 验证配置
   */
  public validateConfig() {
    return configManager.validateConfig();
  }

  /**
   * 重置配置
   */
  public resetConfig(): void {
    configManager.resetToDefaults();
  }

  /**
   * 导出配置
   */
  public exportConfig(): string {
    return configManager.exportConfig();
  }

  /**
   * 导入配置
   */
  public importConfig(configJson: string): boolean {
    return configManager.importConfig(configJson);
  }
}

// 导出单例实例
export const aiService = AIService.getInstance();