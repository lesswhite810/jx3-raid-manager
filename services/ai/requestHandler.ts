/**
 * AI请求处理模块
 * 负责处理AI请求、格式化prompt和解析响应
 */

import { AIModel, AIProvider, AIRequestParams, AIResponse, AIErrorType } from './types';
import { configManager } from './configManager';

/**
 * AI请求处理器类
 */
export class AIRequestHandler {
  private static instance: AIRequestHandler;

  private constructor() {}

  /**
   * 获取请求处理器单例
   */
  public static getInstance(): AIRequestHandler {
    if (!AIRequestHandler.instance) {
      AIRequestHandler.instance = new AIRequestHandler();
    }
    return AIRequestHandler.instance;
  }

  /**
   * 发送AI请求
   */
  public async sendRequest(params: AIRequestParams): Promise<AIResponse> {
    try {
      // 获取当前配置
      const config = configManager.getRequestConfig();
      const model = configManager.getCurrentModel();
      
      if (!model) {
        return {
          success: false,
          error: '未选择AI模型',
          errorType: AIErrorType.INVALID_REQUEST,
        };
      }
      
      // 验证配置
      const validation = configManager.validateConfig();
      if (!validation.isValid) {
        return {
          success: false,
          error: validation.error,
          errorType: AIErrorType.INVALID_REQUEST,
        };
      }
      
      // 根据模型提供商处理请求
      switch (model.provider) {
        case AIProvider.ZHIPU:
          return await this.handleZhipuRequest(params, config, model);
        case AIProvider.GEMINI:
          return await this.handleGeminiRequest(params, config, model);
        case AIProvider.OPENAI:
          return await this.handleOpenAIRequest(params, config, model);
        case AIProvider.CLAUDE:
          return await this.handleClaudeRequest(params, config, model);
        default:
          return {
            success: false,
            error: `不支持的模型提供商: ${model.provider}`,
            errorType: AIErrorType.INVALID_REQUEST,
          };
      }
    } catch (error) {
      console.error('AI请求处理失败:', error);
      return {
        success: false,
        error: `请求处理失败: ${error instanceof Error ? error.message : String(error)}`,
        errorType: AIErrorType.UNKNOWN_ERROR,
      };
    }
  }

