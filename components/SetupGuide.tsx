import React, { useCallback, useEffect, useState } from 'react';
import { FolderOpen, Search, Check, Loader2, ChevronRight, Shield, Download } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { useAppConfig } from '../contexts/AppConfigContext';
import { scanJx3Clients, Jx3ClientInfo } from '../services/gameDirectoryScanner';
import { isValidGamePath } from '../utils/configUtils';
import { db } from '../services/db';
import { toast } from '../utils/toastManager';
import { ImportRolesModal } from './ImportRolesModal';

interface StepStatus {
  state: 'idle' | 'loading' | 'success' | 'error';
  message?: string;
}

export const SetupGuide: React.FC = () => {
  const { appConfig, updateGameDirectory, updateAccountIds, markSetupCompleted } = useAppConfig();

  const [gameDirectory, setGameDirectory] = useState<string>(appConfig?.gameDirectory ?? '');
  const [pathValidation, setPathValidation] = useState<'idle' | 'checking' | 'valid' | 'invalid'>('idle');

  const [scanStatus, setScanStatus] = useState<StepStatus>({ state: 'idle' });
  const [scanResult, setScanResult] = useState<{ accounts: number; roles: number } | null>(null);
  const [scanningClients, setScanningClients] = useState(false);
  const [clientResults, setClientResults] = useState<Jx3ClientInfo[]>([]);
  const [showClientResults, setShowClientResults] = useState(false);

  const [isImportRolesModalOpen, setIsImportRolesModalOpen] = useState(false);
  const [completing, setCompleting] = useState(false);

  // 初始化时使用 appConfig 中的目录
  useEffect(() => {
    if (appConfig?.gameDirectory && !gameDirectory) {
      setGameDirectory(appConfig.gameDirectory);
    }
  }, [appConfig, gameDirectory]);

  // 校验游戏目录
  useEffect(() => {
    if (!gameDirectory.trim()) {
      setPathValidation('idle');
      return;
    }
    setPathValidation('checking');
    let cancelled = false;
    isValidGamePath(gameDirectory).then(result => {
      if (!cancelled) {
        setPathValidation(result.isValid ? 'valid' : 'invalid');
      }
    });
    return () => {
      cancelled = true;
    };
  }, [gameDirectory]);

  const handleBrowse = useCallback(async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择剑网3游戏目录',
      });
      if (typeof selected === 'string') {
        setGameDirectory(selected);
      }
    } catch (error) {
      console.error('选择目录失败:', error);
      toast.error('选择目录失败');
    }
  }, []);

  const handleScanClients = useCallback(async () => {
    setScanningClients(true);
    setShowClientResults(false);
    try {
      const result = await scanJx3Clients();
      if (result.success && result.clients.length > 0) {
        setClientResults(result.clients);
        setShowClientResults(true);
        if (result.clients.length === 1 && !gameDirectory) {
          const client = result.clients[0];
          setGameDirectory(client.workDirectory);
          toast.success(`已自动填入 ${client.displayName} 的安装目录`);
        } else if (result.clients.length > 1) {
          toast.info(`检测到 ${result.clients.length} 个客户端，请选择`);
        }
      } else {
        toast.error(result.error || '未检测到剑网3客户端，请确认游戏已安装');
      }
    } catch (error) {
      console.error('扫描客户端失败:', error);
      toast.error('扫描失败: ' + (error instanceof Error ? error.message : String(error)));
    } finally {
      setScanningClients(false);
    }
  }, [gameDirectory]);

  const handleSelectClient = useCallback((client: Jx3ClientInfo) => {
    setGameDirectory(client.workDirectory);
    setShowClientResults(false);
    toast.success(`已选择 ${client.displayName}`);
  }, []);

  // 保存游戏目录到 app_config 表
  const handleSaveDirectory = useCallback(async () => {
    if (pathValidation !== 'valid') {
      toast.warning('请先填写有效的游戏目录');
      return;
    }
    try {
      await updateGameDirectory(gameDirectory);
      toast.success('游戏目录已保存');
    } catch (error) {
      console.error('保存游戏目录失败:', error);
    }
  }, [gameDirectory, pathValidation, updateGameDirectory]);

  // 打开导入本地角色弹窗（与账号管理一致）
  const handleScanRoles = useCallback(async () => {
    const dir = appConfig?.gameDirectory ?? gameDirectory;
    if (!dir) {
      toast.warning('请先设置游戏目录');
      return;
    }
    // 确保 app_config 已写入最新的目录
    if (appConfig?.gameDirectory !== dir) {
      try {
        await updateGameDirectory(dir);
      } catch {
        return;
      }
    }
    setIsImportRolesModalOpen(true);
  }, [appConfig, gameDirectory, updateGameDirectory]);

  // 导入完成后回调：刷新账号列表与 app_config
  const handleImportedRoles = useCallback(async () => {
    try {
      const accounts = await db.getAccounts();
      const accountIds = accounts
        .map(a => a.id)
        .filter((id): id is string => typeof id === 'string' && id.length > 0);
      await updateAccountIds(accountIds);

      // 统计账号与角色总数用于展示
      let roleCount = 0;
      for (const acc of accounts) {
        const roles = await db.getRolesByAccount(acc.id);
        roleCount += roles.length;
      }
      setScanResult({ accounts: accounts.length, roles: roleCount });
      setScanStatus({ state: 'success' });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setScanStatus({ state: 'error', message });
      toast.error('刷新账号列表失败: ' + message);
    }
  }, [updateAccountIds]);

  const handleComplete = useCallback(async () => {
    setCompleting(true);
    try {
      await markSetupCompleted();
      // App.tsx 会监听 appConfig.setupCompleted 变化，自动切换主界面
    } catch (error) {
      console.error('完成引导失败:', error);
    } finally {
      setCompleting(false);
    }
  }, [markSetupCompleted]);

  const directorySaved = (appConfig?.gameDirectory ?? '') === gameDirectory && pathValidation === 'valid';
  const canComplete = directorySaved && scanStatus.state === 'success';

  return (
    <div className="min-h-screen bg-base flex items-center justify-center p-4">
      <div className="w-full max-w-2xl bg-surface rounded-xl border border-base shadow-sm overflow-hidden">
        {/* 顶部标题区 */}
        <div className="px-8 pt-8 pb-6 border-b border-base">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 bg-primary rounded-lg flex items-center justify-center text-primary-foreground font-bold text-lg">
              剑
            </div>
            <h1 className="text-2xl font-bold text-main">欢迎使用副本管家</h1>
          </div>
          <p className="text-sm text-muted">请按以下步骤完成初始配置，配置完成后可随时在"配置"页面修改。</p>
        </div>

        {/* 步骤区 */}
        <div className="px-8 py-6 space-y-6">
          {/* 步骤 1：游戏目录 */}
          <section>
            <div className="flex items-center gap-2 mb-3">
              <StepBadge number={1} active={!directorySaved} done={directorySaved} />
              <h2 className="text-base font-semibold text-main">设置游戏目录</h2>
            </div>
            <div className="space-y-2">
              <div className="flex gap-2">
                <input
                  type="text"
                  value={gameDirectory}
                  onChange={e => setGameDirectory(e.target.value)}
                  placeholder="例如：E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd"
                  className="flex-1 px-3 py-2 text-sm bg-base border border-base rounded-lg text-main placeholder:text-muted focus:outline-none focus:border-primary"
                />
                <button
                  onClick={handleBrowse}
                  className="px-3 py-2 text-sm bg-base border border-base rounded-lg text-main hover:bg-surface transition-colors flex items-center gap-1"
                  title="浏览"
                >
                  <FolderOpen size={16} />
                  <span className="hidden sm:inline">浏览</span>
                </button>
                <button
                  onClick={handleScanClients}
                  disabled={scanningClients}
                  className="px-3 py-2 text-sm bg-base border border-base rounded-lg text-main hover:bg-surface transition-colors flex items-center gap-1 disabled:opacity-50"
                  title="扫描已安装客户端"
                >
                  {scanningClients ? <Loader2 size={16} className="animate-spin" /> : <Search size={16} />}
                  <span className="hidden sm:inline">扫描客户端</span>
                </button>
              </div>

              {/* 路径校验状态 */}
              {pathValidation === 'checking' && (
                <div className="text-xs text-muted flex items-center gap-1">
                  <Loader2 size={12} className="animate-spin" />
                  正在校验目录...
                </div>
              )}
              {pathValidation === 'valid' && (
                <div className="text-xs text-emerald-600 flex items-center gap-1">
                  <Check size={12} />
                  游戏目录有效
                </div>
              )}
              {pathValidation === 'invalid' && (
                <div className="text-xs text-amber-600">目录无效，请确认路径包含 SeasunGame\Game\JX3\bin\zhcn_hd</div>
              )}

              {/* 多客户端结果 */}
              {showClientResults && clientResults.length > 0 && (
                <div className="mt-2 p-3 bg-base border border-base rounded-lg space-y-1">
                  <div className="text-xs text-muted mb-2">检测到 {clientResults.length} 个客户端，请选择：</div>
                  {clientResults.map(client => (
                    <button
                      key={client.clientType}
                      onClick={() => handleSelectClient(client)}
                      className="w-full text-left px-3 py-2 text-sm bg-surface border border-base rounded hover:border-primary transition-colors"
                    >
                      <div className="font-medium text-main">{client.displayName}</div>
                      <div className="text-xs text-muted truncate">{client.workDirectory}</div>
                    </button>
                  ))}
                </div>
              )}

              {/* 保存按钮 */}
              {pathValidation === 'valid' && !directorySaved && (
                <button
                  onClick={handleSaveDirectory}
                  className="px-4 py-2 text-sm bg-primary text-primary-text rounded-lg hover:bg-primary-hover transition-colors flex items-center gap-1"
                >
                  <Check size={14} />
                  保存游戏目录
                </button>
              )}
              {directorySaved && (
                <div className="text-xs text-emerald-600 flex items-center gap-1">
                  <Check size={12} />
                  游戏目录已保存
                </div>
              )}
            </div>
          </section>

          {/* 步骤 2：导入本地角色 */}
          <section>
            <div className="flex items-center gap-2 mb-3">
              <StepBadge number={2} active={directorySaved && scanStatus.state !== 'success'} done={scanStatus.state === 'success'} />
              <h2 className="text-base font-semibold text-main">导入本地角色</h2>
            </div>
            <div className="space-y-2">
              <button
                onClick={handleScanRoles}
                disabled={!directorySaved}
                className="px-4 py-2 text-sm bg-primary text-primary-text rounded-lg hover:bg-primary-hover transition-colors flex items-center gap-1 disabled:opacity-50"
              >
                <Download size={14} />
                导入本地角色
              </button>

              {scanStatus.state === 'success' && scanResult && (
                <div className="text-xs text-emerald-600 flex items-center gap-1">
                  <Check size={12} />
                  导入完成：当前共 {scanResult.accounts} 个账号、{scanResult.roles} 个角色
                </div>
              )}
              {scanStatus.state === 'error' && (
                <div className="text-xs text-amber-600">{scanStatus.message ?? '导入失败'}</div>
              )}
              <p className="text-xs text-muted">
                将读取茗伊插件目录下的角色信息供你选择导入，不会修改游戏文件。
              </p>
            </div>
          </section>

          {/* 步骤 3：完成 */}
          <section>
            <div className="flex items-center gap-2 mb-3">
              <StepBadge number={3} active={canComplete} done={false} />
              <h2 className="text-base font-semibold text-main">完成</h2>
            </div>
            <button
              onClick={handleComplete}
              disabled={!canComplete || completing}
              className="px-4 py-2 text-sm bg-primary text-primary-text rounded-lg hover:bg-primary-hover transition-colors flex items-center gap-1 disabled:opacity-50"
            >
              {completing ? <Loader2 size={14} className="animate-spin" /> : <ChevronRight size={14} />}
              进入应用
            </button>
            {!canComplete && (
              <p className="text-xs text-muted mt-2">请先完成前两步</p>
            )}
          </section>
        </div>

        {/* 底部说明 */}
        <div className="px-8 py-4 bg-base border-t border-base">
          <div className="flex items-start gap-2 text-xs text-muted">
            <Shield size={14} className="mt-0.5 flex-shrink-0" />
            <span>所有数据均存储在本地，不会上传到服务器。后续可在"配置"页面修改游戏目录或重新初始化。</span>
          </div>
        </div>
      </div>
      <ImportRolesModal
        isOpen={isImportRolesModalOpen}
        onClose={() => setIsImportRolesModalOpen(false)}
        gameDirectory={appConfig?.gameDirectory ?? gameDirectory}
        onImported={handleImportedRoles}
      />
    </div>
  );
};

const StepBadge: React.FC<{ number: number; active: boolean; done: boolean }> = ({ number, active, done }) => {
  if (done) {
    return (
      <span className="w-6 h-6 rounded-full bg-emerald-500 text-white text-xs font-bold flex items-center justify-center">
        <Check size={12} />
      </span>
    );
  }
  return (
    <span
      className={`w-6 h-6 rounded-full text-xs font-bold flex items-center justify-center ${
        active ? 'bg-primary text-primary-text' : 'bg-base text-muted border border-base'
      }`}
    >
      {number}
    </span>
  );
};
