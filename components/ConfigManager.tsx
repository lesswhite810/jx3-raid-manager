/**
 * 系统配置管理组件
 * 使用新的AI模块替换原有AI相关功能
 */

import React, { useState, useEffect, useRef } from 'react';
import { Config } from '../types';
import { Check, AlertTriangle, Save, FolderOpen, Settings, Network, Zap } from 'lucide-react';
import {
  validateConfig,
  saveConfigToStorage,
  isValidGamePath
} from '../utils/configUtils';
import { aiService } from '../services/ai';

interface ConfigManagerProps {
  config: Config;
  setConfig: React.Dispatch<React.SetStateAction<Config>>;
}

export const ConfigManager: React.FC<ConfigManagerProps> = ({ config, setConfig }) => {
  const [isSaving, setIsSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [pathValid, setPathValid] = useState<boolean | null>(null);
  const [testingAI, setTestingAI] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);

  // 使用 useRef 跟踪最新的配置值
  const configRef = useRef(config);
  configRef.current = config;

  // 同步AI配置到全局配置
  useEffect(() => {
    // 从AI服务获取配置
    const aiConfig = aiService.getConfig();

    // 更新全局配置
    const newConfig = {
      ...config,
      ai: {
        enabled: aiConfig.enabled ?? !!aiConfig.apiKey,
        apiKey: aiConfig.apiKey || '',
        model: aiConfig.model || 'glm-4.6',
        temperature: aiConfig.temperature || 0.7,
        proxyUrl: aiConfig.proxyUrl || '',
        proxyEnabled: aiConfig.proxyEnabled || false
      }
    };

    // 只有当配置不同时才更新
    if (JSON.stringify(newConfig) !== JSON.stringify(config)) {
      setConfig(newConfig);
    }
  }, []);

  const handleSaveConfig = async () => {
    setIsSaving(true);
    setSaveSuccess(false);
    setTestResult(null);

    // 使用 ref 获取最新的配置值
    const currentConfig = configRef.current;
    console.log('[ConfigManager] 保存配置 - 当前配置:', {
      aiEnabled: currentConfig.ai.enabled,
      apiKeyConfigured: !!currentConfig.ai.apiKey,
      model: currentConfig.ai.model,
      proxyEnabled: currentConfig.ai.proxyEnabled,
      fullConfig: JSON.stringify(currentConfig, null, 2)
    });

    try {
      const validation = await validateConfig(currentConfig);
      if (!validation.isValid) {
        setIsSaving(false);
        setTestResult({ success: false, message: `配置验证失败: ${validation.error}` });
        return;
      }

      console.log('[ConfigManager] 配置验证通过，开始保存...');

      // 更新 AI 服务配置
      console.log('[ConfigManager] 更新AI服务配置');
      aiService.updateConfig({
        enabled: currentConfig.ai.enabled,
        apiKey: currentConfig.ai.apiKey,
        model: currentConfig.ai.model,
        temperature: currentConfig.ai.temperature,
        proxyUrl: currentConfig.ai.proxyUrl,
        proxyEnabled: currentConfig.ai.proxyEnabled
      });

      // 保存到本地存储
      console.log('[ConfigManager] 保存配置到localStorage');
      saveConfigToStorage(currentConfig);

      // 验证保存结果
      const savedConfig = localStorage.getItem('jx3_config');
      if (savedConfig) {
        const parsed = JSON.parse(savedConfig);
        console.log('[ConfigManager] 验证保存结果', {
          hasAiConfig: !!parsed.ai,
          aiEnabled: parsed.ai?.enabled,
          aiApiKey: parsed.ai?.apiKey ? '✓ 已保存' : '✗ 未保存',
          aiModel: parsed.ai?.model || '✗ 未保存',
          aiProxyEnabled: parsed.ai?.proxyEnabled,
          fullConfig: JSON.stringify(parsed, null, 2)
        });
      } else {
        console.warn('[ConfigManager] localStorage中没有找到保存的配置');
      }

      setIsSaving(false);
      setSaveSuccess(true);
      console.log('[ConfigManager] 配置保存成功');

      setTimeout(() => setSaveSuccess(false), 3000);
    } catch (error) {
      console.error('[ConfigManager] 保存配置失败:', error);
      setIsSaving(false);
      setTestResult({ success: false, message: '保存失败，请重试' });
    }
  };

  const handleConfigChange = (section: keyof Config, key: string, value: any) => {
    const newConfig = { ...config, [section]: { ...config[section], [key]: value } };
    setConfig(newConfig);

    if (section === 'game' && key === 'gameDirectory') {
      isValidGamePath(value).then(result => {
        setPathValid(result.isValid ? true : false);
      });

      const gameConfig = { ...config.game, gameDirectory: value };
      saveConfigToStorage({ ...config, game: gameConfig });
    }

    if (section === 'ai') {
      // 只在 API key 变化时同步到 AI 服务（用于实时验证）
      if (key === 'apiKey') {
        aiService.setApiKey(value || '');
      }
    }
  };

  const handleTestAIConnection = async () => {
    const aiConfig = aiService.getConfig();

    if (!aiConfig.apiKey) {
      setTestResult({ success: false, message: '请先配置API密钥' });
      return;
    }

    setTestingAI(true);
    setTestResult(null);

    try {
      const result = await aiService.testConnection();
      setTestResult(result);
    } catch (error) {
      console.error('连接测试失败:', error);
      setTestResult({ success: false, message: '连接测试失败' });
    } finally {
      setTestingAI(false);
    }
  };

  // 获取当前模型信息
  const getCurrentModel = () => {
    const models = aiService.getModels();
    return models.find((m: any) => m.id === config.ai.model);
  };

  const selectedModel = getCurrentModel();

  return (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <h2 className="text-2xl font-bold text-main">系统配置</h2>
        <div className="flex gap-2">
          <button
            onClick={handleSaveConfig}
            className="btn btn-primary flex items-center gap-2"
            disabled={isSaving}
          >
            {isSaving ? (
              <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
            ) : (
              <Save className="w-4 h-4" />
            )}
            {isSaving ? '保存中...' : '保存配置'}
          </button>
        </div>
      </div>

      {saveSuccess && (
        <div className="bg-emerald-50 dark:bg-emerald-900/10 border border-emerald-200 dark:border-emerald-800 rounded-lg p-4 flex items-center gap-2 animate-in fade-in slide-in-from-top-2 duration-300">
          <Check className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
          <p className="text-sm text-emerald-700 dark:text-emerald-300">配置保存成功！</p>
        </div>
      )}

      <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
        <div className="flex items-center gap-3 mb-6">
          <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
            <Zap className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-lg font-bold text-main">AI功能配置</h3>
            <p className="text-xs text-muted">配置AI助手以获得智能分析建议</p>
          </div>
        </div>

        <div className="space-y-5">
          <div className="flex items-center justify-between p-4 bg-base/50 rounded-lg border border-base">
            <div>
              <h4 className="text-sm font-medium text-main">启用AI功能</h4>
            </div>
            <label className="relative inline-flex items-center cursor-pointer">
              <input
                type="checkbox"
                checked={config.ai.enabled}
                onChange={(e) => handleConfigChange('ai', 'enabled', e.target.checked)}
                className="sr-only peer"
              />
              <div className="w-11 h-6 bg-zinc-200 dark:bg-zinc-700 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-primary/20 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
            </label>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-center">
            <label className="text-sm font-medium text-muted">
              API密钥
            </label>
            <div className="col-span-2">
              <input
                type="password"
                value={config.ai.apiKey}
                onChange={(e) => handleConfigChange('ai', 'apiKey', e.target.value)}
                className="w-full px-3 py-2 bg-base/50 border border-base rounded-lg text-main focus:bg-surface focus:ring-1 focus:ring-primary focus:border-primary transition-all placeholder:text-muted/50 text-sm"
                placeholder="sk-..."
                disabled={!config.ai.enabled}
              />
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-start">
            <label className="text-sm font-medium text-muted pt-2">
              AI模型
            </label>
            <div className="col-span-2">
              <select
                value={config.ai.model}
                onChange={(e) => handleConfigChange('ai', 'model', e.target.value)}
                className="w-full px-3 py-2 bg-base/50 border border-base rounded-lg text-main focus:bg-surface focus:ring-1 focus:ring-primary focus:border-primary transition-all text-sm appearance-none cursor-pointer"
                disabled={!config.ai.enabled}
              >
                {aiService.getModels().map((model: any) => (
                  <option key={model.id} value={model.id}>{model.name}</option>
                ))}
              </select>
              {selectedModel && (
                <div className="flex items-center gap-2 mt-2">
                  <span className="text-xs px-2 py-0.5 rounded-full bg-base text-muted border border-base">
                    {selectedModel.provider}
                  </span>
                  {selectedModel.requiresProxy && (
                    <span className="text-xs px-2 py-0.5 rounded-full bg-amber-50 text-amber-600 border border-amber-200 flex items-center gap-1">
                      <AlertTriangle className="w-3 h-3" /> 需要代理
                    </span>
                  )}
                </div>
              )}
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-center">
            <label className="text-sm font-medium text-muted">
              生成温度
            </label>
            <div className="col-span-2">
              <div className="flex items-center gap-4">
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.1"
                  value={config.ai.temperature}
                  onChange={(e) => handleConfigChange('ai', 'temperature', parseFloat(e.target.value))}
                  className="w-full h-1.5 bg-base rounded-full appearance-none cursor-pointer accent-primary"
                  disabled={!config.ai.enabled}
                />
                <span className="text-sm font-mono text-main w-8 text-right">{config.ai.temperature.toFixed(1)}</span>
              </div>
            </div>
          </div>

          {selectedModel?.requiresProxy && !config.ai.proxyEnabled && (
            <div className="bg-amber-50 dark:bg-amber-900/10 border border-amber-200 dark:border-amber-800 rounded-lg p-3 flex items-center gap-2">
              <AlertTriangle className="w-4 h-4 text-amber-600 dark:text-amber-500" />
              <p className="text-sm text-amber-700 dark:text-amber-400">
                {selectedModel.name}需要代理才能访问，请配置下方代理设置
              </p>
            </div>
          )}

          <div className="border-t border-base pt-5 mt-2">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center gap-2">
                <div className="p-1.5 bg-base rounded-md">
                  <Network className="w-4 h-4 text-muted" />
                </div>
                <div>
                  <h4 className="text-sm font-medium text-main">代理设置</h4>
                  <p className="text-xs text-muted">用于访问需要代理的AI模型</p>
                </div>
              </div>
              <label className="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  checked={config.ai.proxyEnabled}
                  onChange={(e) => handleConfigChange('ai', 'proxyEnabled', e.target.checked)}
                  className="sr-only peer"
                />
                <div className="w-11 h-6 bg-zinc-200 dark:bg-zinc-700 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-primary/20 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
              </label>
            </div>

            {config.ai.proxyEnabled && (
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-center animate-in slide-in-from-top-2 duration-200">
                <label className="text-sm font-medium text-muted">
                  代理地址
                </label>
                <div className="col-span-2">
                  <input
                    type="text"
                    value={config.ai.proxyUrl}
                    onChange={(e) => handleConfigChange('ai', 'proxyUrl', e.target.value)}
                    className="w-full px-3 py-2 bg-base/50 border border-base rounded-lg text-main focus:bg-surface focus:ring-1 focus:ring-primary focus:border-primary transition-all placeholder:text-muted/50 text-sm"
                    placeholder="http://127.0.0.1:7890"
                  />
                  <p className="text-xs text-muted mt-1">
                    支持HTTP/HTTPS/SOCKS代理，格式: 协议://地址:端口
                  </p>
                </div>
              </div>
            )}
          </div>

          <div className="flex items-center gap-3 pt-2">
            <button
              onClick={handleTestAIConnection}
              disabled={!config.ai.enabled || testingAI}
              className="btn btn-secondary flex items-center gap-2 text-sm"
            >
              {testingAI ? (
                <div className="w-3.5 h-3.5 border-2 border-currentColor border-t-transparent rounded-full animate-spin"></div>
              ) : (
                <Settings className="w-3.5 h-3.5" />
              )}
              {testingAI ? '测试中...' : '测试连接'}
            </button>
            {testResult && (
              <span className={`text-sm flex items-center gap-1.5 ${testResult.success ? 'text-emerald-600' : 'text-red-600'}`}>
                {testResult.success ? <Check className="w-4 h-4" /> : <AlertTriangle className="w-4 h-4" />}
                {testResult.message}
              </span>
            )}
          </div>
        </div>
      </div>

      <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
        <div className="flex items-center gap-3 mb-6">
          <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
            <FolderOpen className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-lg font-bold text-main">游戏配置</h3>
            <p className="text-xs text-muted">设置游戏安装路径以实现自动化功能</p>
          </div>
        </div>

        <div className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-center">
            <label className="text-sm font-medium text-muted">
              游戏安装目录
            </label>
            <div className="col-span-2">
              <input
                type="text"
                value={config.game.gameDirectory}
                onChange={(e) => handleConfigChange('game', 'gameDirectory', e.target.value)}
                className="w-full px-3 py-2 bg-base/50 border border-base rounded-lg text-main focus:bg-surface focus:ring-1 focus:ring-primary focus:border-primary transition-all placeholder:text-muted/50 text-sm"
                placeholder="输入剑网三游戏安装目录"
              />
              {pathValid === false && (
                <div className="flex items-center gap-1.5 text-xs text-red-500 mt-2">
                  <AlertTriangle className="w-3.5 h-3.5" />
                  <span>游戏目录路径无效</span>
                </div>
              )}

              {pathValid === true && (
                <div className="flex items-center gap-1.5 text-xs text-emerald-500 mt-2">
                  <Check className="w-3.5 h-3.5" />
                  <span>游戏目录路径有效</span>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};