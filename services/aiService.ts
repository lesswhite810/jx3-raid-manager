import { RaidRecord, Account } from '../types';

export interface AIAnalysisResult {
  success: boolean;
  content?: string;
  error?: string;
};

export interface AIServiceConfig {
  apiKey: string;
  model: string;
  temperature: number;
  proxyUrl?: string;
  proxyEnabled?: boolean;
  timeout?: number;
  maxRetries?: number;
}

const DEFAULT_TIMEOUT = 60000;
const DEFAULT_MAX_RETRIES = 3;

export const supportedModels = [
  { id: 'glm-4.6', name: 'GLM-4.6', provider: '智谱AI', requiresProxy: false },
  { id: 'glm-4.6-flash', name: 'GLM-4.6-Flash', provider: '智谱AI', requiresProxy: false },
  { id: 'spark-lite', name: '星火Lite', provider: '讯飞AI', requiresProxy: false, apiDoc: 'https://xinghuo.xfyun.cn/doc/spark' },
  { id: 'doubao-pro-4k', name: '豆包Pro-4k', provider: '字节跳动(火山引擎)', requiresProxy: false, apiDoc: 'https://www.volcengine.com/docs/6791/1362740' },
  { id: 'doubao-lite-4k', name: '豆包Lite-4k', provider: '字节跳动(火山引擎)', requiresProxy: false, apiDoc: 'https://www.volcengine.com/docs/6791/1362740' },
];

export async function analyzeRaidWithAI(
  records: RaidRecord[],
  _accounts: Account[],
  config: AIServiceConfig
): Promise<AIAnalysisResult> {
  console.log('[AI分析] 开始执行analyzeRaidWithAI', {
    model: config.model,
    recordCount: records.length,
    proxyEnabled: config.proxyEnabled
  });

  if (!config.apiKey) {
    console.error('[AI分析] API密钥未配置');
    return { success: false, error: 'API密钥未配置' };
  }

  const isGLM = config.model.startsWith('glm-');
  const isGemini = config.model.startsWith('gemini-');
  const isSpark = config.model.startsWith('spark-');
  const isDoubao = config.model.startsWith('doubao-');
  console.log('[AI分析] 模型类型识别', {
    isGLM,
    isGemini,
    isSpark,
    isDoubao,
    model: config.model
  });

  try {
    if (isGLM) {
      console.log('[AI分析] 调用GLM API');
      const result = await callGLMAPIWithRetry(records, _accounts, config);
      console.log('[AI分析] GLM API返回结果', { success: result.success });
      return result;
    } else if (isGemini) {
      console.log('[AI分析] 调用Gemini API');
      const result = await callGeminiAPIWithRetry(records, _accounts, config);
      console.log('[AI分析] Gemini API返回结果', { success: result.success });
      return result;
    } else if (isSpark) {
      console.log('[AI分析] 调用讯飞星火API');
      const result = await callSparkAPIWithRetry(records, _accounts, config);
      console.log('[AI分析] 讯飞星火API返回结果', { success: result.success });
      return result;
    } else if (isDoubao) {
      console.log('[AI分析] 调用豆包API');
      const result = await callDoubaoAPIWithRetry(records, _accounts, config);
      console.log('[AI分析] 豆包API返回结果', { success: result.success });
      return result;
    } else {
      console.error('[AI分析] 不支持的模型', { model: config.model });
      return {
        success: false,
        error: 'AI服务暂时不可用，请稍后重试'
      };
    }
  } catch (error) {
    console.error('[AI分析] 执行过程中发生异常', error);
    return {
      success: false,
      error: error instanceof Error ? error.message : 'AI服务暂时不可用，请稍后重试'
    };
  }
}

