/**
 * AI连通性测试模块
 * 负责测试AI模型的连通性和可用性
 */

import { AIModel, AIProvider, AIConnectionTestResult, AIErrorType } from './types';
import { configManager } from './configManager';

/**
 * AI连通性测试器类
 */
export class AIConnectionTester {
  private static instance: AIConnectionTester;

  private constructor() {}

  /**
   * 获取连通性测试器单例
   */
  public static getInstance(): AIConnectionTester {
    if (!AIConnectionTester.instance) {
      AIConnectionTester.instance = new AIConnectionTester();
    }
    return AIConnectionTester.instance;
  }

  /**
   * 测试当前模型的连通性
   */
  public async testCurrentModel(): Promise<AIConnectionTestResult> {
    const config = configManager.getConfig();
    const model = configManager.getCurrentModel();
    
    if (!model) {
      return {
        success: false,
        message: '未选择AI模型',
        errorType: AIErrorType.INVALID_REQUEST,
      };
    }
    
    return this.testModel(model, config);
  }

  /**
   * 测试指定模型的连通性
   */
  public async testModel(model: AIModel, config?: any): Promise<AIConnectionTestResult> {
    const startTime = Date.now();
    
    try {
      // 使用传入的配置或当前配置
      const testConfig = config || configManager.getConfig();
      
      // 验证基本配置
      if (!testConfig.apiKey) {
        return {
          success: false,
          message: 'API密钥未配置',
          errorType: AIErrorType.AUTHENTICATION_ERROR,
        };
      }
      
      // 根据模型提供商进行不同的测试
      switch (model.provider) {
        case AIProvider.ZHIPU:
          return await this.testZhipuModel(model, testConfig, startTime);
        case AIProvider.GEMINI:
          return await this.testGeminiModel(model, testConfig, startTime);
        case AIProvider.OPENAI:
          return await this.testOpenAIModel(model, testConfig, startTime);
        case AIProvider.CLAUDE:
          return await this.testClaudeModel(model, testConfig, startTime);
        default:
          return {
            success: false,
            message: `不支持的模型提供商: ${model.provider}`,
            errorType: AIErrorType.INVALID_REQUEST,
          };
      }
    } catch (error) {
      const latency = Date.now() - startTime;
      return {
        success: false,
        message: `测试失败: ${error instanceof Error ? error.message : String(error)}`,
        errorType: AIErrorType.UNKNOWN_ERROR,
        latency,
      };
    }
  }

  /**
   * 测试智谱AI模型 - 修复：使用完整的API密钥
   */
  private async testZhipuModel(
    model: AIModel, 
    config: any, 
    startTime: number
  ): Promise<AIConnectionTestResult> {
    try {
      const baseUrl = model.apiUrl || 'https://open.bigmodel.cn/api/paas/v4';
      console.log('[连接测试] 开始测试智谱AI模型', {
        model: model.name,
        baseUrl,
        proxyEnabled: config.proxyEnabled,
        proxyUrl: config.proxyUrl,
        hasApiKey: !!config.apiKey
      });
      
      // 构建fetch选项 - 修复：使用完整的API密钥
      const fetchOptions: RequestInit = {
        method: 'GET',
        headers: {
          'Authorization': `Bearer ${config.apiKey}`,
        },
        signal: AbortSignal.timeout(config.timeout || 30000),
      };
      
      // 如果启用了代理，添加代理配置
      if (config.proxyEnabled && config.proxyUrl) {
        // 注意：浏览器环境中的代理通常通过系统设置或浏览器扩展处理
        // 这里我们只是记录代理设置，实际的代理配置可能需要其他方式
        console.log(`[连接测试] 使用代理: ${config.proxyUrl}`);
      } else {
        console.log('[连接测试] 未使用代理');
      }
      
      // 测试模型列表接口
      console.log('[连接测试] 发送请求到:', `${baseUrl}/models`);
      const response = await fetch(`${baseUrl}/models`, fetchOptions);
      console.log('[连接测试] 收到响应', {
        status: response.status,
        statusText: response.statusText,
        headers: Object.fromEntries(response.headers.entries())
      });
      
      const latency = Date.now() - startTime;
      
      if (response.ok) {
        console.log('[连接测试] 连接成功', { latency });
        return {
          success: true,
          message: `${model.name} 连接成功`,
          latency,
        };
      } else {
        let errorMessage = '连接失败';
        let errorType = AIErrorType.API_ERROR;
        
        try {
          const errorData = await response.json();
          console.log('[连接测试] 错误响应数据:', errorData);
          
          if (response.status === 401 || response.status === 403) {
            errorMessage = 'API密钥无效或已过期';
            errorType = AIErrorType.AUTHENTICATION_ERROR;
          } else if (response.status === 429) {
            errorMessage = '请求频率过高，请稍后重试';
            errorType = AIErrorType.RATE_LIMIT_ERROR;
          } else if (errorData.error?.message) {
            errorMessage = errorData.error.message;
          }
        } catch (parseError) {
          console.error('[连接测试] 解析错误响应失败:', parseError);
          errorMessage = `HTTP ${response.status}: ${response.statusText}`;
        }
        
        return {
          success: false,
          message: errorMessage,
          errorType,
          latency,
        };
      }
    } catch (error) {
      const latency = Date.now() - startTime;
      console.error('[连接测试] 请求过程中发生错误:', error);
      
      if (error instanceof Error) {
        console.error('[连接测试] 错误详情:', {
          name: error.name,
          message: error.message,
          stack: error.stack
        });
        
        if (error.name === 'AbortError') {
          return {
            success: false,
            message: '请求超时',
            errorType: AIErrorType.TIMEOUT_ERROR,
            latency,
          };
        } else if (error.message.includes('fetch') || error.message.includes('network')) {
          return {
            success: false,
            message: '网络连接失败，请检查网络设置',
            errorType: AIErrorType.NETWORK_ERROR,
            latency,
          };
        }
      }
      
      return {
        success: false,
        message: `连接失败: ${error instanceof Error ? error.message : String(error)}`,
        errorType: AIErrorType.NETWORK_ERROR,
        latency,
      };
    }
  }

