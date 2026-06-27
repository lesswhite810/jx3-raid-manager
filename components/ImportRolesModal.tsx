import React, { useEffect, useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import { invoke } from '@tauri-apps/api/core';
import { X, Loader2, AlertCircle, User, Download, Check, ChevronDown, ChevronRight } from 'lucide-react';
import { toast } from '../utils/toastManager';
import { getBaseServerName } from '../utils/serverUtils';
import { SectIcon } from './SectIcon';

interface PreviewableRole {
  name: string;
  region: string;
  server: string;
  forceName?: string | null;
  kungfuName?: string | null;
  equipmentScore: number;
  alreadyImported: boolean;
}

interface PreviewableAccount {
  accountName: string;
  roles: PreviewableRole[];
}

interface PreviewResult {
  success: boolean;
  accounts: PreviewableAccount[];
  error?: string | null;
}

interface ImportResult {
  success: boolean;
  newAccounts: number;
  updatedAccounts: number;
  newRoles: number;
  updatedRoles: number;
  error?: string | null;
}

interface ImportRoleItem {
  accountName: string;
  roleName: string;
  region: string;
  server: string;
}

interface ImportRolesModalProps {
  isOpen: boolean;
  onClose: () => void;
  gameDirectory: string;
  onImported: () => void;
}

// 角色唯一键（用于选中集合）
const buildRoleKey = (accountName: string, role: PreviewableRole) =>
  `${accountName}|${role.name}|${role.region}|${role.server}`;

export const ImportRolesModal: React.FC<ImportRolesModalProps> = ({
  isOpen,
  onClose,
  gameDirectory,
  onImported,
}) => {
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [accounts, setAccounts] = useState<PreviewableAccount[]>([]);
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());
  const [expandedAccounts, setExpandedAccounts] = useState<Set<string>>(new Set());

  // 打开时拉取预览数据
  useEffect(() => {
    if (!isOpen || !gameDirectory) return;

    setLoading(true);
    setError(null);
    setAccounts([]);
    setSelectedKeys(new Set());
    setExpandedAccounts(new Set());

    invoke<PreviewResult>('preview_importable_roles', { gameDirectory })
      .then(result => {
        if (result.success) {
          setAccounts(result.accounts);
        } else {
          setError(result.error || '扫描失败');
        }
      })
      .catch(err => {
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        setLoading(false);
      });
  }, [isOpen, gameDirectory]);

  // 统计信息
  const stats = useMemo(() => {
    let totalRoles = 0;
    let importedCount = 0;
    accounts.forEach(account => {
      account.roles.forEach(role => {
        totalRoles++;
        if (role.alreadyImported) importedCount++;
      });
    });
    return {
      totalRoles,
      importedCount,
      importableCount: totalRoles - importedCount,
    };
  }, [accounts]);

  // 当前选中数量
  const selectedCount = selectedKeys.size;

  // 切换单个角色选中状态
  const toggleRole = (key: string) => {
    setSelectedKeys(prev => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  // 切换账号展开/收起
  const toggleAccountExpansion = (accountName: string) => {
    setExpandedAccounts(prev => {
      const next = new Set(prev);
      if (next.has(accountName)) {
        next.delete(accountName);
      } else {
        next.add(accountName);
      }
      return next;
    });
  };

  // 判断账号是否所有角色都已导入（用于禁用账号级全选）
  const isAccountAllImported = (account: PreviewableAccount) =>
    account.roles.length > 0 && account.roles.every(role => role.alreadyImported);

  // 账号级全选/取消全选（仅切换该账号下未导入角色）
  const toggleAccountAll = (account: PreviewableAccount) => {
    if (isAccountAllImported(account) || importing) return;
    const importableKeys = account.roles
      .filter(role => !role.alreadyImported)
      .map(role => buildRoleKey(account.accountName, role));
    setSelectedKeys(prev => {
      const next = new Set(prev);
      // 如果当前账号下所有可导入角色都已选中，则取消选中；否则全选
      const allSelected = importableKeys.every(k => next.has(k));
      if (allSelected) {
        importableKeys.forEach(k => next.delete(k));
      } else {
        importableKeys.forEach(k => next.add(k));
      }
      return next;
    });
  };

  // 全选所有未导入角色
  const selectAllImportable = () => {
    const next = new Set<string>();
    accounts.forEach(account => {
      account.roles.forEach(role => {
        if (!role.alreadyImported) {
          next.add(buildRoleKey(account.accountName, role));
        }
      });
    });
    setSelectedKeys(next);
  };

  // 清空选择
  const clearSelection = () => {
    setSelectedKeys(new Set());
  };

  // 执行导入
  const handleImport = async () => {
    if (selectedKeys.size === 0 || importing) return;

    setImporting(true);
    setError(null);

    try {
      // 构建导入列表
      const rolesToImport: ImportRoleItem[] = [];
      accounts.forEach(account => {
        account.roles.forEach(role => {
          const key = buildRoleKey(account.accountName, role);
          if (selectedKeys.has(key)) {
            rolesToImport.push({
              accountName: account.accountName,
              roleName: role.name,
              region: role.region,
              server: role.server,
            });
          }
        });
      });

      const result = await invoke<ImportResult>('import_selected_roles', {
        gameDirectory,
        roles: rolesToImport,
      });

      if (result.success) {
        const parts: string[] = [];
        if (result.newAccounts > 0) parts.push(`新增 ${result.newAccounts} 个账号`);
        if (result.newRoles > 0) parts.push(`新增 ${result.newRoles} 个角色`);
        if (result.updatedRoles > 0) parts.push(`更新 ${result.updatedRoles} 个角色`);
        toast.success(parts.length > 0 ? `导入完成：${parts.join('，')}` : '没有需要变更的角色');
        onImported();
        onClose();
      } else {
        setError(result.error || '导入失败');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setImporting(false);
    }
  };

  if (!isOpen) return null;

  return createPortal(
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm overflow-hidden"
      onClick={onClose}
    >
      <div
        className="bg-surface border border-base rounded-xl shadow-xl w-full max-w-2xl mx-4 flex flex-col overflow-hidden animate-in fade-in zoom-in-95 duration-200"
        style={{ maxHeight: '85vh' }}
        onClick={e => e.stopPropagation()}
      >
        {/* 头部 */}
        <div className="px-5 py-2.5 flex items-center justify-between border-b border-base shrink-0">
          <div className="flex items-center gap-2 min-w-0">
            <Download className="w-4 h-4 text-primary shrink-0" />
            <h2 className="text-base font-semibold text-main truncate">导入本地角色</h2>
          </div>
          <button
            onClick={onClose}
            className="text-muted hover:text-main hover:bg-base p-1.5 rounded-lg transition-colors shrink-0"
            disabled={importing}
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* 统计信息栏 */}
        {!loading && !error && accounts.length > 0 && (
          <div className="px-5 py-1.5 bg-base/40 border-b border-base shrink-0 flex items-center justify-between gap-3 flex-wrap">
            <div className="flex items-center gap-3 text-sm text-muted">
              <span>共 <span className="font-medium text-main">{stats.totalRoles}</span> 个角色</span>
              <span className="text-muted/50">·</span>
              <span>已导入 <span className="font-medium text-emerald-600">{stats.importedCount}</span></span>
              <span className="text-muted/50">·</span>
              <span>可导入 <span className="font-medium text-primary">{stats.importableCount}</span></span>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={selectAllImportable}
                className="text-xs px-2.5 py-1 rounded-md bg-surface border border-base hover:border-primary hover:text-primary text-main transition-all"
                disabled={importing || stats.importableCount === 0}
              >
                全选可导入
              </button>
              <button
                onClick={clearSelection}
                className="text-xs px-2.5 py-1 rounded-md bg-surface border border-base hover:border-primary hover:text-primary text-main transition-all"
                disabled={importing || selectedCount === 0}
              >
                清空
              </button>
            </div>
          </div>
        )}

        {/* 内容区 */}
        <div className="flex-1 overflow-y-auto">
          {loading && (
            <div className="flex flex-col items-center justify-center py-16 text-muted">
              <Loader2 className="w-8 h-8 animate-spin mb-3 text-primary" />
              <p className="text-sm">正在扫描本地角色...</p>
            </div>
          )}

          {!loading && error && (
            <div className="p-6">
              <div className="p-4 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-lg flex items-start gap-3 text-red-600 dark:text-red-400">
                <AlertCircle className="w-5 h-5 shrink-0 mt-0.5" />
                <div className="text-sm whitespace-pre-wrap">{error}</div>
              </div>
            </div>
          )}

          {!loading && !error && accounts.length === 0 && (
            <div className="flex flex-col items-center justify-center py-16 text-muted">
              <User className="w-10 h-10 text-muted/40 mb-3" />
              <p className="text-sm">未扫描到任何本地角色</p>
              <p className="text-xs text-muted/60 mt-1">请确认游戏目录配置正确，且 userdata 目录下有角色数据</p>
            </div>
          )}

          {!loading && !error && accounts.length > 0 && (
            <div className="p-4 space-y-3">
              {[...accounts]
                .sort((a, b) => {
                  const aAllImported = isAccountAllImported(a);
                  const bAllImported = isAccountAllImported(b);
                  if (aAllImported === bAllImported) return 0;
                  return aAllImported ? 1 : -1;
                })
                .map(account => {
                const accountSelectedCount = account.roles.filter(role =>
                  selectedKeys.has(buildRoleKey(account.accountName, role))
                ).length;
                const accountImportableCount = account.roles.filter(role => !role.alreadyImported).length;
                const allImported = isAccountAllImported(account);
                const accountDisabled = allImported || importing;
                const isExpanded = expandedAccounts.has(account.accountName);
                const importableKeys = account.roles
                  .filter(role => !role.alreadyImported)
                  .map(role => buildRoleKey(account.accountName, role));
                const accountAllSelected = importableKeys.length > 0 && importableKeys.every(k => selectedKeys.has(k));

                return (
                  <div key={account.accountName} className="bg-base/30 border border-base rounded-lg overflow-hidden">
                    {/* 账号标题（可点击展开/收起） */}
                    <div
                      className={`px-4 py-1.5 bg-base/50 border-b border-base flex items-center gap-2 ${
                        allImported ? 'opacity-60' : ''
                      }`}
                    >
                      {/* 账号级全选 checkbox */}
                      <input
                        type="checkbox"
                        checked={accountAllSelected}
                        onChange={() => toggleAccountAll(account)}
                        disabled={accountDisabled}
                        onClick={e => e.stopPropagation()}
                        className={`w-4 h-4 text-primary border-base rounded focus:ring-primary shrink-0 ${
                          accountDisabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'
                        }`}
                      />
                      {/* 展开/收起按钮 */}
                      <button
                        type="button"
                        onClick={() => toggleAccountExpansion(account.accountName)}
                        className="flex items-center gap-2 flex-1 min-w-0 text-left hover:text-primary transition-colors"
                      >
                        {isExpanded ? <ChevronDown className="w-4 h-4 text-muted shrink-0" /> : <ChevronRight className="w-4 h-4 text-muted shrink-0" />}
                        <User className="w-4 h-4 text-primary shrink-0" />
                        <span className="text-sm font-semibold text-main truncate">{account.accountName}</span>
                        <span className="text-xs text-muted shrink-0">
                          {accountSelectedCount}/{account.roles.length} 选中
                          {allImported ? (
                            <span className="text-emerald-600 ml-1">· 全部已导入</span>
                          ) : accountImportableCount > 0 && accountImportableCount < account.roles.length ? (
                            <span className="text-muted/60 ml-1">({accountImportableCount} 个可导入)</span>
                          ) : null}
                        </span>
                      </button>
                    </div>

                    {/* 角色列表（展开时显示） */}
                    {isExpanded && (
                      <div className="divide-y divide-base/60">
                        {[...account.roles]
                          .sort((a, b) => {
                            if (a.alreadyImported === b.alreadyImported) return 0;
                            return a.alreadyImported ? 1 : -1;
                          })
                          .map(role => {
                          const key = buildRoleKey(account.accountName, role);
                          const isSelected = selectedKeys.has(key);
                          const isDisabled = role.alreadyImported || importing;
                          return (
                            <label
                              key={key}
                              className={`flex items-center gap-3 px-4 py-1.5 transition-colors ${
                                isDisabled
                                  ? 'cursor-not-allowed bg-base/20'
                                  : `cursor-pointer ${isSelected ? 'bg-primary/5' : 'hover:bg-base/40'}`
                              }`}
                            >
                              <input
                                type="checkbox"
                                checked={isSelected}
                                onChange={() => toggleRole(key)}
                                disabled={isDisabled}
                                className={`w-4 h-4 text-primary border-base rounded focus:ring-primary shrink-0 ${
                                  isDisabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'
                                }`}
                              />
                              <div className={`flex-1 min-w-0 flex items-center gap-2 flex-wrap ${role.alreadyImported ? 'opacity-60' : ''}`}>
                                <span className={`text-sm font-medium truncate ${role.alreadyImported ? 'text-muted' : 'text-main'}`}>
                                  {role.name}
                                </span>
                                <span className="text-xs text-muted">·</span>
                                <span className="text-xs text-muted truncate">{getBaseServerName(role.server)}</span>
                                {(role.kungfuName || role.forceName) && (
                                  <SectIcon sectName={role.kungfuName || role.forceName || ''} variant="image" size="sm" />
                                )}
                                {role.equipmentScore > 0 && (
                                  <span className="text-[11px] bg-blue-50 text-blue-700 border border-blue-100 px-1.5 py-0.5 rounded-md font-medium shrink-0">
                                    装分 {role.equipmentScore.toLocaleString()}
                                  </span>
                                )}
                                {role.alreadyImported && (
                                  <span className="text-[10px] font-medium text-emerald-700 bg-emerald-50 px-1.5 py-0.5 rounded-full border border-emerald-100 shrink-0 flex items-center gap-0.5">
                                    <Check size={10} />
                                    已导入
                                  </span>
                                )}
                              </div>
                            </label>
                          );
                        })}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* 底部操作栏 */}
        <div className="px-6 py-4 bg-base/50 flex items-center justify-between gap-3 border-t border-base shrink-0">
          <div className="text-sm text-muted">
            已选择 <span className="font-medium text-primary">{selectedCount}</span> 个角色
          </div>
          <div className="flex gap-3">
            <button
              onClick={onClose}
              disabled={importing}
              className="px-4 py-2 bg-surface hover:bg-base border border-base text-main hover:border-primary hover:text-primary active:scale-[0.98] rounded-lg transition-all duration-200 font-medium disabled:opacity-60 disabled:cursor-not-allowed"
            >
              取消
            </button>
            <button
              onClick={handleImport}
              disabled={selectedCount === 0 || importing || !!error}
              className="px-4 py-2 bg-primary hover:bg-primary-hover text-white shadow-sm hover:shadow active:scale-[0.98] rounded-lg transition-all duration-200 font-medium disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-2"
            >
              {importing && <Loader2 className="w-4 h-4 animate-spin" />}
              {importing ? '导入中...' : `导入选中角色`}
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body
  );
};
