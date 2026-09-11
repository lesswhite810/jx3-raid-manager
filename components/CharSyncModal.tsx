import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { createPortal } from 'react-dom';
import {
  RefreshCw,
  X,
  AlertTriangle,
  CheckCircle,
  Undo2,
  Info,
  History,
  Search,
  CheckSquare,
  Square,
  User as UserIcon,
  ChevronDown,
  ChevronRight,
} from 'lucide-react';
import {
  charSyncService,
  type RoleDirInfo,
  type SyncOptions,
  type SyncResult,
  type BackupEntry,
} from '../services/charSync';
import { getBaseServerName } from '../utils/serverUtils';
import { toast } from '../utils/toastManager';
import { SectIcon } from './SectIcon';
import type { Account } from '../types';

const DEFAULT_OPTIONS: SyncOptions = {
  syncAddonJx3dat: true,
  syncCustom: true,
  syncHotkey: true,
  syncUserprefs: true,
  overwrite: true,
};

type Tab = 'sync' | 'history';

export interface CharSyncModalProps {
  isOpen: boolean;
  onClose: () => void;
  gameDirectory: string;
  /** 源角色（从角色卡片入口进入时指定），按角色名+服务器匹配磁盘角色目录 */
  sourceRole: { name: string; server: string };
  /** 应用内账号数据（用于按角色名+服务器匹配心法，显示职业图标） */
  appAccounts: Account[];
}

interface BatchTargetResult {
  target: RoleDirInfo;
  result: SyncResult | null;
  error?: string;
}