  /**
   * 测试Gemini模型
   */
  private async testGeminiModel(
    model: AIModel, 
    config: any, 
    startTime: number
  ): Promise<AIConnectionTestResult> {
    try {
      // 动态导入GoogleGenAI
      const { GoogleGenAI } = await import('@google/genai');
      
      const ai = new GoogleGenAI({ apiKey: config.apiKey });
      
      // 发送测试请求
      const response = await ai.models.generateContent({
        model: model.id,
        contents: '测试连接',
      });
      
      const latency = Date.now() - startTime;
      
      if (response.text !== undefined) {
        return {
          success: true,
          message: `${model.name} 连接成功`,
          latency,
        };
      } else {
        return {
          success: false,
          message: 'API响应异常',
          errorType: AIErrorType.API_ERROR,
          latency,
        };
      }
    } catch (error) {
      const latency = Date.now() - startTime;
      
      if (error instanceof Error) {
        if (error.message.includes('API key')) {
          return {
            success: false,
            message: 'API密钥无效',
            errorType: AIErrorType.AUTHENTICATION_ERROR,
            latency,
          };
        } else if (error.message.includes('fetch') || error.message.includes('network')) {
          return {
            success: false,
            message: '网络连接失败，请检查代理设置',
            errorType: AIErrorType.NETWORK_ERROR,
            latency,
          };
        } else if (error.message.includes('quota') || error.message.includes('rate limit')) {
          return {
            success: false,
            message: 'API配额已用完或请求频率过高',
            errorType: AIErrorType.RATE_LIMIT_ERROR,
            latency,
          };
        }
      }
      
      return {
        success: false,
        message: `连接失败: ${error instanceof Error ? error.message : String(error)}`,
        errorType: AIErrorType.API_ERROR,
        latency,
      };
    }
  }

  /**
   * 测试OpenAI模型
   */
  private async testOpenAIModel(
    _model: AIModel, 
    _config: any, 
    _startTime: number
  ): Promise<AIConnectionTestResult> {
    // 这里可以实现OpenAI模型的测试逻辑
    return {
      success: false,
      message: 'OpenAI模型测试功能暂未实现',
      errorType: AIErrorType.INVALID_REQUEST,
    };
  }

  /**
   * 测试Claude模型
   */
  private async testClaudeModel(
    _model: AIModel, 
    _config: any, 
    _startTime: number
  ): Promise<AIConnectionTestResult> {
    // 这里可以实现Claude模型的测试逻辑
    return {
      success: false,
      message: 'Claude模型测试功能暂未实现',
      errorType: AIErrorType.INVALID_REQUEST,
    };
  }

  /**
   * 批量测试多个模型
   */
  public async testMultipleModels(models: AIModel[]): Promise<AIConnectionTestResult[]> {
    const results: AIConnectionTestResult[] = [];
    
    // 并行测试所有模型
    const promises = models.map(model => this.testModel(model));
    const testResults = await Promise.allSettled(promises);
    
    // 处理结果
    testResults.forEach((result) => {
      if (result.status === 'fulfilled') {
        results.push(result.value);
      } else {
        results.push({
          success: false,
          message: `测试失败: ${result.reason}`,
          errorType: AIErrorType.UNKNOWN_ERROR,
        });
      }
    });
    
    return results;
  }

  /**
   * 测试网络连接
   */
  public async testNetworkConnectivity(url?: string): Promise<boolean> {
    try {
      const testUrl = url || 'https://www.google.com';
      await fetch(testUrl, {
        method: 'HEAD',
        mode: 'no-cors',
        signal: AbortSignal.timeout(5000),
      });
      return true;
    } catch (error) {
      console.error('网络连接测试失败:', error);
      return false;
    }
  }

  /**
   * 测试代理连接
   */
  public async testProxyConnection(_proxyUrl: string): Promise<boolean> {
    try {
      // 这里可以实现代理连接测试逻辑
      // 例如通过代理发送一个简单的请求
      return true;
    } catch (error) {
      console.error('代理连接测试失败:', error);
      return false;
    }
  }
}

// 导出单例实例
export const connectionTester = AIConnectionTester.getInstance();