async function callGLMAPIWithRetry(
  records: RaidRecord[],
  _accounts: Account[],
  config: AIServiceConfig
): Promise<AIAnalysisResult> {
  const timeout = config.timeout || DEFAULT_TIMEOUT;
  const maxRetries = config.maxRetries || DEFAULT_MAX_RETRIES;
  let lastError: Error | null = null;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    console.log(`[AI分析] GLM API尝试 ${attempt}/${maxRetries}`);
    
    try {
      const result = await callGLMAPI(records, _accounts, config, timeout);
      if (result.success) {
        return result;
      }
      lastError = new Error(result.error || '未知错误');
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
      console.error(`[AI分析] GLM API第${attempt}次尝试失败`, lastError.message);
    }

    if (attempt < maxRetries) {
      const delay = Math.pow(2, attempt) * 1000;
      console.log(`[AI分析] 等待 ${delay}ms 后重试...`);
      await new Promise(resolve => setTimeout(resolve, delay));
    }
  }

  console.error('[AI分析] GLM API所有重试均失败', lastError);
  return {
    success: false,
    error: lastError?.message || 'AI服务暂时不可用，请稍后重试'
  };
}

async function callGeminiAPIWithRetry(
  records: RaidRecord[],
  _accounts: Account[],
  config: AIServiceConfig
): Promise<AIAnalysisResult> {
  const timeout = config.timeout || DEFAULT_TIMEOUT;
  const maxRetries = config.maxRetries || DEFAULT_MAX_RETRIES;
  let lastError: Error | null = null;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    console.log(`[AI分析] Gemini API尝试 ${attempt}/${maxRetries}`);
    
    try {
      const result = await callGeminiAPI(records, _accounts, config, timeout);
      if (result.success) {
        return result;
      }
      lastError = new Error(result.error || '未知错误');
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
      console.error(`[AI分析] Gemini API第${attempt}次尝试失败`, lastError.message);
    }

    if (attempt < maxRetries) {
      const delay = Math.pow(2, attempt) * 1000;
      console.log(`[AI分析] 等待 ${delay}ms 后重试...`);
      await new Promise(resolve => setTimeout(resolve, delay));
    }
  }

  console.error('[AI分析] Gemini API所有重试均失败', lastError);
  return {
    success: false,
    error: lastError?.message || 'AI服务暂时不可用，请稍后重试'
  };
}