export const CharSyncModal: React.FC<CharSyncModalProps> = ({
  isOpen,
  onClose,
  gameDirectory,
  sourceRole: sourceRoleSpec,
  appAccounts,
}) => {
  const [activeTab, setActiveTab] = useState<Tab>('sync');
  const [roles, setRoles] = useState<RoleDirInfo[]>([]);
  const [loadingRoles, setLoadingRoles] = useState(false);
  const [sourceRole, setSourceRole] = useState<RoleDirInfo | null>(null);
  const [targetRoles, setTargetRoles] = useState<RoleDirInfo[]>([]);
  const [targetSearch, setTargetSearch] = useState('');
  /** 目标账号展开状态（按账号名折叠，展开后显示该账号下角色） */
  const [expandedAccounts, setExpandedAccounts] = useState<Set<string>>(new Set());
  const [options, setOptions] = useState<SyncOptions>(DEFAULT_OPTIONS);
  const [syncing, setSyncing] = useState(false);
  const [batchResults, setBatchResults] = useState<BatchTargetResult[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [loadingBackups, setLoadingBackups] = useState(false);
  const [rolling, setRolling] = useState(false);
  /** 备份目录 → 回滚时间（后端 SQLite 持久化） */
  const [rollbackMarks, setRollbackMarks] = useState<Record<string, string>>({});
  /** 源角色是否已解析（避免刷新角色列表时重复套用） */
  const sourceResolvedRef = useRef(false);

  const resolveSourceRole = useCallback((list: RoleDirInfo[]) => {
    if (sourceResolvedRef.current) return;
    sourceResolvedRef.current = true;
    // 优先按角色名 + 服务器匹配，找不到再按角色名匹配（处理同名跨服角色）
    const match =
      list.find(
        (r) =>
          r.roleName === sourceRoleSpec.name &&
          getBaseServerName(r.server) === getBaseServerName(sourceRoleSpec.server),
      ) ?? list.find((r) => r.roleName === sourceRoleSpec.name);
    if (match) {
      setSourceRole(match);
    }
  }, [sourceRoleSpec]);

  const loadRoles = useCallback(async () => {
    if (!gameDirectory) return;
    setLoadingRoles(true);
    setError(null);
    try {
      const list = await charSyncService.listSyncableRoles(gameDirectory);
      setRoles(list);
      resolveSourceRole(list);
      // 账号默认收起，点击账号行再展开角色
      setExpandedAccounts(new Set());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingRoles(false);
    }
  }, [gameDirectory, resolveSourceRole]);

  const loadBackups = useCallback(async () => {
    setLoadingBackups(true);
    try {
      const list = await charSyncService.listBackups();
      setBackups(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingBackups(false);
    }
  }, []);

  // 打开弹窗时从后端 SQLite 加载回滚标记（V18 之前来自 localStorage，V18 起改为数据库存储）
  useEffect(() => {
    if (isOpen) {
      charSyncService.rollbackMarks
        .list()
        .then((marks) => setRollbackMarks(marks))
        .catch((e) => console.error('[CharSyncModal] 加载回滚标记失败:', e));
    }
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    // 每次打开重置解析标记，加载完角色列表后重新解析源角色
    sourceResolvedRef.current = false;
    // 角色列表两个页签都要用（备份历史页回滚需要按目录匹配角色），打开即加载
    loadRoles();
    if (activeTab === 'history') {
      loadBackups();
    }
  }, [isOpen, activeTab, loadRoles, loadBackups]);

  useEffect(() => {
    if (!isOpen) return;
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !syncing && !rolling) onClose();
    };
    window.addEventListener('keydown', handleEsc);
    return () => window.removeEventListener('keydown', handleEsc);
  }, [isOpen, onClose, syncing, rolling]);

  /** 角色名@基础服务器 → 心法名（用于显示职业图标；磁盘目录无职业信息，从应用内角色数据匹配） */
  const martialByRoleKey = useMemo(() => {
    const map = new Map<string, string>();
    for (const acc of appAccounts) {
      for (const r of acc.roles) {
        const sectName = r.martial || r.sect;
        if (!sectName) continue;
        const key = `${r.name}@${getBaseServerName(r.server)}`;
        if (!map.has(key)) {
          map.set(key, sectName);
        }
      }
    }
    return map;
  }, [appAccounts]);

  const getMartialOf = useCallback(
    (r: RoleDirInfo) => martialByRoleKey.get(`${r.roleName}@${getBaseServerName(r.server)}`),
    [martialByRoleKey],
  );

  const filteredTargetRoles = useMemo(() => {
    const candidates = roles.filter((r) => r.roleDir !== sourceRole?.roleDir);
    if (!targetSearch.trim()) return candidates;
    const kw = targetSearch.toLowerCase();
    return candidates.filter(
      (r) =>
        r.accountName.toLowerCase().includes(kw) ||
        r.server.toLowerCase().includes(kw) ||
        r.roleName.toLowerCase().includes(kw),
    );
  }, [roles, sourceRole, targetSearch]);

  /** 目标角色按账号分组（先选账号，再选该账号下角色） */
  const targetAccountGroups = useMemo(() => {
    const map = new Map<string, RoleDirInfo[]>();
    for (const r of filteredTargetRoles) {
      const arr = map.get(r.accountName);
      if (arr) {
        arr.push(r);
      } else {
        map.set(r.accountName, [r]);
      }
    }
    return Array.from(map.entries()).map(([accountName, accRoles]) => ({
      accountName,
      roles: accRoles,
    }));
  }, [filteredTargetRoles]);

  const toggleAccountExpanded = (accountName: string) => {
    setExpandedAccounts((prev) => {
      const next = new Set(prev);
      if (next.has(accountName)) {
        next.delete(accountName);
      } else {
        next.add(accountName);
      }
      return next;
    });
  };

  const canSync = sourceRole && targetRoles.length > 0 && !syncing;

  const toggleTargetRole = (role: RoleDirInfo) => {
    setBatchResults(null);
    setTargetRoles((prev) => {
      const exists = prev.some((r) => r.roleDir === role.roleDir);
      if (exists) {
        return prev.filter((r) => r.roleDir !== role.roleDir);
      }
      return [...prev, role];
    });
  };

  const selectAllVisibleTargets = () => {
    setBatchResults(null);
    setTargetRoles((prev) => {
      const existing = new Set(prev.map((r) => r.roleDir));
      const toAdd = filteredTargetRoles.filter((r) => !existing.has(r.roleDir));
      return [...prev, ...toAdd];
    });
  };

  const clearTargets = () => {
    setBatchResults(null);
    setTargetRoles([]);
  };

  const handleBatchSync = async () => {
    if (!sourceRole || targetRoles.length === 0) return;
    setSyncing(true);
    setError(null);
    setBatchResults(null);
    const results: BatchTargetResult[] = [];
    for (const target of targetRoles) {
      try {
        const res = await charSyncService.syncCharacterSettings(gameDirectory, sourceRole, target, options);
        results.push({ target, result: res });
      } catch (e) {
        results.push({ target, result: null, error: String(e) });
      }
    }
    setBatchResults(results);
    setSyncing(false);
  };

  const handleRollbackOne = async (backup: BackupEntry, targetDir: string) => {
    setRolling(true);
    setError(null);
    try {
      await charSyncService.rollback(gameDirectory, backup.backupDir, targetDir);
      const now = new Date().toISOString();
      // 同步更新本地 state，避免重新拉取一次
      setRollbackMarks((prev) => ({ ...prev, [backup.backupDir]: now }));
      // 标记仅用于界面提示，写入失败不影响已完成的回滚，单独捕获避免误报
      charSyncService.rollbackMarks
        .set(backup.backupDir, now)
        .catch((e) => console.error('[CharSyncModal] 写入回滚标记失败:', e));
      await loadBackups();
      toast.success(
        `已从 ${formatTimestamp(backup.timestamp)} 备份为「${backup.targetRole}」恢复 ${backup.fileCount} 个文件，请在游戏内登录角色确认效果`,
        4000,
      );
    } catch (e) {
      const msg = String(e);
      setError(msg);
      toast.error(`回滚失败：${msg}`);
    } finally {
      setRolling(false);
    }
  };

  const handleReset = () => {
    setBatchResults(null);
    setError(null);
    setTargetRoles([]);
    setOptions(DEFAULT_OPTIONS);
    setTargetSearch('');
  };

  if (!isOpen) return null;

  const roleLabel = (r: RoleDirInfo) => `${r.accountName} / ${r.server} / ${r.roleName}`;

  return createPortal(
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm"
      onClick={(e) => { if (e.target === e.currentTarget && !syncing && !rolling) onClose(); }}
    >
      <div
        className="bg-surface border border-base rounded-card shadow-ds-modal w-full max-w-[760px] h-[85vh] flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-6 py-4 flex items-center justify-between border-b border-base flex-shrink-0">
          <div className="flex items-center gap-2">
            <RefreshCw className="w-5 h-5 text-primary" />
            <h2 className="text-lg font-semibold text-main">跨角色设置同步</h2>
          </div>
          <button
            onClick={onClose}
            className="text-muted hover:text-main hover:bg-base p-1.5 rounded-lg transition-colors"
            aria-label="关闭"
            disabled={syncing || rolling}
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="flex border-b border-base flex-shrink-0 px-6">
          <button
            onClick={() => setActiveTab('sync')}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors flex items-center gap-1.5 ${
              activeTab === 'sync' ? 'border-primary text-primary' : 'border-transparent text-muted hover:text-main'
            }`}
          >
            <RefreshCw className="w-3.5 h-3.5" />
            同步设置
          </button>
          <button
            onClick={() => setActiveTab('history')}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors flex items-center gap-1.5 ${
              activeTab === 'history' ? 'border-primary text-primary' : 'border-transparent text-muted hover:text-main'
            }`}
          >
            <History className="w-3.5 h-3.5" />
            备份历史
            {backups.length > 0 && (
              <span className="ml-1 px-1.5 py-0.5 text-[10px] rounded-full bg-base text-muted">{backups.length}</span>
            )}
          </button>
        </div>

        <div className="flex-1 px-6 py-4 min-h-0 overflow-y-auto">
          {error && (
            <div className="mb-4 flex items-start gap-2 p-3 rounded-lg bg-red-50 border border-red-200 text-sm text-red-700">
              <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0" />
              <span>{error}</span>
            </div>
          )}

          {activeTab === 'sync' && !loadingRoles && !sourceRole && !error && (
            <div className="mb-4 flex items-start gap-2 p-3 rounded-lg bg-ds-warning-soft border border-ds-warning-soft text-sm text-ds-warning-strong">
              <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0" />
              <span>
                未在游戏目录中找到「{sourceRoleSpec.name}」的角色数据目录，
                请确认该角色已在游戏中登录过；如需同步请从其他角色进入。
              </span>
            </div>
          )}

          {activeTab === 'sync' ? (
            <SyncTabContent
              loading={loadingRoles}
              sourceRole={sourceRole}
              sourceSpec={sourceRoleSpec}
              targetRoles={targetRoles}
              toggleTargetRole={toggleTargetRole}
              selectAllVisibleTargets={selectAllVisibleTargets}
              clearTargets={clearTargets}
              targetSearch={targetSearch}
              setTargetSearch={setTargetSearch}
              targetAccountGroups={targetAccountGroups}
              expandedAccounts={expandedAccounts}
              toggleAccountExpanded={toggleAccountExpanded}
              options={options}
              setOptions={setOptions}
              syncing={syncing}
              batchResults={batchResults}
              canSync={Boolean(canSync)}
              roleLabel={roleLabel}
              getMartialOf={getMartialOf}
            />
          ) : (
            <HistoryTabContent
              backups={backups}
              loading={loadingBackups}
              roles={roles}
              gameDirectory={gameDirectory}
              rolling={rolling}
              onRollback={handleRollbackOne}
              onReload={loadBackups}
              rollbackMarks={rollbackMarks}
            />
          )}
        </div>

        {activeTab === 'sync' && (
          <div className="px-6 py-4 border-t border-base flex items-center justify-end gap-3 flex-shrink-0">
            {batchResults && batchResults.length > 0 && (
              <button onClick={handleReset} className="px-3 py-2 text-sm text-muted hover:text-main transition-colors" disabled={syncing || rolling}>继续同步其他角色</button>
            )}
            <button onClick={onClose} className="px-4 py-2 text-sm text-muted hover:text-main border border-base rounded-lg transition-colors" disabled={syncing || rolling}>{batchResults ? '关闭' : '取消'}</button>
            {!batchResults && (
              <button onClick={handleBatchSync} disabled={!canSync} className="px-4 py-2 text-sm font-medium text-white bg-ds-success rounded-lg hover:bg-ds-success transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1.5">
                {syncing ? (
                  <><RefreshCw className="w-4 h-4 animate-spin" />同步中…</>
                ) : (
                  <><RefreshCw className="w-4 h-4" />执行同步{targetRoles.length > 1 && <span className="ml-1 px-1.5 py-0.5 text-xs bg-white/20 rounded">{targetRoles.length}</span>}</>
                )}
              </button>
            )}
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
};

interface TargetAccountGroup {
  accountName: string;
  roles: RoleDirInfo[];
}

interface SyncTabContentProps {
  loading: boolean;
  sourceRole: RoleDirInfo | null;
  /** 入口指定的源角色信息（角色名 + 服务器），用于未匹配到磁盘目录时的展示 */
  sourceSpec: { name: string; server: string };
  targetRoles: RoleDirInfo[];
  toggleTargetRole: (r: RoleDirInfo) => void;
  selectAllVisibleTargets: () => void;
  clearTargets: () => void;
  targetSearch: string;
  setTargetSearch: (v: string) => void;
  targetAccountGroups: TargetAccountGroup[];
  expandedAccounts: Set<string>;
  toggleAccountExpanded: (accountName: string) => void;
  options: SyncOptions;
  setOptions: (o: SyncOptions) => void;
  syncing: boolean;
  batchResults: BatchTargetResult[] | null;
  canSync: boolean;
  roleLabel: (r: RoleDirInfo) => string;
  /** 按磁盘角色目录信息取应用内心法名（无匹配时返回 undefined，不显示图标） */
  getMartialOf: (r: RoleDirInfo) => string | undefined;
}

const SyncTabContent: React.FC<SyncTabContentProps> = ({
  loading, sourceRole, sourceSpec, targetRoles, toggleTargetRole,
  selectAllVisibleTargets, clearTargets,
  targetSearch, setTargetSearch, targetAccountGroups, expandedAccounts, toggleAccountExpanded,
  options, setOptions, syncing, batchResults, roleLabel, getMartialOf,
}) => {
  if (loading) {
    return <div className="text-center py-8 text-muted">正在扫描可同步角色…</div>;
  }
  if (!sourceRole) {
    return (
      <div className="text-center py-8 text-muted">
        未在游戏目录中找到「{sourceSpec.name}」的角色数据目录，
        请确认该角色已在游戏中登录过。
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <div>
        <label className="block text-sm font-medium text-main mb-1.5">源角色（复制设置来源）</label>
        <div className="flex items-center gap-2 px-3 py-2.5 bg-ds-success-soft border border-ds-success-soft rounded-lg text-sm text-ds-success-strong font-medium">
          <UserIcon className="w-4 h-4 flex-shrink-0 text-ds-success-strong" />
          {(() => {
            const martial = getMartialOf(sourceRole);
            return martial ? <SectIcon sectName={martial} size="xs" className="flex-shrink-0" /> : null;
          })()}
          <span className="truncate">{roleLabel(sourceRole)}</span>
        </div>
      </div>

      <div>
        <div className="flex items-center justify-between mb-1.5">
          <label className="text-sm font-medium text-main">目标角色（先选账号，再选角色，已选 {targetRoles.length} 个）</label>
          <div className="flex items-center gap-2 text-xs">
            <button onClick={selectAllVisibleTargets} disabled={syncing || !sourceRole}
              className="text-primary hover:underline disabled:opacity-50 disabled:cursor-not-allowed">全选可见</button>
            {targetRoles.length > 0 && (
              <button onClick={clearTargets} disabled={syncing} className="text-muted hover:underline disabled:opacity-50">清空</button>
            )}
          </div>
        </div>
        <div className="relative mb-2">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted pointer-events-none" />
          <input type="text" placeholder="搜索账号 / 服务器 / 角色名" value={targetSearch}
            onChange={(e) => setTargetSearch(e.target.value)} disabled={syncing}
            className="w-full pl-9 pr-3 py-2 bg-surface border border-base rounded-lg text-main text-sm focus:outline-none focus:ring-2 focus:ring-ds-success" />
        </div>
        <div className="border border-base rounded-lg divide-y divide-base">
          {targetAccountGroups.length === 0 ? (
            <div className="px-3 py-2 text-sm text-muted text-center">无匹配角色</div>
          ) : (
            targetAccountGroups.map((group) => {
              const expanded = expandedAccounts.has(group.accountName);
              const selectedCount = group.roles.filter((r) =>
                targetRoles.some((t) => t.roleDir === r.roleDir),
              ).length;
              return (
                <div key={group.accountName}>
                  <button
                    onClick={() => toggleAccountExpanded(group.accountName)}
                    disabled={syncing}
                    className="w-full px-3 py-2 flex items-center gap-2 text-sm text-main bg-base/50 hover:bg-base transition-colors"
                  >
                    {expanded ? (
                      <ChevronDown className="w-4 h-4 flex-shrink-0 text-muted" />
                    ) : (
                      <ChevronRight className="w-4 h-4 flex-shrink-0 text-muted" />
                    )}
                    <UserIcon className="w-4 h-4 flex-shrink-0 text-muted" />
                    <span className="font-medium truncate">{group.accountName}</span>
                    <span className="text-xs text-muted">
                      {group.roles.length} 个角色
                      {selectedCount > 0 && `，已选 ${selectedCount}`}
                    </span>
                  </button>
                  {expanded && (
                    <div className="divide-y divide-base">
                      {group.roles.map((r) => {
                        const selected = targetRoles.some((t) => t.roleDir === r.roleDir);
                        const martial = getMartialOf(r);
                        return (
                          <button key={r.roleDir} onClick={() => toggleTargetRole(r)} disabled={syncing}
                            className={`w-full px-3 py-2 pl-9 text-left text-sm flex items-center gap-2 transition-colors ${selected ? 'bg-ds-success-soft text-ds-success-strong font-medium' : 'hover:bg-base text-main'}`}>
                            {selected ? <CheckSquare className="w-4 h-4 flex-shrink-0 text-ds-success-strong" /> : <Square className="w-4 h-4 flex-shrink-0 text-muted" />}
                            {martial && <SectIcon sectName={martial} size="xs" className="flex-shrink-0" />}
                            <span className="truncate">{r.server} / {r.roleName}</span>
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>

      <div>
        <label className="block text-sm font-medium text-main mb-2">同步内容</label>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
          <OptionCheckbox label="插件启用状态" hint="addon.jx3dat" checked={options.syncAddonJx3dat} disabled={syncing}
            onChange={(v) => setOptions({ ...options, syncAddonJx3dat: v })} />
          <OptionCheckbox label="聊天自定义" hint="custom.dat + .addon" checked={options.syncCustom} disabled={syncing}
            onChange={(v) => setOptions({ ...options, syncCustom: v })} />
          <OptionCheckbox label="热键" hint="hotkey.data + hotkey_*.txt" checked={options.syncHotkey} disabled={syncing}
            onChange={(v) => setOptions({ ...options, syncHotkey: v })} />
          <OptionCheckbox label="UI 设置 + 技能栏" hint="userpreferences.jx3dat" checked={options.syncUserprefs} disabled={syncing}
            onChange={(v) => setOptions({ ...options, syncUserprefs: v })} />
        </div>
        <label className="mt-2 flex items-center gap-2 text-sm text-main cursor-pointer">
          <input type="checkbox" className="rounded border-base text-ds-success-strong focus:ring-ds-success"
            checked={options.overwrite} onChange={(e) => setOptions({ ...options, overwrite: e.target.checked })} disabled={syncing} />
          <span>覆盖目标已有文件</span>
        </label>
        <div className="mt-2 flex items-start gap-1.5 text-xs text-muted">
          <Info className="w-3.5 h-3.5 mt-0.5 flex-shrink-0" />
          <span>服务器同步标志位 (userpreferencesasync.jx3dat) 始终不复制——请在游戏内手动关闭。</span>
        </div>
      </div>

      {sourceRole && targetRoles.length > 0 && !batchResults && (
        <div className="flex items-start gap-2 p-3 rounded-lg bg-ds-warning-soft border border-ds-warning-soft text-xs text-ds-warning-strong">
          <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0" />
          <div className="space-y-1">
            <div>将覆盖 {targetRoles.length} 个目标角色的现有设置，已自动备份可回滚。</div>
            <div>执行前请确认：目标角色已在游戏内「系统设置 → 综合 → 服务器同步设置」关闭全部勾选，否则本地设置会被服务器回写覆盖。</div>
          </div>
        </div>
      )}

      {batchResults && batchResults.length > 0 && (
        <div className="space-y-2">
          <div className="text-sm font-medium text-main">
            同步结果（{batchResults.filter((r) => r.result?.success).length}/{batchResults.length} 成功）
          </div>
          {batchResults.map(({ target, result, error: err }) => (
            <BatchResultRow key={target.roleDir} target={target} result={result} error={err} roleLabel={roleLabel} />
          ))}
        </div>
      )}
    </div>
  );
};

interface BatchResultRowProps {
  target: RoleDirInfo;
  result: SyncResult | null;
  error?: string;
  roleLabel: (r: RoleDirInfo) => string;
}

const BatchResultRow: React.FC<BatchResultRowProps> = ({ target, result, error, roleLabel }) => {
  const [expanded, setExpanded] = useState(false);
  const success = result?.success && !error;
  return (
    <div className="p-3 rounded-lg border border-base bg-base/30 space-y-2">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-sm min-w-0">
          {success ? <CheckCircle className="w-4 h-4 text-ds-success-strong flex-shrink-0" /> : <AlertTriangle className="w-4 h-4 text-ds-warning-strong flex-shrink-0" />}
          <span className="font-medium text-main truncate">{roleLabel(target)}</span>
          {error && <span className="text-xs text-red-600 truncate">· {error}</span>}
        </div>
        <div className="flex items-center gap-2 flex-shrink-0">
          <span className="text-xs text-muted">{result ? `复制 ${result.copiedFiles.length}${result.skippedFiles.length > 0 ? ` / 跳过 ${result.skippedFiles.length}` : ''}` : '—'}</span>
          {result?.backupDir && (
            <button onClick={() => setExpanded(!expanded)} className="text-xs text-primary hover:underline">{expanded ? '收起' : '详情'}</button>
          )}
        </div>
      </div>
      {expanded && result && (
        <div className="text-xs text-muted space-y-1 pl-6">
          {result.copiedFiles.length > 0 && (
            <div>
              <div className="font-medium text-main mb-0.5">已复制：</div>
              <ul className="space-y-0.5">{result.copiedFiles.map((f) => <li key={f}>· {f}</li>)}</ul>
            </div>
          )}
          {result.skippedFiles.length > 0 && (
            <div>
              <div className="font-medium text-main mb-0.5">已跳过：</div>
              <ul className="space-y-0.5">{result.skippedFiles.map((f) => <li key={f}>· {f}</li>)}</ul>
            </div>
          )}
          {result.backupDir && <div className="break-all text-[11px]">备份: {result.backupDir}</div>}
          {result.error && <div className="text-red-600">错误: {result.error}</div>}
        </div>
      )}
    </div>
  );
};

interface OptionCheckboxProps {
  label: string;
  hint: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (v: boolean) => void;
}

const OptionCheckbox: React.FC<OptionCheckboxProps> = ({ label, hint, checked, disabled, onChange }) => (
  <label className="flex items-start gap-2 px-3 py-2 border border-base rounded-lg cursor-pointer hover:border-ds-success transition-colors">
    <input type="checkbox" className="mt-0.5 rounded border-base text-ds-success-strong focus:ring-ds-success"
      checked={checked} onChange={(e) => onChange(e.target.checked)} disabled={disabled} />
    <div className="min-w-0">
      <div className="text-sm text-main">{label}</div>
      <div className="text-xs text-muted truncate">{hint}</div>
    </div>
  </label>
);

interface HistoryTabContentProps {
  backups: BackupEntry[];
  loading: boolean;
  roles: RoleDirInfo[];
  gameDirectory: string;
  rolling: boolean;
  onRollback: (backup: BackupEntry, targetDir: string) => Promise<void>;
  onReload: () => Promise<void>;
  /** key: 备份目录；value: 回滚时间 ISO（后端 SQLite 持久化） */
  rollbackMarks: Record<string, string>;
}

/** 判断目录是否位于当前游戏目录下（忽略大小写与分隔符差异） */
function isUnderGameDir(dir: string, gameDirectory: string): boolean {
  const norm = (p: string) => {
    const t = p.replace(/\//g, '\\').toLowerCase();
    return t.endsWith('\\') ? t : t + '\\';
  };
  return norm(dir).startsWith(norm(gameDirectory));
}

const HistoryTabContent: React.FC<HistoryTabContentProps> = ({
  backups, loading, roles, gameDirectory, rolling, onRollback, onReload, rollbackMarks,
}) => {
  const [confirming, setConfirming] = useState<string | null>(null);
  if (loading) {
    return <div className="text-center py-8 text-muted">正在加载备份历史…</div>;
  }
  if (backups.length === 0) {
    return (
      <div className="text-center py-8 text-muted">
        暂无备份记录。每次同步前会自动备份目标角色设置。
        <button onClick={onReload} className="block mx-auto mt-3 text-xs text-primary hover:underline">刷新</button>
      </div>
    );
  }
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs text-muted">共 {backups.length} 条备份，按时间倒序</span>
        <button onClick={onReload} className="text-xs text-primary hover:underline">刷新</button>
      </div>
      {backups.map((b) => {
        // 优先按备份元数据中的精确角色目录匹配，避免跨账号/服务器同名角色误匹配；
        // 旧备份无元数据时降级按角色名匹配
        const matchedRole = roles.find((r) =>
          b.targetRoleDir ? r.roleDir === b.targetRoleDir : r.roleName === b.targetRole,
        );
        const targetDir =
          matchedRole?.roleDir ??
          (b.targetRoleDir && isUnderGameDir(b.targetRoleDir, gameDirectory) ? b.targetRoleDir : null);
        const isConfirming = confirming === b.backupDir;
        const rollbackAt = rollbackMarks[b.backupDir];
        return (
          <div key={b.backupDir}
            className={`p-3 rounded-lg border space-y-2 transition-colors ${
              rollbackAt ? 'border-ds-success-soft bg-ds-success-soft' : 'border-base bg-base/30'
            }`}>
            <div className="flex items-center gap-2 text-sm min-w-0 flex-wrap">
              <History className={`w-4 h-4 flex-shrink-0 ${rollbackAt ? 'text-ds-success-strong' : 'text-muted'}`} />
              <span className="font-medium text-main truncate">{b.targetRole}</span>
              <span className="text-xs text-muted">· {formatTimestamp(b.timestamp)} · {b.fileCount} 文件</span>
              {rollbackAt && (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium text-white bg-ds-success">
                  <CheckCircle className="w-3 h-3" />
                  已回滚到此版本 · {new Date(rollbackAt).toLocaleString('zh-CN', { hour12: false })}
                </span>
              )}
            </div>
            <div className="text-xs text-muted break-all pl-6">{b.backupDir}</div>
            <div className="pl-6 flex items-center gap-2 flex-wrap">
              {targetDir ? (
                isConfirming ? (
                  <>
                    <span className="text-xs text-ds-warning-strong">确认回滚到「{b.targetRole}」？会覆盖该角色当前设置</span>
                    <button onClick={async () => { await onRollback(b, targetDir); setConfirming(null); }}
                      disabled={rolling}
                      className="px-2 py-1 text-xs font-medium text-white bg-red-600 rounded hover:bg-red-700 transition-colors disabled:opacity-50">
                      {rolling ? '回滚中…' : '确认'}
                    </button>
                    <button onClick={() => setConfirming(null)} disabled={rolling}
                      className="px-2 py-1 text-xs text-muted hover:text-main transition-colors">取消</button>
                  </>
                ) : (
                  <button onClick={() => setConfirming(b.backupDir)} disabled={rolling}
                    className="flex items-center gap-1 px-2 py-1 text-xs font-medium text-red-600 border border-red-300 rounded hover:bg-red-50 transition-colors disabled:opacity-50">
                    <Undo2 className="w-3 h-3" />{rollbackAt ? '再次回滚到此版本' : '回滚到此角色'}
                  </button>
                )
              ) : (
                <span className="text-xs text-ds-warning-strong">当前游戏目录中未找到同名角色「{b.targetRole}」，无法回滚</span>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
};

function formatTimestamp(ts: string): string {
  if (ts.length !== 15 || ts[8] !== '-') return ts;
  const date = `${ts.slice(0, 4)}-${ts.slice(4, 6)}-${ts.slice(6, 8)}`;
  const time = `${ts.slice(9, 11)}:${ts.slice(11, 13)}:${ts.slice(13, 15)}`;
  return `${date} ${time}`;
}

