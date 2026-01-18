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
        <h2 className="text-2xl font-bold text-slate-800">系统配置</h2>
        <div className="flex gap-2">
          <button
            onClick={handleSaveConfig}
            className="bg-indigo-600 hover:bg-indigo-700 text-white px-4 py-2 rounded-lg flex items-center gap-2 transition-colors shadow-sm hover:shadow-md disabled:opacity-70 disabled:cursor-not-allowed"
            disabled={isSaving}
          >
            {isSaving ? (
              <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
            ) : (
              <Save className="w-4 h-4" />
            )}
            {isSaving ? '保存中...' : '保存配置'}
          </button>
        </div>
      </div>

      {saveSuccess && (
        <div className="bg-green-50 border border-green-200 rounded-lg p-4 flex items-center gap-2 animate-pulse">
          <Check className="w-5 h-5 text-green-600" />
          <p className="text-sm text-green-700">配置保存成功！</p>
        </div>
      )}

      <div className="bg-white p-6 rounded-xl shadow-sm border border-slate-200">
        <div className="flex items-center gap-2 mb-4">
          <div className="w-10 h-10 bg-purple-100 text-purple-700 rounded-lg flex items-center justify-center">
            <Zap className="w-5 h-5" />
          </div>
          <h3 className="text-lg font-semibold text-slate-800">AI功能配置</h3>
        </div>

        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <h4 className="text-sm font-medium text-slate-700">启用AI功能</h4>
              <p className="text-xs text-slate-500 mt-0.5">开启后可使用AI辅助分析</p>
            </div>
            <label className="relative inline-flex items-center cursor-pointer">
              <input
                type="checkbox"
                checked={config.ai.enabled}
                onChange={(e) => handleConfigChange('ai', 'enabled', e.target.checked)}
                className="sr-only peer"
              />
              <div className="w-11 h-6 bg-slate-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-purple-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-slate-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-purple-600"></div>
            </label>
          </div>

          <div className="grid grid-cols-3 gap-3">
            <label className="flex items-center text-slate-600 font-medium">
              API密钥
            </label>
            <div className="col-span-2">
              <input
                type="password"
                value={config.ai.apiKey}
                onChange={(e) => handleConfigChange('ai', 'apiKey', e.target.value)}
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-transparent transition-all"
                placeholder="输入AI API密钥"
                disabled={!config.ai.enabled}
              />
            </div>
          </div>

          <div className="grid grid-cols-3 gap-3">
            <label className="flex items-center text-slate-600 font-medium">
              AI模型
            </label>
            <div className="col-span-2">
              <select
                value={config.ai.model}
                onChange={(e) => handleConfigChange('ai', 'model', e.target.value)}
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-transparent transition-all"
                disabled={!config.ai.enabled}
              >
                {aiService.getModels().map((model: any) => (
                  <option key={model.id} value={model.id}>{model.name}</option>
                ))}
              </select>
              {selectedModel && (
                <p className="text-xs text-slate-500 mt-1">
                  提供商: {selectedModel.provider}
                  {selectedModel.requiresProxy && (
                    <span className="text-amber-600 ml-2">⚠ 需要代理</span>
                  )}
                </p>
              )}
            </div>
          </div>

          <div className="grid grid-cols-3 gap-3">
            <label className="flex items-center text-slate-600 font-medium">
              生成温度
            </label>
            <div className="col-span-2">
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.1"
                  value={config.ai.temperature}
                  onChange={(e) => handleConfigChange('ai', 'temperature', parseFloat(e.target.value))}
                  className="w-full h-2 bg-slate-200 rounded-lg appearance-none cursor-pointer accent-purple-600"
                  disabled={!config.ai.enabled}
                />
                <span className="text-sm font-medium text-slate-700 w-12">{config.ai.temperature.toFixed(1)}</span>
              </div>
            </div>
          </div>

          {selectedModel?.requiresProxy && (
            <div className="bg-amber-50 border border-amber-200 rounded-lg p-3 flex items-center gap-2">
              <AlertTriangle className="w-4 h-4 text-amber-600" />
              <p className="text-sm text-amber-700">
                {selectedModel.name}需要代理才能访问，请配置下方代理设置
              </p>
            </div>
          )}

          <div className="border-t border-slate-200 pt-4 mt-4">
            <div className="flex items-center gap-2 mb-3">
              <Network className="w-4 h-4 text-slate-600" />
              <h4 className="text-sm font-medium text-slate-700">代理设置</h4>
            </div>

            <div className="flex items-center justify-between mb-3">
              <div>
                <h5 className="text-xs font-medium text-slate-600">启用代理</h5>
                <p className="text-xs text-slate-500">用于访问需要代理的AI模型</p>
              </div>
              <label className="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  checked={config.ai.proxyEnabled}
                  onChange={(e) => handleConfigChange('ai', 'proxyEnabled', e.target.checked)}
                  className="sr-only peer"
                />
                <div className="w-11 h-6 bg-slate-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-slate-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
              </label>
            </div>

            {config.ai.proxyEnabled && (
              <div className="grid grid-cols-3 gap-3">
                <label className="flex items-center text-slate-600 font-medium">
                  代理地址
                </label>
                <div className="col-span-2">
                  <input
                    type="text"
                    value={config.ai.proxyUrl}
                    onChange={(e) => handleConfigChange('ai', 'proxyUrl', e.target.value)}
                    className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all"
                    placeholder="http://127.0.0.1:7890"
                  />
                  <p className="text-xs text-slate-500 mt-1">
                    支持HTTP/HTTPS/SOCKS代理，格式: 协议://地址:端口
                  </p>
                </div>
              </div>
            )}
          </div>

          <div className="flex items-center gap-2 pt-2">
            <button
              onClick={handleTestAIConnection}
              disabled={!config.ai.enabled || testingAI}
              className="bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded-lg flex items-center gap-2 transition-colors text-sm disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {testingAI ? (
                <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
              ) : (
                <Settings className="w-4 h-4" />
              )}
              {testingAI ? '测试中...' : '测试连接'}
            </button>
            {testResult && (
              <span className={`text-sm ${testResult.success ? 'text-green-600' : 'text-red-600'}`}>
                {testResult.message}
              </span>
            )}
          </div>
        </div>
      </div>

      <div className="bg-white p-6 rounded-xl shadow-sm border border-slate-200">
        <div className="flex items-center gap-2 mb-4">
          <div className="w-10 h-10 bg-green-100 text-green-700 rounded-lg flex items-center justify-center">
            <FolderOpen className="w-5 h-5" />
          </div>
          <h3 className="text-lg font-semibold text-slate-800">游戏目录配置</h3>
        </div>

        <div className="space-y-4">
          <div className="grid grid-cols-3 gap-3">
            <label className="flex items-center text-slate-600 font-medium">
              游戏安装目录
            </label>
            <div className="col-span-2">
              <input
                type="text"
                value={config.game.gameDirectory}
                onChange={(e) => handleConfigChange('game', 'gameDirectory', e.target.value)}
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-green-500 focus:border-transparent transition-all"
                placeholder="输入剑网三游戏安装目录"
              />
            </div>
          </div>

          {pathValid === false && (
            <div className="flex items-center gap-1 text-xs text-red-500 mt-1">
              <AlertTriangle className="w-3 h-3" />
              <span>游戏目录路径无效</span>
            </div>
          )}

          {pathValid === true && (
            <div className="flex items-center gap-1 text-xs text-green-500 mt-1">
              <Check className="w-3 h-3" />
              <span>游戏目录路径有效</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};