import { Config } from '../types';
import { readDir } from '@tauri-apps/plugin-fs';

export interface ConfigValidationResult {
  isValid: boolean;
  error?: string;
}

export const DEFAULT_CONFIG: Config = {
  ai: {
    enabled: false,
    apiKey: '',
    model: 'glm-4.6',
    temperature: 0.7,
    proxyUrl: '',
    proxyEnabled: false
  },
  game: {
    gameDirectory: '',
    autoDetectEnabled: false
  }
};

export const validateConfigSync = (config: Partial<Config>): ConfigValidationResult => {
  if (!config || typeof config !== 'object') {
    return { isValid: false, error: '配置格式无效' };
  }

  const errors: string[] = [];

  if (config.ai) {
    if (typeof config.ai.enabled !== 'boolean') {
      errors.push('AI启用状态必须是布尔值');
    }
    if (typeof config.ai.apiKey !== 'string') {
      errors.push('API密钥必须是字符串');
    }
    if (typeof config.ai.model !== 'string' || !config.ai.model.trim()) {
      errors.push('AI模型不能为空');
    }
    if (typeof config.ai.temperature !== 'number' || config.ai.temperature < 0 || config.ai.temperature > 1) {
      errors.push('生成温度必须在0-1之间');
    }
    if (config.ai.proxyUrl !== undefined && typeof config.ai.proxyUrl !== 'string') {
      errors.push('代理地址必须是字符串');
    }
    if (config.ai.proxyEnabled !== undefined && typeof config.ai.proxyEnabled !== 'boolean') {
      errors.push('AI代理启用状态必须是布尔值');
    }
  }

  if (config.game) {
    if (typeof config.game.gameDirectory !== 'string') {
      errors.push('游戏目录必须是字符串');
    }
    if (typeof config.game.autoDetectEnabled !== 'boolean') {
      errors.push('自动检测状态必须是布尔值');
    }
  }

  return {
    isValid: errors.length === 0,
    error: errors.length > 0 ? errors.join('; ') : undefined
  };
};

export const validateConfig = async (config: Partial<Config>): Promise<ConfigValidationResult> => {
  if (!config || typeof config !== 'object') {
    return { isValid: false, error: '配置格式无效' };
  }

  const errors: string[] = [];

  if (config.ai) {
    if (typeof config.ai.enabled !== 'boolean') {
      errors.push('AI启用状态必须是布尔值');
    }
    if (typeof config.ai.apiKey !== 'string') {
      errors.push('API密钥必须是字符串');
    }
    if (typeof config.ai.model !== 'string' || !config.ai.model.trim()) {
      errors.push('AI模型不能为空');
    }
    if (typeof config.ai.temperature !== 'number' || config.ai.temperature < 0 || config.ai.temperature > 1) {
      errors.push('生成温度必须在0-1之间');
    }
    if (config.ai.proxyUrl !== undefined && typeof config.ai.proxyUrl !== 'string') {
      errors.push('代理地址必须是字符串');
    }
    if (config.ai.proxyEnabled !== undefined && typeof config.ai.proxyEnabled !== 'boolean') {
      errors.push('AI代理启用状态必须是布尔值');
    }
  }

  if (config.game) {
    if (typeof config.game.gameDirectory !== 'string') {
      errors.push('游戏目录必须是字符串');
    }

    if (typeof config.game.autoDetectEnabled !== 'boolean') {
      errors.push('自动检测状态必须是布尔值');
    }
  }

  return {
    isValid: errors.length === 0,
    error: errors.length > 0 ? errors.join('; ') : undefined
  };
};

export const mergeConfig = (base: Config, override: Partial<Config>): Config => {
  const validation = validateConfigSync(override);
  if (!validation.isValid) {
    console.error('配置合并失败:', validation.error);
    return base;
  }

  return {
    ai: {
      ...base.ai,
      ...override.ai
    },
    game: {
      ...base.game,
      ...override.game
    }
  };
};

export const saveConfigToStorage = (config: Config): void => {
  try {
    localStorage.setItem('jx3_config', JSON.stringify(config));
  } catch (error) {
    console.error('保存配置到localStorage失败:', error);
  }
};

export const loadConfigFromStorage = (): Config => {
  try {
    const saved = localStorage.getItem('jx3_config');
    if (saved) {
      const parsed = JSON.parse(saved);
      const validation = validateConfigSync(parsed);
      if (!validation.isValid) {
        console.error('配置格式无效，使用默认配置:', validation.error);
        return DEFAULT_CONFIG;
      }
      return mergeConfig(DEFAULT_CONFIG, parsed);
    }
  } catch (error) {
    console.error('从localStorage加载配置失败:', error);
    return DEFAULT_CONFIG;
  }
  return DEFAULT_CONFIG;
};

export interface GamePathValidationResult {
  isValid: boolean;
  message: string;
  details?: {
    checkedPath: string;
    expectedStructure: string;
    currentStructure: string;
    missingDirectories: string[];
    checkTime: string;
  };
}