async function callGLMAPI(
  records: RaidRecord[],
  _accounts: Account[],
  config: AIServiceConfig,
  timeout: number
): Promise<AIAnalysisResult> {
  try {
    console.log('[AI分析] 开始处理GLM API请求', { timeout });
    const dataSummary = prepareDataSummary(records);
    console.log('[AI分析] 数据摘要准备完成', {
      totalRuns: dataSummary.totalRuns,
      totalGold: dataSummary.totalGold
    });
    
    const prompt = buildPrompt(dataSummary);
    console.log('[AI分析] Prompt构建完成', { promptLength: prompt.length });

    const baseUrl = config.proxyEnabled && config.proxyUrl
      ? config.proxyUrl.trim()
      : 'https://open.bigmodel.cn/api/paas/v4';
    console.log('[AI分析] 确定请求URL', { baseUrl, proxyEnabled: config.proxyEnabled });

    const startTime = Date.now();
    console.log('[AI分析] 开始发送请求到GLM API', { startTime });
    
    const requestBody = JSON.stringify({
      model: config.model,
      messages: [
        {
          role: 'system',
          content: '你是剑网三副本数据分析助手，专门分析金团数据并提供简洁有趣的中文总结。使用游戏术语，风格轻松幽默。'
        },
        {
          role: 'user',
          content: prompt
        }
      ],
      thinking: {
        type: "enabled"
      },
      temperature: config.temperature,
      max_tokens: 2048  // 增加到2048，避免响应因长度限制被截断
    });
    
    console.log('[AI分析] 请求体构建完成', {
      model: config.model,
      temperature: config.temperature,
      maxTokens: 2048,
      messageCount: 2,
      promptLength: prompt.length,
      hasThinking: true
    });
    
    // 检查模型名称是否正确
    const validModels = ['glm-4', 'glm-4-flash', 'glm-4-air', 'glm-4-airx', 'glm-4-long', 'glm-4.6'];
    if (!validModels.some(model => config.model.includes(model))) {
      console.warn('[AI分析] 模型名称可能不正确', { 
        currentModel: config.model, 
        validModels 
      });
    }
    
    const response = await fetch(`${baseUrl}/chat/completions`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${config.apiKey}`
      },
      body: requestBody,
      signal: AbortSignal.timeout(timeout)
    });
    
    const endTime = Date.now();
    const duration = endTime - startTime;
    console.log('[AI分析] GLM API响应接收完成', {
      status: response.status,
      duration: `${duration}ms`
    });

    if (!response.ok) {
      const errorData = await response.json().catch(() => ({}));
      console.error('[AI分析] GLM API返回错误响应', {
        status: response.status,
        errorData
      });
      
      let errorMessage = errorData.error?.message || `API请求失败: ${response.status}`;
      
      // 添加余额不足错误的友好提示
      if (errorMessage.includes('余额不足') || errorMessage.includes('充值')) {
        errorMessage = `AI服务 ${errorMessage}。建议：\n1. 检查API密钥对应的账户余额\n2. 考虑升级套餐或充值\n3. 或者尝试使用其他模型`;
      } else if (response.status === 401 || response.status === 403) {
        errorMessage = 'API密钥无效或已过期，请检查配置';
      } else if (response.status === 429) {
        errorMessage = '请求频率过高，请稍后重试';
      }
      
      return {
        success: false,
        error: errorMessage
      };
    }

    console.log('[AI分析] 开始解析GLM API响应');
    const result = await response.json();
    console.log('[AI分析] GLM API响应解析完成', {
      hasChoices: !!result.choices,
      choiceCount: result.choices?.length || 0
    });
    
    const choice = result.choices?.[0];
    let content = choice?.message?.content;
    
    // GLM-4.6 with thinking enabled: content might be in reasoning_content
    if (!content && choice?.message?.reasoning_content) {
      console.log('[AI分析] 从reasoning_content字段提取内容');
      content = choice.message.reasoning_content;
    }
    
    // 如果content仍然为空，但finish_reason是length，说明内容被截断
    if (!content && choice?.finish_reason === 'length') {
      console.warn('[AI分析] 响应因长度限制被截断');
      content = choice?.message?.reasoning_content || '响应内容因长度限制被截断，请重试';
    }

    if (content) {
      console.log('[AI分析] 成功获取AI响应内容', { contentLength: content.length });
      return { success: true, content };
    }
    
    console.error('[AI分析] 未能从响应中提取内容', result);
    return { success: false, error: '未能获取AI响应内容' };
  } catch (error) {
    console.error('[AI分析] GLM API调用过程中发生异常', error);
    
    let errorMessage = 'AI服务调用失败';
    if (error instanceof Error) {
      if (error.name === 'AbortError' || error.message.includes('signal timed out') || error.message.includes('timeout')) {
        errorMessage = 'AI服务请求超时，请检查网络连接或稍后重试';
      } else if (error.message.includes('fetch') || error.message.includes('network')) {
        errorMessage = '网络连接失败，请检查网络设置';
      } else {
        errorMessage = `AI服务调用失败: ${error.message}`;
      }
    }
    
    return {
      success: false,
      error: errorMessage
    };
  }
}

async function callGeminiAPI(
  records: RaidRecord[],
  _accounts: Account[],
  config: AIServiceConfig,
  timeout: number
): Promise<AIAnalysisResult> {
  try {
    console.log('[AI分析] 开始处理Gemini API请求', { timeout });
    
    if (!config.apiKey) {
      console.error('[AI分析] Gemini API密钥未配置');
      return { success: false, error: 'API密钥未配置' };
    }

    console.log('[AI分析] 动态导入GoogleGenAI库');
    const { GoogleGenAI } = await import('@google/genai');
    
    console.log('[AI分析] 初始化GoogleGenAI实例');
    const ai = new GoogleGenAI({ apiKey: config.apiKey });
    
    console.log('[AI分析] 准备Gemini API数据');
    const dataSummary = prepareDataSummary(records);
    const prompt = buildPrompt(dataSummary);
    console.log('[AI分析] Gemini Prompt构建完成', { promptLength: prompt.length });

    const startTime = Date.now();
    console.log('[AI分析] 开始发送请求到Gemini API', { startTime });
    
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error('Gemini API请求超时')), timeout);
    });
    
    const response = await Promise.race([
      ai.models.generateContent({
        model: config.model,
        contents: prompt
      }),
      timeoutPromise
    ]);
    
    const endTime = Date.now();
    const duration = endTime - startTime;
    console.log('[AI分析] Gemini API响应接收完成', { duration: `${duration}ms` });

    const content = response.text;
    if (content) {
      console.log('[AI分析] 成功获取Gemini响应内容', { contentLength: content.length });
      return { success: true, content };
    }
    
    console.error('[AI分析] 未能从Gemini响应中提取内容', response);
    return { success: false, error: '未能获取AI响应内容' };
  } catch (error) {
    console.error('[AI分析] Gemini API调用过程中发生异常', error);
    
    let errorMessage = 'Gemini服务暂时不可用，请检查API密钥或网络连接';
    if (error instanceof Error) {
      if (error.message.includes('timeout')) {
        errorMessage = 'Gemini服务请求超时，请检查网络连接或稍后重试';
      } else if (error.message.includes('API key')) {
        errorMessage = 'Gemini API密钥无效，请检查配置';
      } else if (error.message.includes('signal timed out')) {
        errorMessage = 'Gemini服务请求超时，请检查网络连接或稍后重试';
      }
    }
    
    const result: AIAnalysisResult = {
      success: false,
      error: errorMessage
    };
    return result;
  }
}

async function callSparkAPIWithRetry(
  records: RaidRecord[],
  _accounts: Account[],
  config: AIServiceConfig
): Promise<AIAnalysisResult> {
  const timeout = config.timeout || DEFAULT_TIMEOUT;
  const maxRetries = config.maxRetries || DEFAULT_MAX_RETRIES;
  let lastError: Error | null = null;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    console.log(`[AI分析] 讯飞星火API尝试 ${attempt}/${maxRetries}`);
    
    try {
      const result = await callSparkAPI(records, config, timeout);
      if (result.success) {
        return result;
      }
      lastError = new Error(result.error || '未知错误');
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
      console.error(`[AI分析] 讯飞星火API第${attempt}次尝试失败`, lastError.message);
    }

    if (attempt < maxRetries) {
      const delay = Math.pow(2, attempt) * 1000;
      console.log(`[AI分析] 等待 ${delay}ms 后重试...`);
      await new Promise(resolve => setTimeout(resolve, delay));
    }
  }

  console.error('[AI分析] 讯飞星火API所有重试均失败', lastError);
  return {
    success: false,
    error: lastError?.message || '讯飞星火服务暂时不可用'
  };
}

async function callSparkAPI(
  records: RaidRecord[],
  config: AIServiceConfig,
  timeout: number
): Promise<AIAnalysisResult> {
  try {
    console.log('[AI分析] 开始处理讯飞星火API请求');
    
    const dataSummary = prepareDataSummary(records);
    const prompt = buildPrompt(dataSummary);
    
    const baseUrl = 'https://spark-api.xf.cn/api/paas/v4';
    
    const requestBody = JSON.stringify({
      header: {
        app_id: config.apiKey.split('.')[0] || '',
        uid: ''
      },
      parameter: {
        chat: {
          domain: 'generalv3.5',
          temperature: config.temperature,
          max_tokens: 2048
        }
      },
      payload: {
        message: {
          role: [{
            role: 'user',
            content: prompt
          }]
        }
      }
    });

    console.log('[AI分析] 讯飞星火请求构建完成');
    
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeout);
    
    const response = await fetch(`${baseUrl}/chat`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${config.apiKey}`
      },
      body: requestBody,
      signal: controller.signal
    });
    
    clearTimeout(timeoutId);
    
    if (!response.ok) {
      const errorData = await response.json().catch(() => ({}));
      return {
        success: false,
        error: errorData.message || `API请求失败: ${response.status}`
      };
    }

    const result = await response.json();
    const content = result.choices?.[0]?.message?.content;
    
    if (content) {
      console.log('[AI分析] 讯飞星火响应成功');
      return { success: true, content };
    }
    
    return { success: false, error: '未能获取讯飞星火响应' };
  } catch (error) {
    console.error('[AI分析] 讯飞星火API异常', error);
    let errorMessage = '讯飞星火服务调用失败';
    if (error instanceof Error && (error.name === 'AbortError' || error.message.includes('timeout'))) {
      errorMessage = '讯飞星火请求超时';
    }
    return { success: false, error: errorMessage };
  }
}

