import { Config } from '../types';
import { readDir } from '@tauri-apps/api/fs';

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

const logValidationStep = (step: string, message: string, details?: any) => {
  const timestamp = new Date().toISOString();
  console.log(`[游戏目录校验][${timestamp}] ${step}: ${message}`, details ? JSON.stringify(details) : '');
};

export const isValidGamePath = async (path: string): Promise<GamePathValidationResult> => {
  const checkTime = new Date().toISOString();

  logValidationStep('开始校验', '游戏目录校验流程启动', { path, checkTime });

  if (!path || typeof path !== 'string') {
    logValidationStep('基础校验', '路径为空或非字符串', { path });
    return {
      isValid: false,
      message: '非剑网三目录',
      details: {
        checkedPath: path || '',
        expectedStructure: 'SeasunGame\\Game\\JX3\\bin\\zhcn_hd',
        currentStructure: path || '',
        missingDirectories: ['SeasunGame'],
        checkTime
      }
    };
  }

  const windowsPathRegex = /^[a-zA-Z]:\\|^\\\\|^\//;
  const pathValid = windowsPathRegex.test(path);

  logValidationStep('路径格式校验', pathValid ? '路径格式正确' : '路径格式无效', { path, regexMatch: pathValid });

  if (!pathValid) {
    return {
      isValid: false,
      message: '非剑网三目录',
      details: {
        checkedPath: path,
        expectedStructure: 'SeasunGame\\Game\\JX3\\bin\\zhcn_hd',
        currentStructure: path,
        missingDirectories: ['SeasunGame'],
        checkTime
      }
    };
  }

  logValidationStep('准备读取目录', '开始读取根目录内容', { path });

  try {
    const entries = await readDir(path);
    logValidationStep('读取目录成功', `成功读取目录，共 ${entries.length} 个条目`, { entryCount: entries.length });

    const seasunGameEntry = entries.find(entry =>
      entry.name && entry.name.toLowerCase() === 'seasungame'
    );

    let seasunGamePath: string;

    if (seasunGameEntry) {
      logValidationStep('第一级校验', '在根目录下找到 SeasunGame 目录', {
        seasunGamePath: seasunGameEntry.name,
        isDirectory: !!seasunGameEntry.children
      });
      seasunGamePath = path.replace(/\\+$/, '') + '\\SeasunGame';
    } else {
      const pathParts = path.split('\\').filter(p => p);
      const seasunGameIndex = pathParts.findIndex(p => p.toLowerCase() === 'seasungame');

      if (seasunGameIndex === -1) {
        logValidationStep('第一级校验', '路径中未找到 SeasunGame 目录', {
          path,
          pathParts: pathParts.slice(-5),
          availableEntries: entries.map(e => e.name).slice(0, 10)
        });

        return {
          isValid: false,
          message: '非剑网三目录',
          details: {
            checkedPath: path,
            expectedStructure: 'SeasunGame\\Game\\JX3\\bin\\zhcn_hd',
            currentStructure: path,
            missingDirectories: ['SeasunGame'],
            checkTime
          }
        };
      }

      const seasunGamePart = pathParts[seasunGameIndex];
      seasunGamePath = pathParts.slice(0, seasunGameIndex + 1).join('\\');

      logValidationStep('第一级校验', `在路径中找到 SeasunGame 目录`, {
        seasunGamePath: seasunGamePart,
        seasunGameFullPath: seasunGamePath,
        index: seasunGameIndex
      });
    }

    const expectedSuffix = '\\Game\\JX3\\bin\\zhcn_hd';
    const fullPath = seasunGamePath + expectedSuffix;

    logValidationStep('路径解析', '构造完整路径进行校验', {
      seasunGamePath: seasunGamePath,
      expectedSuffix: expectedSuffix,
      fullPathToVerify: fullPath
    });

    const requiredDirs = ['Game', 'JX3', 'bin', 'zhcn_hd'];
    const missingDirs: string[] = [];

    logValidationStep('开始目录链校验', '验证目录链结构', {
      startPath: seasunGamePath,
      requiredDirs
    });

    let validatePath = seasunGamePath;

    for (const dirName of requiredDirs) {
      logValidationStep(`校验目录 [${dirName}]`, `正在验证 ${dirName} 目录是否存在`, {
        parentPath: validatePath,
        targetDir: dirName
      });

      try {
        const currentEntries = await readDir(validatePath);
        const targetEntry = currentEntries.find(entry =>
          entry.name && entry.name.toLowerCase() === dirName.toLowerCase()
        );

        if (!targetEntry) {
          missingDirs.push(dirName);
          logValidationStep(`校验目录 [${dirName}]`, `未找到 ${dirName} 目录`, {
            parentPath: validatePath,
            availableEntries: currentEntries.map(e => e.name).slice(0, 10)
          });
          break;
        }

        logValidationStep(`校验目录 [${dirName}]`, `成功找到 ${dirName} 目录`, {
          dirPath: validatePath + '\\' + dirName,
          isDirectory: !!targetEntry.children
        });

        validatePath = validatePath + '\\' + dirName;
      } catch (error) {
        logValidationStep(`校验目录 [${dirName}]`, `读取目录失败`, {
          parentPath: validatePath,
          error: error instanceof Error ? error.message : String(error)
        });
        missingDirs.push(dirName);
        break;
      }
    }

    if (missingDirs.length > 0) {
      const expectedStructure = 'SeasunGame\\Game\\JX3\\bin\\zhcn_hd';
      const errorMessage = `目录结构不完整，请确保目录结构为 ${expectedStructure}`;

      logValidationStep('校验失败', `目录结构缺失`, {
        missingDirs,
        expectedStructure,
        fullExpectedPath: seasunGamePath + expectedSuffix
      });

      return {
        isValid: false,
        message: errorMessage,
        details: {
          checkedPath: path,
          expectedStructure,
          currentStructure: validatePath.replace(seasunGamePath + '\\', 'SeasunGame\\'),
          missingDirectories: missingDirs,
          checkTime
        }
      };
    }

    logValidationStep('校验成功', '所有目录结构验证通过', {
      finalPath: validatePath,
      structure: 'SeasunGame\\Game\\JX3\\bin\\zhcn_hd'
    });

    return {
      isValid: true,
      message: '游戏目录验证成功',
      details: {
        checkedPath: path,
        expectedStructure: 'SeasunGame\\Game\\JX3\\bin\\zhcn_hd',
        currentStructure: 'SeasunGame\\Game\\JX3\\bin\\zhcn_hd',
        missingDirectories: [],
        checkTime
      }
    };
  } catch (error) {
    logValidationStep('读取目录异常', '读取根目录时发生错误', {
      path,
      error: error instanceof Error ? error.message : String(error)
    });

    return {
      isValid: false,
      message: '非剑网三目录',
      details: {
        checkedPath: path,
        expectedStructure: 'SeasunGame\\Game\\JX3\\bin\\zhcn_hd',
        currentStructure: path,
        missingDirectories: ['SeasunGame'],
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