const GAME_RUNTIME_SUFFIX = ['Game', 'JX3', 'bin', 'zhcn_hd'] as const;
const GAME_RUNTIME_SUFFIX_PATH = 'Game\\JX3\\bin\\zhcn_hd';

const trimTrailingPathSeparators = (targetPath: string): string =>
  targetPath.replace(/[\\/]+$/gu, '');

const splitWindowsPath = (targetPath: string): string[] =>
  trimTrailingPathSeparators(targetPath).split(/[\\/]+/u).filter(Boolean);

const buildWindowsPath = (segments: string[]): string => segments.join('\\');

export const resolveGameRuntimeDirectory = (path: string): string => {
  if (!path || typeof path !== 'string') {
    return '';
  }

  const trimmedPath = trimTrailingPathSeparators(path.trim());
  if (!trimmedPath) {
    return '';
  }

  const pathSegments = splitWindowsPath(trimmedPath);
  const seasunGameIndex = pathSegments.findIndex(segment => segment.toLowerCase() === 'seasungame');

  if (seasunGameIndex === -1) {
    return trimmedPath;
  }

  const baseSegments = pathSegments.slice(0, seasunGameIndex + 1);
  return buildWindowsPath([...baseSegments, ...GAME_RUNTIME_SUFFIX]);
};

const logValidationStep = (step: string, message: string, details?: unknown) => {
  const timestamp = new Date().toISOString();
  console.log('[游戏目录校验][' + timestamp + '] ' + step + ': ' + message, details ? JSON.stringify(details) : '');
};

export const isValidGamePath = async (path: string): Promise<GamePathValidationResult> => {
  const checkTime = new Date().toISOString();
  const runtimePath = resolveGameRuntimeDirectory(path);

  logValidationStep('开始校验', '游戏目录校验流程启动', { path, runtimePath, checkTime });

  if (!runtimePath) {
    return {
      isValid: false,
      message: '非剑网三目录',
      details: {
        checkedPath: path || '',
        expectedStructure: 'SeasunGame\\' + GAME_RUNTIME_SUFFIX_PATH,
        currentStructure: path || '',
        missingDirectories: ['SeasunGame'],
        checkTime
      }
    };
  }

  const windowsPathRegex = /^[a-zA-Z]:\\|^\\\\|^\//;
  if (!windowsPathRegex.test(runtimePath)) {
    return {
      isValid: false,
      message: '非剑网三目录',
      details: {
        checkedPath: runtimePath,
        expectedStructure: 'SeasunGame\\' + GAME_RUNTIME_SUFFIX_PATH,
        currentStructure: runtimePath,
        missingDirectories: ['SeasunGame'],
        checkTime
      }
    };
  }

  try {
    const entries = await readDir(runtimePath);
    const existingEntries = new Set(entries.map(entry => entry.name?.toLowerCase()).filter(Boolean));
    const requiredEntries = ['userdata', 'interface'];
    const missingDirectories = requiredEntries.filter(entry => !existingEntries.has(entry));

    if (missingDirectories.length > 0) {
      return {
        isValid: false,
        message: '目录结构不完整，请确认目录可自动补全到 SeasunGame\\' + GAME_RUNTIME_SUFFIX_PATH,
        details: {
          checkedPath: path,
          expectedStructure: 'SeasunGame\\' + GAME_RUNTIME_SUFFIX_PATH,
          currentStructure: runtimePath,
          missingDirectories,
          checkTime
        }
      };
    }

    return {
      isValid: true,
      message: '游戏目录验证成功',
      details: {
        checkedPath: path,
        expectedStructure: 'SeasunGame\\' + GAME_RUNTIME_SUFFIX_PATH,
        currentStructure: runtimePath,
        missingDirectories: [],
        checkTime
      }
    };
  } catch (error) {
    logValidationStep('读取目录异常', '读取运行时目录时发生错误', {
      runtimePath,
      error: error instanceof Error ? error.message : String(error)
    });

    return {
      isValid: false,
      message: '非剑网三目录',
      details: {
        checkedPath: path,
        expectedStructure: 'SeasunGame\\' + GAME_RUNTIME_SUFFIX_PATH,
        currentStructure: runtimePath,
        missingDirectories: ['userdata', 'interface'],
        checkTime
      }
    };
  }
};
export const isValidApiKey = (apiKey: string): boolean => {
  if (!apiKey || typeof apiKey !== 'string') {
    return false;
  }

  return apiKey.trim().length > 0;
};

export const isValidTemperature = (temperature: number): boolean => {
  return typeof temperature === 'number' && temperature >= 0 && temperature <= 1;
};

export const isValidProxyUrl = (url: string): boolean => {
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

export const getConfigSummary = (config: Config): string => {
  const aiStatus = config.ai.enabled ? '已启用' : '未启用';
  const modelInfo = config.ai.model ? `${config.ai.model} (${config.ai.temperature.toFixed(1)})` : '未设置';
  const proxyStatus = config.ai.proxyEnabled ? `已启用 (${config.ai.proxyUrl})` : '未启用';

  return `AI: ${aiStatus}, 模型: ${modelInfo}, 代理: ${proxyStatus}`;
};