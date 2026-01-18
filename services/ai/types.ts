/**
 * AI模块类型定义
 * 定义AI服务中使用的所有接口和类型
 */

// AI模型提供商
export enum AIProvider {
  ZHIPU = 'zhipu',
  GEMINI = 'gemini',
  OPENAI = 'openai',
  CLAUDE = 'claude',
}

// AI模型类型
export interface AIModel {
  id: string;
  name: string;
  provider: AIProvider;
  requiresProxy: boolean;
  maxTokens?: number;
  description?: string;
  apiUrl?: string;
}

// AI请求配置
export interface AIRequestConfig {
  apiKey: string;
  model: string;
  temperature: number;
  maxTokens?: number;
  proxyUrl?: string;
  proxyEnabled?: boolean;
  timeout?: number;
}

// AI请求参数
export interface AIRequestParams {
  prompt: string;
  systemPrompt?: string;
  temperature?: number;
  maxTokens?: number;
  stream?: boolean;
}

// AI响应结果
export interface AIResponse {
  success: boolean;
  content?: string;
  usage?: {
    promptTokens: number;
    completionTokens: number;
    totalTokens: number;
  };
  error?: string;
  errorType?: AIErrorType;
}

// AI错误类型
export enum AIErrorType {
  NETWORK_ERROR = 'network_error',
  AUTHENTICATION_ERROR = 'authentication_error',
  RATE_LIMIT_ERROR = 'rate_limit_error',
  INVALID_REQUEST = 'invalid_request',
  API_ERROR = 'api_error',
  TIMEOUT_ERROR = 'timeout_error',
  UNKNOWN_ERROR = 'unknown_error',
  PAYMENT_REQUIRED_ERROR = 'payment_required_error', // 添加余额不足错误类型
}

// 连通性测试结果
export interface AIConnectionTestResult {
  success: boolean;
  latency?: number;
  message: string;
  errorType?: AIErrorType;
  details?: any;
}

// 模型缓存状态
export interface AIModelCacheStatus {
  lastUpdated: Date;
  models: AIModel[];
  isExpired: boolean;
}

// AI服务配置
export interface AIServiceConfig {
  enabled?: boolean;
  apiKey: string;
  model: string;
  temperature: number;
  proxyUrl?: string;
  proxyEnabled?: boolean;
  maxTokens?: number;
  timeout?: number;
}

// AI服务状态
export interface AIServiceStatus {
  isInitialized: boolean;
  currentModel?: AIModel;
  config?: AIServiceConfig;
  connectionStatus?: 'connected' | 'disconnected' | 'connecting';
}

// AI配置验证结果
export interface AIConfigValidation {
  isValid: boolean;
  error?: string;
  issues?: string[];
}