async function callDoubaoAPIWithRetry(
  records: RaidRecord[],
  _accounts: Account[],
  config: AIServiceConfig
): Promise<AIAnalysisResult> {
  const timeout = config.timeout || DEFAULT_TIMEOUT;
  const maxRetries = config.maxRetries || DEFAULT_MAX_RETRIES;
  let lastError: Error | null = null;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    console.log(`[AI分析] 豆包API尝试 ${attempt}/${maxRetries}`);
    
    try {
      const result = await callDoubaoAPI(records, config, timeout);
      if (result.success) {
        return result;
      }
      lastError = new Error(result.error || '未知错误');
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
      console.error(`[AI分析] 豆包API第${attempt}次尝试失败`, lastError.message);
    }

    if (attempt < maxRetries) {
      const delay = Math.pow(2, attempt) * 1000;
      await new Promise(resolve => setTimeout(resolve, delay));
    }
  }

  return { success: false, error: lastError?.message || '豆包服务暂时不可用' };
}

async function callDoubaoAPI(
  records: RaidRecord[],
  config: AIServiceConfig,
  timeout: number
): Promise<AIAnalysisResult> {
  try {
    console.log('[AI分析] 开始处理豆包API请求');
    
    const dataSummary = prepareDataSummary(records);
    const prompt = buildPrompt(dataSummary);
    
    const baseUrl = 'https://ark.cn-beijing.volces.com/api/v3';
    
    const requestBody = JSON.stringify({
      model: config.model,
      messages: [
        { role: 'system', content: '你是剑网三副本数据分析助手' },
        { role: 'user', content: prompt }
      ],
      temperature: config.temperature,
      max_tokens: 2048
    });

    console.log('[AI分析] 豆包请求构建完成');
    
    const response = await fetch(`${baseUrl}/chat/completions`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${config.apiKey}`
      },
      body: requestBody,
      signal: AbortSignal.timeout(timeout)
    });

    if (!response.ok) {
      const errorData = await response.json().catch(() => ({}));
      console.error('[AI分析] 豆包API错误', errorData);
      return { success: false, error: errorData.message || `API请求失败: ${response.status}` };
    }

    const result = await response.json();
    const content = result.choices?.[0]?.message?.content;
    
    if (content) {
      console.log('[AI分析] 豆包响应成功');
      return { success: true, content };
    }
    
    return { success: false, error: '未能获取豆包响应' };
  } catch (error) {
    console.error('[AI分析] 豆包API异常', error);
    let errorMessage = '豆包服务调用失败';
    if (error instanceof Error && (error.name === 'AbortError' || error.message.includes('timeout'))) {
      errorMessage = '豆包请求超时';
    }
    return { success: false, error: errorMessage };
  }
}

function prepareDataSummary(records: RaidRecord[]): any {
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

  return {
    totalRuns: raidCount,
    totalGold,
    xuanjingCount,
    clearedCount,
    clearRate: raidCount > 0 ? (clearedCount / raidCount * 100).toFixed(1) : 0,
    xuanjingRate: raidCount > 0 ? (xuanjingCount / raidCount * 100).toFixed(1) : 0,
    avgGoldPerRun: raidCount > 0 ? Math.round(totalGold / raidCount) : 0,
    topRaids,
    xuanjingRaids,
    recentLog: recentRecords.slice(0, 10).map(r => {
      const dateStr = typeof r.date === 'number' ? new Date(r.date).toISOString() : r.date;
      return {
        raid: r.raidName,
        gold: r.goldIncome,
        xuanjing: r.hasXuanjing,
        date: dateStr.split('T')[0]
      };
    })
  };
}

function buildPrompt(data: any): string {
  return `
分析以下剑网三金团数据并提供本周总结（150字以内，中文）：

数据概览：
- 总共 ${data.totalRuns} 次副本
- 总收入 ${data.totalGold} 金
- 通关 ${data.clearedCount} 次（通关率 ${data.clearRate}%）
- 玄晶掉落 ${data.xuanjingCount} 次（掉率 ${data.xuanjingRate}%）
- 平均每次 ${data.avgGoldPerRun} 金

收入排行：
${data.topRaids.map((r: any) => `- ${r.name}: ${r.gold} 金`).join('\n')}

${data.xuanjingRaids.length > 0 ? `玄晶出货副本: ${data.xuanjingRaids.join(', ')}` : '本周暂无玄晶出货'}

请提供：
1. 玄晶运势评价（出货多就说脸好，没出货就调侃一下）
2. 收入效率评价
3. 使用剑网三游戏术语
4. 风格轻松幽默，适当玩梗

直接输出总结内容，不需要Markdown格式。
`;
}

export async function testAIConnection(config: AIServiceConfig): Promise<AIAnalysisResult> {
  if (!config.apiKey) {
    return { success: false, error: '请先配置API密钥' };
  }

  const isGLM = config.model.startsWith('glm-');
  const isGemini = config.model.startsWith('gemini-');

  try {
    if (isGLM) {
      return await testGLMConnection(config);
    } else if (isGemini) {
      return await testGeminiConnection(config);
    } else {
      return await testGLMConnection(config);
    }
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : '连接测试失败'
    };
  }
}

async function testGLMConnection(config: AIServiceConfig): Promise<AIAnalysisResult> {
  const baseUrl = config.proxyEnabled && config.proxyUrl
    ? config.proxyUrl
    : 'https://open.bigmodel.cn/api/paas/v4';

  try {
    const response = await fetch(`${baseUrl}/models`, {
      headers: {
        'Authorization': `Bearer ${config.apiKey}`
      }
    });

    if (response.ok) {
      return { success: true, content: 'GLM API连接成功' };
    }

    const errorData = await response.json().catch(() => ({}));
    return {
      success: false,
      error: errorData.error?.message || 'API密钥无效'
    };
  } catch (error) {
    return {
      success: false,
      error: '无法连接到GLM API，请检查网络或代理设置'
    };
  }
}

async function testGeminiConnection(config: AIServiceConfig): Promise<AIAnalysisResult> {
  try {
    const { GoogleGenAI } = await import('@google/genai');
    const ai = new GoogleGenAI({ apiKey: config.apiKey });

    const response = await ai.models.generateContent({
      model: config.model,
      contents: 'test'
    });

    if (response.text !== undefined) {
      return { success: true, content: 'Gemini API连接成功' };
    }

    return { success: false, error: 'API响应异常' };
  } catch (error) {
    return {
      success: false,
      error: '无法连接到Gemini API，请检查API密钥或网络连接'
    };
  }
}

export function getModelProvider(modelId: string): string {
  const model = supportedModels.find(m => m.id === modelId);
  return model?.provider || '未知';
}

export function modelRequiresProxy(modelId: string): boolean {
  const model = supportedModels.find(m => m.id === modelId);
  return model?.requiresProxy || false;
}

export function getProxyWarning(modelId: string): string {
  const model = supportedModels.find(m => m.id === modelId);
  if (model?.requiresProxy) {
    return `【注意】${model.name}需要代理才能访问。请在代理设置中配置有效的代理服务器。`;
  }
  return '';
}