  /**
   * 处理智谱AI请求
   */
  private async handleZhipuRequest(
    params: AIRequestParams,
    config: any,
    model: AIModel
  ): Promise<AIResponse> {
    try {
      const baseUrl = model.apiUrl || 'https://open.bigmodel.cn/api/paas/v4';
      console.log('[AI请求] 开始处理请求', {
        model: model.name,
        baseUrl,
        proxyEnabled: config.proxyEnabled,
        proxyUrl: config.proxyUrl,
        hasApiKey: !!config.apiKey
      });
      
      // 构建请求消息
      const messages = [];
      
      if (params.systemPrompt) {
        messages.push({
          role: 'system',
          content: params.systemPrompt,
        });
      }
      
      messages.push({
        role: 'user',
        content: params.prompt,
      });
      
      // 构建请求体
      const requestBody = JSON.stringify({
        model: config.model,
        messages,
        temperature: params.temperature ?? config.temperature,
        max_tokens: params.maxTokens ?? config.maxTokens,
        stream: params.stream ?? false,
      });
      
      console.log('[AI请求] 请求体构建完成', {
        messageCount: messages.length,
        temperature: params.temperature ?? config.temperature,
        maxTokens: params.maxTokens ?? config.maxTokens
      });
      
      // 构建fetch选项 - 使用完整的API密钥
      const fetchOptions: RequestInit = {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${config.apiKey}`,
        },
        body: requestBody,
        signal: AbortSignal.timeout(config.timeout || 30000),
      };
      
      // 如果启用了代理，添加代理配置
      if (config.proxyEnabled && config.proxyUrl) {
        // 注意：浏览器环境中的代理通常通过系统设置或浏览器扩展处理
        // 这里我们只是记录代理设置，实际的代理配置可能需要其他方式
        console.log(`[AI请求] 使用代理: ${config.proxyUrl}`);
      } else {
        console.log('[AI请求] 未使用代理');
      }
      
      // 发送请求 - 区分测试和实际请求
      const endpoint = params.prompt.includes('测试连接') ? '/models' : '/chat/completions';
      console.log('[AI请求] 发送请求到:', `${baseUrl}${endpoint}`);
      console.log('[AI请求] 请求类型:', params.prompt.includes('测试连接') ? '测试请求' : '实际请求');
      
      // 发送请求
      const response = await fetch(`${baseUrl}${endpoint}`, fetchOptions);
      console.log('[AI请求] 收到响应', {
        status: response.status,
        statusText: response.statusText,
        headers: Object.fromEntries(response.headers.entries())
      });
      
      if (!response.ok) {
        let errorMessage = '请求失败';
        let errorType = AIErrorType.API_ERROR;
        
        try {
          const errorData = await response.json();
          console.log('[AI请求] 错误响应数据:', errorData);
          
          // 智能解析不同类型的错误
          if (response.status === 401 || response.status === 403) {
            errorMessage = 'API密钥无效或已过期';
            errorType = AIErrorType.AUTHENTICATION_ERROR;
          } else if (response.status === 429) {
            errorMessage = '请求频率过高，请稍后重试';
            errorType = AIErrorType.RATE_LIMIT_ERROR;
          } else if (errorData.error?.message) {
            // 直接使用API返回的错误信息，包括余额不足情况
            errorMessage = errorData.error.message;
            
            // 添加更友好的提示并设置正确的错误类型
            if (errorMessage.includes('余额不足') || errorMessage.includes('充值')) {
              errorMessage = `AI服务 ${errorMessage}。建议：\n1. 检查API密钥对应的账户余额\n2. 考虑升级套餐或充值\n3. 或者尝试使用其他模型`;
              errorType = AIErrorType.PAYMENT_REQUIRED_ERROR;
            }
          } else if (errorData.message) {
            errorMessage = errorData.message;
          }
        } catch (parseError) {
          console.error('[AI请求] 解析错误响应失败:', parseError);
          errorMessage = `HTTP ${response.status}: ${response.statusText}`;
        }
        
        return {
          success: false,
          error: errorMessage,
          errorType,
        };
      }
      
      const result = await response.json();
      console.log('[AI请求] 成功响应数据:', result);
      
      // 根据端点返回不同格式的响应
      if (endpoint === '/models') {
        // 测试请求响应处理
        return {
          success: true,
          content: JSON.stringify(result),
          usage: undefined,
        };
      } else {
        // 实际请求响应处理
        const content = result.choices?.[0]?.message?.content;
        
        if (content) {
          return {
            success: true,
            content,
            usage: result.usage ? {
              promptTokens: result.usage.prompt_tokens,
              completionTokens: result.usage.completion_tokens,
              totalTokens: result.usage.total_tokens,
            } : undefined,
          };
        }
      }
      
      return {
        success: false,
        error: '未能获取AI响应内容',
        errorType: AIErrorType.API_ERROR,
      };
    } catch (error) {
      console.error('[AI请求] 请求过程中发生错误:', error);
      
      if (error instanceof Error) {
        console.error('[AI请求] 错误详情:', {
          name: error.name,
          message: error.message,
          stack: error.stack
        });
        
        if (error.name === 'AbortError') {
          return {
            success: false,
            error: '请求超时',
            errorType: AIErrorType.TIMEOUT_ERROR,
          };
        } else if (error.message.includes('fetch') || error.message.includes('network')) {
          return {
            success: false,
            error: '网络连接失败，请检查网络设置',
            errorType: AIErrorType.NETWORK_ERROR,
          };
        }
      }
      
      return {
        success: false,
        error: `请求失败: ${error instanceof Error ? error.message : String(error)}`,
        errorType: AIErrorType.NETWORK_ERROR,
      };
    }
  }

  /**
   * 处理Gemini请求
   */
  private async handleGeminiRequest(
    params: AIRequestParams,
    config: any,
    _model: AIModel
  ): Promise<AIResponse> {
    try {
      // 动态导入GoogleGenAI
      const { GoogleGenAI } = await import('@google/genai');
      
      const ai = new GoogleGenAI({ apiKey: config.apiKey });
      
      // 构建请求内容
      let content = params.prompt;
      if (params.systemPrompt) {
        content = `${params.systemPrompt}\n\n${params.prompt}`;
      }
      
      // 发送请求
      const response = await ai.models.generateContent({
        model: config.model,
        contents: content,
        config: {
          temperature: params.temperature ?? config.temperature,
          maxOutputTokens: params.maxTokens ?? config.maxTokens,
        },
      });
      
      if (response.text) {
        return {
          success: true,
          content: response.text,
          usage: undefined, // 暂时设置为undefined，因为API可能不同
        };
      }
      
      return {
        success: false,
        error: '未能获取AI响应内容',
        errorType: AIErrorType.API_ERROR,
      };
    } catch (error) {
      if (error instanceof Error) {
        if (error.message.includes('API key')) {
          return {
            success: false,
            error: 'API密钥无效',
            errorType: AIErrorType.AUTHENTICATION_ERROR,
          };
        } else if (error.message.includes('fetch') || error.message.includes('network')) {
          return {
            success: false,
            error: '网络连接失败，请检查代理设置',
            errorType: AIErrorType.NETWORK_ERROR,
          };
        } else if (error.message.includes('quota') || error.message.includes('rate limit')) {
          return {
            success: false,
            error: 'API配额已用完或请求频率过高',
            errorType: AIErrorType.RATE_LIMIT_ERROR,
          };
        } else if (error.message.includes('余额不足') || error.message.includes('充值')) {
          return {
            success: false,
            error: `AI服务 ${error.message}。建议：\n1. 检查API密钥对应的账户余额\n2. 考虑升级套餐或充值\n3. 或者尝试使用其他模型`,
            errorType: AIErrorType.PAYMENT_REQUIRED_ERROR,
          };
        }
      }
      
      return {
        success: false,
        error: `请求失败: ${error instanceof Error ? error.message : String(error)}`,
        errorType: AIErrorType.API_ERROR,
      };
    }
  }

  /**
   * 处理OpenAI请求
   */
  private async handleOpenAIRequest(
    _params: AIRequestParams,
    _config: any,
    _model: AIModel
  ): Promise<AIResponse> {
    // 这里可以实现OpenAI模型的请求处理逻辑
    return {
      success: false,
      error: 'OpenAI模型请求功能暂未实现',
      errorType: AIErrorType.INVALID_REQUEST,
    };
  }

  /**
   * 处理Claude请求
   */
  private async handleClaudeRequest(
    _params: AIRequestParams,
    _config: any,
    _model: AIModel
  ): Promise<AIResponse> {
    // 这里可以实现Claude模型的请求处理逻辑
    return {
      success: false,
      error: 'Claude模型请求功能暂未实现',
      errorType: AIErrorType.INVALID_REQUEST,
    };
  }

  /**
   * 格式化prompt
   */
  public formatPrompt(template: string, variables: Record<string, any>): string {
    let formattedPrompt = template;
    
    // 替换模板中的变量
    Object.entries(variables).forEach(([key, value]) => {
      const placeholder = `{{${key}}}`;
      formattedPrompt = formattedPrompt.replace(
        new RegExp(placeholder.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'g'),
        String(value)
      );
    });
    
    return formattedPrompt;
  }

  /**
   * 构建副本分析prompt
   */
  public buildRaidAnalysisPrompt(data: any): string {
    const template = `
分析以下剑网三金团数据并提供本周总结（150字以内，中文）：

数据概览：
- 总共 {{totalRuns}} 次副本
- 总收入 {{totalGold}} 金
- 通关 {{clearedCount}} 次（通关率 {{clearRate}}%）
- 玄晶掉落 {{xuanjingCount}} 次（掉率 {{xuanjingRate}}%）
- 平均每次 {{avgGoldPerRun}} 金

收入排行：
{{topRaids}}

{{xuanjingSection}}

请提供：
1. 玄晶运势评价（出货多就说脸好，没出货就调侃一下）
2. 收入效率评价
3. 使用剑网三游戏术语
4. 风格轻松幽默，适当玩梗

直接输出总结内容，不需要Markdown格式。
`;
    
    // 构建收入排行部分
    const topRaidsSection = data.topRaids
      ? data.topRaids.map((r: any) => `- ${r.name}: ${r.gold} 金`).join('\n')
      : '暂无数据';
    
    // 构建玄晶部分
    const xuanjingSection = data.xuanjingRaids && data.xuanjingRaids.length > 0
      ? `玄晶出货副本: ${data.xuanjingRaids.join(', ')}`
      : '本周暂无玄晶出货';
    
    return this.formatPrompt(template, {
      totalRuns: data.totalRuns || 0,
      totalGold: data.totalGold || 0,
      clearedCount: data.clearedCount || 0,
      clearRate: data.clearRate || 0,
      xuanjingCount: data.xuanjingCount || 0,
      xuanjingRate: data.xuanjingRate || 0,
      avgGoldPerRun: data.avgGoldPerRun || 0,
      topRaids: topRaidsSection,
      xuanjingSection,
    });
  }

  /**
   * 流式请求处理（预留接口）
   */
  public async sendStreamRequest(
    params: AIRequestParams,
    onChunk: (chunk: string) => void
  ): Promise<AIResponse> {
    // 这里可以实现流式请求处理逻辑
    // 目前返回普通请求结果
    const response = await this.sendRequest({ ...params, stream: true });
    
    if (response.success && response.content) {
      // 模拟流式输出
      const chunks = response.content.split(' ');
      for (const chunk of chunks) {
        onChunk(chunk + ' ');
        await new Promise(resolve => setTimeout(resolve, 50));
      }
    }
    
    return response;
  }
}

// 导出单例实例
export const requestHandler = AIRequestHandler.getInstance();
