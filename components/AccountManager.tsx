import React, { useState, useMemo } from 'react';
import { Account, AccountType, Role, Config } from '../types';
import { SECTS } from '../constants';
import { Plus, Trash2, User, UserCheck, Eye, EyeOff, Clipboard, Check, Loader2, AlertCircle, CheckCircle2, XCircle, Search, X, Settings, ChevronDown, ChevronRight } from 'lucide-react';
import { convertToSystemAccounts } from '../services/directoryParser';
import { sortRoles } from '../utils/accountUtils';
import { generateUUID } from '../utils/uuid';
import { scanGameDirectory, ScanProgress } from '../services/gameDirectoryScanner';
import { toast } from '../utils/toastManager';
import { AddAccountModal } from './AddAccountModal';
import { AddRoleModal } from './AddRoleModal';


interface AccountManagerProps {
  accounts: Account[];
  setAccounts: React.Dispatch<React.SetStateAction<Account[]>>;
  config?: Config;
}

export const AccountManager: React.FC<AccountManagerProps> = ({ accounts, setAccounts, config }) => {
  const safeAccounts = Array.isArray(accounts) ? accounts : [];
  // Modal State
  const [isAddAccountModalOpen, setIsAddAccountModalOpen] = useState(false);
  const [addingRoleToAccountId, setAddingRoleToAccountId] = useState<string | null>(null);
  const [expandedAccountIds, setExpandedAccountIds] = useState<Set<string>>(new Set());

  const toggleAccountExpansion = (accountId: string) => {
    setExpandedAccountIds(prev => {
      const newSet = new Set(prev);
      if (newSet.has(accountId)) {
        newSet.delete(accountId);
      } else {
        newSet.add(accountId);
      }
      return newSet;
    });
  };

  const [visiblePasswords, setVisiblePasswords] = useState<Set<string>>(new Set());
  const [copySuccess, setCopySuccess] = useState<string | null>(null);
  const [copyUsernameSuccess, setCopyUsernameSuccess] = useState<string | null>(null);

  const [selectedAccounts, setSelectedAccounts] = useState<Set<string>>(new Set());
  const [isAllSelected, setIsAllSelected] = useState(false);

  // 解析相关状态
  const [isScanning, setIsScanning] = useState(false);
  const [parseError, setParseError] = useState<string | null>(null);
  const [scanProgress, setScanProgress] = useState<ScanProgress | null>(null);

  // Dialog State
  const [showBatchDeleteConfirm, setShowBatchDeleteConfirm] = useState(false);
  const [confirmDeleteAccountId, setConfirmDeleteAccountId] = useState<string | null>(null);
  const [confirmDeleteRole, setConfirmDeleteRole] = useState<{ accountId: string; roleId: string } | null>(null);

  // 搜索相关状态
  const [searchTerm, setSearchTerm] = useState('');

  // 搜索筛选逻辑
  const filteredAccounts = useMemo(() => {
    if (!searchTerm.trim()) {
      return safeAccounts;
    }

    const term = searchTerm.toLowerCase().trim();

    return safeAccounts.filter(account => {
      // 搜索账号名称
      if (account.accountName.toLowerCase().includes(term)) {
        return true;
      }

      // 搜索角色名称
      if (Array.isArray(account.roles)) {
        return account.roles.some(role =>
          role.name.toLowerCase().includes(term) ||
          role.server.toLowerCase().includes(term) ||
          role.region.toLowerCase().includes(term) ||
          (role.sect && role.sect.toLowerCase().includes(term))
        );
      }

      return false;
    });
  }, [safeAccounts, searchTerm]);

  // 清空搜索
  const clearSearch = () => {
    setSearchTerm('');
  };



  // 打开修改角色信息弹窗
  const [editRoleModal, setEditRoleModal] = useState<{
    open: boolean;
    accountId: string;
    roleId: string;
    sect: string;
    equipmentScore: number | undefined;
  } | null>(null);

  // 角色信息表单验证状态
  const [roleFormErrors, setRoleFormErrors] = useState<{
    sect?: string;
    equipmentScore?: string;
  }>({});

  // 当accounts变化时，检查并清理重复账户
  React.useEffect(() => {
    if (safeAccounts.length > 0) {
      // 使用Map进行去重，保留第一个出现的账户
      const uniqueAccountsMap = new Map<string, Account>();
      const duplicateAccounts: string[] = [];

      for (const account of safeAccounts) {
        if (!uniqueAccountsMap.has(account.id)) {
          uniqueAccountsMap.set(account.id, account);
        } else {
          duplicateAccounts.push(account.accountName);
          console.log(`账户去重：发现重复账号 ${account.accountName} (ID: ${account.id})，保留第一个，删除后续重复项`);
        }
      }

      const uniqueAccounts = Array.from(uniqueAccountsMap.values());

      // 如果有重复账户被清理，更新账户列表
      if (uniqueAccounts.length !== safeAccounts.length) {
        const removedCount = safeAccounts.length - uniqueAccounts.length;
        console.log(`账户去重：成功清理了 ${removedCount} 个重复账户，重复账号包括：${duplicateAccounts.join(', ')}`);
        console.log(`账户去重：清理后剩余 ${uniqueAccounts.length} 个唯一账户`);
        setAccounts(uniqueAccounts);
      }
    }
  }, [safeAccounts, setAccounts]);

  // 全选/取消全选功能
  const handleSelectAll = () => {
    if (isAllSelected) {
      // 取消全选
      setSelectedAccounts(new Set());
      setIsAllSelected(false);
    } else {
      // 全选所有可见账户
      const allIds = safeAccounts.map(account => account.id);
      setSelectedAccounts(new Set(allIds));
      setIsAllSelected(true);
    }
  };

  // 单个账户选择功能
  const handleSelectAccount = (accountId: string) => {
    setSelectedAccounts(prev => {
      const newSet = new Set(prev);
      if (newSet.has(accountId)) {
        newSet.delete(accountId);
      } else {
        newSet.add(accountId);
      }
      // 更新全选状态
      setIsAllSelected(newSet.size === safeAccounts.length && safeAccounts.length > 0);
      return newSet;
    });
  };

  // 批量删除功能 - 显示确认对话框
  const handleBatchDeleteClick = () => {
    if (selectedAccounts.size === 0) return;
    setShowBatchDeleteConfirm(true);
  };

  // 批量删除确认 - 执行删除
  const handleBatchDeleteConfirm = () => {
    // 删除选中的账户
    setAccounts(prev => prev.filter(account => !selectedAccounts.has(account.id)));
    // 清空选中状态
    setSelectedAccounts(new Set());
    setIsAllSelected(false);

    // 显示成功消息
    toast.success(`成功删除了 ${selectedAccounts.size} 个账号`);
    setShowBatchDeleteConfirm(false);
  };

  // 批量删除取消 - 关闭对话框
  const handleBatchDeleteCancel = () => {
    setShowBatchDeleteConfirm(false);
  };

  // 打开修改角色信息弹窗
  const handleOpenEditRoleModal = (accountId: string, role: Role) => {
    setEditRoleModal({
      open: true,
      accountId,
      roleId: role.id,
      sect: role.sect || '',
      equipmentScore: role.equipmentScore
    });
    setRoleFormErrors({});
  };

  // 关闭修改角色信息弹窗
  const handleCloseEditRoleModal = () => {
    setEditRoleModal(null);
    setRoleFormErrors({});
  };

  // 验证角色信息表单
  const validateRoleForm = (sect: string, equipmentScore: number | undefined): boolean => {
    const errors: { sect?: string; equipmentScore?: string } = {};
    let isValid = true;

    if (!sect.trim()) {
      errors.sect = '请选择门派';
      isValid = false;
    }

    if (equipmentScore !== undefined && equipmentScore !== null) {
      if (equipmentScore < 0) {
        errors.equipmentScore = '装分不能为负数';
        isValid = false;
      }
      if (!Number.isInteger(equipmentScore)) {
        errors.equipmentScore = '装分必须为整数';
        isValid = false;
      }
    }

    setRoleFormErrors(errors);
    return isValid;
  };

  // 保存修改后的角色信息
  const handleSaveRoleInfo = () => {
    if (!editRoleModal) return;

    const { accountId, roleId, sect, equipmentScore } = editRoleModal;

    if (!validateRoleForm(sect, equipmentScore)) {
      return;
    }

    setAccounts(prev => prev.map(account => {
      if (account.id !== accountId) return account;

      const updatedRoles = account.roles.map(role => {
        if (role.id !== roleId) return role;
        return {
          ...role,
          sect: sect.trim(),
          equipmentScore: equipmentScore !== undefined && equipmentScore !== null ? equipmentScore : undefined
        };
      });

      return {
        ...account,
        roles: sortRoles(updatedRoles)
      };
    }));

    toast.success('角色信息更新成功');
    handleCloseEditRoleModal();
  };

  // 处理装分输入变化
  const handleEquipmentScoreChange = (value: string) => {
    if (!editRoleModal) return;

    const numValue = value === '' ? undefined : parseInt(value, 10);
    if (value !== '' && (isNaN(numValue!) || numValue! < 0)) {
      setRoleFormErrors(prev => ({ ...prev, equipmentScore: '请输入有效的非负整数' }));
    } else {
      setRoleFormErrors(prev => ({ ...prev, equipmentScore: undefined }));
    }

    setEditRoleModal(prev => prev ? { ...prev, equipmentScore: numValue } : null);
  };

  // 使用配置目录解析功能
  const handleUseConfigDirectory = async () => {
    if (!config?.game?.gameDirectory) {
      toast.error('请先在配置页面设置游戏目录');
      return;
    }

    setIsScanning(true);
    setScanProgress(null);
    setParseError(null);

    try {
      const result = await scanGameDirectory({
        gameDirectory: config.game.gameDirectory,
        onProgress: (progress) => {
          setScanProgress(progress);
        }
      });

      if (result.success && result.accounts.length > 0) {
        const systemAccounts = convertToSystemAccounts(result.accounts);

        const filteredAccounts: Account[] = [];
        const duplicateAccounts: Account[] = [];

        for (const newAccount of systemAccounts) {
          const existingAccount = safeAccounts.find(acc => acc.accountName === newAccount.accountName);

          if (!existingAccount) {
            filteredAccounts.push(newAccount);
          } else {
            duplicateAccounts.push(newAccount);
          }
        }

        if (filteredAccounts.length > 0) {
          setAccounts(prev => [...prev, ...filteredAccounts]);

          toast.success(`成功从配置目录解析到 ${filteredAccounts.length} 个账号，共 ${filteredAccounts.reduce((total, acc) => total + acc.roles.length, 0)} 个角色`);
        } else {
          toast.info('解析完成，但所有账号都已存在。');
        }
      } else {
        setParseError('解析完成，但未找到有效的账号和角色。请检查配置的游戏目录路径是否正确。');
      }
    } catch (error) {
      console.error('使用配置目录解析失败:', error);
      setParseError(`解析失败: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setIsScanning(false);
      setScanProgress(null);
    }
  };

  const handleAddAccountSubmit = (data: { accountName: string; type: AccountType; password?: string; notes?: string }) => {
    const account: Account = {
      id: generateUUID(),
      accountName: data.accountName,
      type: data.type,
      username: data.accountName,
      password: data.password || '',
      notes: data.notes || '',
      roles: []
    };

    setAccounts(prev => [...prev, account]);
    toast.success('成功添加账号');
  };

  const handleAddRoleSubmit = (data: {
    name: string;
    server: string;
    region: string;
    sect: string;
    equipmentScore?: number;
    isClient: boolean;
  }) => {
    if (!addingRoleToAccountId) return;

    const targetAccount = safeAccounts.find(a => a.id === addingRoleToAccountId);
    if (!targetAccount) return;

    const role: Role = {
      id: generateUUID(),
      name: data.name,
      server: data.server,
      region: data.region,
      sect: data.sect,
      isClient: data.isClient,
      equipmentScore: data.equipmentScore
    };

    setAccounts(prev => prev.map(account => {
      if (account.id === addingRoleToAccountId) {
        return {
          ...account,
          roles: sortRoles([...account.roles, role])
        };
      }
      return account;
    }));

    toast.success('成功添加角色');
  };

  // 显示删除账号确认对话框
  const handleDeleteAccountClick = (id: string) => {
    setConfirmDeleteAccountId(id);
  };

  // 执行删除账号
  const handleDeleteAccountConfirm = () => {
    if (confirmDeleteAccountId) {
      setAccounts(prev => prev.filter(a => a.id !== confirmDeleteAccountId));
      setConfirmDeleteAccountId(null);
      toast.success('成功删除账号');
    }
  };

  // 取消删除账号
  const handleDeleteAccountCancel = () => {
    setConfirmDeleteAccountId(null);
  };

  const togglePasswordVisibility = (id: string) => {
    const newSet = new Set(visiblePasswords);
    if (newSet.has(id)) newSet.delete(id);
    else newSet.add(id);
    setVisiblePasswords(newSet);
  };

  const copyPassword = (password: string | undefined, accountId: string) => {
    if (!password) return;
    navigator.clipboard.writeText(password)
      .then(() => {
        setCopySuccess(accountId);
        setTimeout(() => setCopySuccess(null), 2000);
      })
      .catch(err => {
        console.error('复制密码失败:', err);
      });
  };

  const copyUsername = (username: string, accountId: string) => {
    if (!username) return;
    navigator.clipboard.writeText(username)
      .then(() => {
        setCopyUsernameSuccess(accountId);
        setTimeout(() => setCopyUsernameSuccess(null), 2000);
      })
      .catch(err => {
        console.error('复制账号失败:', err);
      });
  };

  // 显示删除角色确认对话框
  const handleDeleteRoleClick = (accountId: string, roleId: string) => {
    setConfirmDeleteRole({ accountId, roleId });
  };

  // 执行删除角色
  const handleDeleteRoleConfirm = () => {
    if (confirmDeleteRole) {
      setAccounts(prev => prev.map(account => {
        if (account.id === confirmDeleteRole.accountId) {
          return {
            ...account,
            roles: account.roles.filter(role => role.id !== confirmDeleteRole.roleId)
          };
        }
        return account;
      }));
      setConfirmDeleteRole(null);
      toast.success('成功删除角色');
    }
  };

  // 取消删除角色
  const handleDeleteRoleCancel = () => {
    setConfirmDeleteRole(null);
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4">
        <h2 className="text-xl font-bold text-main">账号管理</h2>
        <div className="flex flex-wrap gap-2 w-full sm:w-auto">
          {/* 搜索框 */}
          <div className="relative flex-1 sm:flex-none">
            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
            <input
              type="text"
              placeholder="搜索账号 or 角色..."
              value={searchTerm}
              onChange={e => setSearchTerm(e.target.value)}
              className="pl-9 pr-8 py-1.5 w-full sm:w-48 bg-surface border border-base rounded-lg text-sm focus:outline-none focus:ring-1 focus:ring-primary focus:border-primary placeholder:text-muted text-main transition-all"
            />
            {searchTerm && (
              <button
                onClick={clearSearch}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted hover:text-main p-0.5 rounded-md hover:bg-base/50"
              >
                <X className="w-3 h-3" />
              </button>
            )}
          </div>

          {/* 搜索结果提示 */}
          {searchTerm && (
            <span className="text-sm text-muted">
              找到 <span className="font-medium text-emerald-600">{filteredAccounts.length}</span> 个匹配
            </span>
          )}

          {/* 使用配置目录解析按钮 */}
          {config?.game?.gameDirectory && (
            <button
              onClick={handleUseConfigDirectory}
              className="bg-emerald-50 text-emerald-600 border border-emerald-200 hover:bg-emerald-100 hover:border-emerald-300 px-3 py-1.5 rounded-lg flex items-center gap-2 transition-all text-sm font-medium"
              disabled={isScanning}
            >
              {isScanning ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  <span>解析中...</span>
                </>
              ) : (
                <>
                  <Loader2 className="w-4 h-4" />
                  <span>目录解析</span>
                </>
              )}
            </button>
          )}
          <button
            onClick={() => setIsAddAccountModalOpen(true)}
            className="bg-primary hover:bg-primary-hover text-white px-3 py-1.5 rounded-lg flex items-center gap-2 transition-all shadow-sm active:scale-[0.98] text-sm font-medium"
          >
            <Plus className="w-4 h-4" /> 新增账号
          </button>

          {/* 批量删除按钮 */}
          {selectedAccounts.size > 0 && (
            <button
              onClick={handleBatchDeleteClick}
              className="bg-red-50 text-red-600 border border-red-200 hover:bg-red-100 hover:border-red-300 px-3 py-1.5 rounded-lg flex items-center gap-2 transition-all text-sm font-medium"
            >
              <Trash2 className="w-4 h-4" />
              删除 ({selectedAccounts.size})
            </button>
          )}
        </div>
      </div>


      {/* 全选功能 */}
      {
        safeAccounts.length > 0 && (
          <div className="flex items-center gap-2 px-1">
            <input
              type="checkbox"
              checked={isAllSelected}
              onChange={handleSelectAll}
              className="w-4 h-4 text-primary border-base rounded focus:ring-primary"
            />
            <label className="text-sm font-medium text-main">全选 ({safeAccounts.length} 个账户)</label>
          </div>
        )
      }

      {/* 解析错误显示 */}
      {
        parseError && (
          <div className="bg-red-50 border border-red-200 rounded-lg p-4">
            <div className="flex flex-col gap-3">
              <div className="flex items-start gap-2">
                <AlertCircle className="w-5 h-5 text-red-600" />
                <div>
                  <h4 className="font-medium text-red-800">解析错误</h4>
                  <p className="text-sm text-red-600 mt-1">{parseError}</p>
                </div>
              </div>
              <div className="flex justify-end gap-2">
                <button
                  onClick={handleUseConfigDirectory}
                  className="text-sm bg-emerald-600 hover:bg-emerald-700 text-white px-3 py-1.5 rounded-lg flex items-center gap-1 transition-colors"
                >
                  <Loader2 className="w-3 h-3" /> 重试解析
                </button>
                <button
                  onClick={() => setParseError(null)}
                  className="text-sm bg-slate-200 hover:bg-slate-300 text-slate-700 px-3 py-1.5 rounded-lg transition-colors"
                >
                  关闭
                </button>
              </div>
            </div>
          </div>
        )
      }

      {/* 扫描进度显示 */}
      {
        scanProgress && (
          <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
            <div className="flex items-center gap-3">
              <Loader2 className="w-5 h-5 text-blue-600 animate-spin" />
              <div className="flex-1">
                <p className="font-medium text-blue-800">{scanProgress.message}</p>
                <div className="mt-2 bg-blue-100 rounded-full h-2 overflow-hidden">
                  <div
                    className="bg-blue-600 h-full transition-all duration-300"
                    style={{ width: `${(scanProgress.current / scanProgress.total) * 100}%` }}
                  />
                </div>
                <p className="text-xs text-blue-600 mt-1">
                  {scanProgress.current} / {scanProgress.total}
                </p>
              </div>
            </div>
          </div>
        )
      }



      {/* 无搜索结果提示 */}
      {
        searchTerm && filteredAccounts.length === 0 && (
          <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-8 text-center">
            <Search className="w-12 h-12 text-slate-200 mx-auto mb-3" />
            <p className="text-slate-500 mb-2">未找到匹配的结果</p>
            <p className="text-sm text-slate-400">
              搜索关键词: <span className="font-medium text-slate-600">"{searchTerm}"</span>
            </p>
            <button
              onClick={clearSearch}
              className="mt-4 px-4 py-2 bg-base hover:bg-base/80 text-muted rounded-lg transition-colors text-sm"
            >
              清除搜索
            </button>
          </div>
        )
      }

      <div className="space-y-6">
        {filteredAccounts.map(account => {
          const isExpanded = expandedAccountIds.has(account.id);
          return (
            <div key={account.id} className={`bg-surface rounded-lg border border-base transition-all ${isExpanded ? 'ring-1 ring-primary/20 shadow-sm' : 'hover:border-primary/30'} ${account.disabled ? 'opacity-60' : ''}`}>
              {/* 可点击的头部区域 */}
              <div
                className={`flex items-center justify-between p-4 cursor-pointer select-none transition-colors ${isExpanded ? 'bg-base/50' : 'hover:bg-base/30'}`}
                onClick={() => toggleAccountExpansion(account.id)}
              >
                <div className="flex items-center gap-3 overflow-hidden">
                  {/* 展开/折叠图标 */}
                  <div className="text-muted shrink-0 transition-transform duration-200">
                    {isExpanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
                  </div>

                  {/* 复选框 - 阻止冒泡 */}
                  <div onClick={e => e.stopPropagation()} className="flex items-center">
                    <input
                      type="checkbox"
                      checked={selectedAccounts.has(account.id)}
                      onChange={() => handleSelectAccount(account.id)}
                      className="w-4 h-4 text-primary border-base rounded focus:ring-primary cursor-pointer"
                    />
                  </div>

                  {/* 账号类型图标 */}
                  <span className={`p-1.5 rounded-lg shrink-0 ${account.type === AccountType.OWN ? 'bg-primary/10 text-primary' : 'bg-emerald-50 text-emerald-600'}`}>
                    {account.type === AccountType.OWN ? <User size={16} /> : <UserCheck size={16} />}
                  </span>

                  {/* 账号信息概要 */}
                  <div className="min-w-0 flex flex-col">
                    <div className="flex items-center gap-2">
                      <h3 className={`font-medium text-main truncate ${account.disabled ? 'line-through text-muted' : ''}`}>{account.accountName}</h3>
                      {account.type === AccountType.CLIENT && (
                        <span className="text-[10px] font-medium text-emerald-600 bg-emerald-50 px-1.5 py-0.5 rounded border border-emerald-100">代清</span>
                      )}
                    </div>
                    {!isExpanded && (
                      <p className="text-xs text-muted truncate mt-0.5">
                        {Array.isArray(account.roles) ? account.roles.length : 0} 个角色
                        {account.username && <span className="mx-1">·</span>}
                        {account.username}
                      </p>
                    )}
                  </div>
                </div>

                {/* 操作按钮区域 - 阻止冒泡 */}
                <div className="flex gap-1 shrink-0 ml-2" onClick={e => e.stopPropagation()}>
                  <button
                    onClick={() => {
                      setAccounts(prev => prev.map(a => {
                        if (a.id === account.id) {
                          return {
                            ...a,
                            type: a.type === AccountType.CLIENT ? AccountType.OWN : AccountType.CLIENT
                          };
                        }
                        return a;
                      }));
                    }}
                    className={`text-slate-300 hover:text-emerald-600 transition-colors p-1.5 rounded-md hover:bg-emerald-50`}
                    title={account.type === AccountType.CLIENT ? '取消代清' : '标记代清'}
                  >
                    {account.type === AccountType.CLIENT ? <CheckCircle2 size={16} className="text-green-600" /> : <UserCheck size={16} />}
                  </button>
                  <button
                    onClick={() => {
                      setAccounts(prev => prev.map(a => {
                        if (a.id === account.id) {
                          return { ...a, disabled: !a.disabled };
                        }
                        return a;
                      }));
                    }}
                    className={`text-slate-300 hover:text-emerald-600 transition-colors p-1.5 rounded-md hover:bg-emerald-50`}
                    title={account.disabled ? '启用账号' : '禁用账号'}
                  >
                    {account.disabled ? <CheckCircle2 size={16} /> : <XCircle size={16} />}
                  </button>
                  <button
                    onClick={() => handleDeleteAccountClick(account.id)}
                    className="text-muted hover:text-red-500 transition-colors p-1.5 rounded-md hover:bg-red-50"
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>

              {/* 折叠内容区域 */}
              {isExpanded && (
                <div className="p-4 border-t border-base animate-in slide-in-from-top-2 duration-200 fade-in space-y-4 cursor-default">
                  {/* 账号信息编辑区域 */}
                  <div className="bg-base/30 rounded-lg border border-base p-4">
                    <h4 className="font-semibold text-main mb-3 flex items-center gap-2">
                      <User className="w-4 h-4" />
                      账号信息
                    </h4>

                    <div className="space-y-4">
                      {/* 登录账号 */}
                      <div className="flex items-center gap-3">
                        <label className="text-sm font-semibold text-main w-24 flex-shrink-0">
                          登录账号
                        </label>
                        <div className="flex-1 flex gap-2">
                          <div className="flex-1 px-4 py-2.5 bg-surface border border-base rounded-lg text-main text-sm font-medium">
                            {account.username || account.accountName}
                          </div>
                          {(account.username || account.accountName) && (
                            <button
                              onClick={() => copyUsername(account.username || account.accountName, account.id)}
                              className={`p-2.5 rounded-lg border border-base transition-colors ${copyUsernameSuccess === account.id ? 'bg-emerald-50 text-emerald-600 border-emerald-200' : 'bg-surface text-muted hover:text-primary hover:bg-base'}`}
                              title="复制账号"
                            >
                              {copyUsernameSuccess === account.id ? <Check size={16} /> : <Clipboard size={16} />}
                            </button>
                          )}
                        </div>
                      </div>

                      {/* 密码 */}
                      <div className="flex items-center gap-3">
                        <label className="text-sm font-semibold text-main w-24 flex-shrink-0">
                          密码
                        </label>
                        <div className="flex-1 flex gap-2">
                          <div className="relative flex-1">
                            <input
                              type={visiblePasswords.has(account.id) ? 'text' : 'password'}
                              value={account.password || ''}
                              onChange={(e) => {
                                setAccounts(prev => prev.map(a => {
                                  if (a.id === account.id) {
                                    return { ...a, password: e.target.value };
                                  }
                                  return a;
                                }));
                              }}
                              className="w-full px-4 py-2.5 pr-10 border border-base bg-surface rounded-lg text-main text-sm font-medium focus:ring-1 focus:ring-primary focus:border-primary transition-all placeholder:text-muted"
                              placeholder="输入游戏密码"
                            />
                            <button
                              onClick={() => togglePasswordVisibility(account.id)}
                              className="absolute right-2 top-1/2 -translate-y-1/2 p-1.5 text-muted hover:text-primary transition-colors"
                              title={visiblePasswords.has(account.id) ? '隐藏密码' : '显示密码'}
                            >
                              {visiblePasswords.has(account.id) ? <EyeOff size={16} /> : <Eye size={16} />}
                            </button>
                          </div>
                          {account.password && (
                            <button
                              onClick={() => copyPassword(account.password, account.id)}
                              className={`p-2.5 rounded-lg border border-base transition-colors ${copySuccess === account.id ? 'bg-emerald-50 text-emerald-600 border-emerald-200' : 'bg-surface text-muted hover:text-primary hover:bg-base'}`}
                              title="复制密码"
                            >
                              {copySuccess === account.id ? <Check size={16} /> : <Clipboard size={16} />}
                            </button>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Roles Section */}
                  <div className="bg-base/30 rounded-lg border border-base p-4">
                    <div className="flex justify-between items-center mb-3">
                      <h4 className="font-semibold text-main flex items-center gap-2">
                        <User className="w-4 h-4" />
                        角色列表
                      </h4>
                      <button
                        onClick={() => setAddingRoleToAccountId(account.id)}
                        className="flex items-center gap-2 bg-primary hover:bg-primary-hover text-white px-3 py-1.5 rounded-lg transition-all text-sm font-medium shadow-sm active:scale-[0.98]"
                      >
                        <Plus className="w-4 h-4" />
                        添加角色
                      </button>
                    </div>

                    <div className="space-y-2">
                      {!Array.isArray(account.roles) || account.roles.length === 0 ? (
                        <div className="text-center py-5 text-muted">
                          <p className="flex items-center justify-center gap-2">
                            <User className="w-5 h-5" />
                            暂无角色，点击上方按钮添加
                          </p>
                        </div>
                      ) : (
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                          {account.roles.map(role => (
                            <div key={role.id} className={`bg-surface border border-base p-3 rounded-lg hover:border-primary/50 transition-colors ${role.disabled ? 'opacity-60' : ''}`}>
                              <div className="flex justify-between items-start">
                                <div className="flex-1">
                                  <div className="flex flex-wrap items-center gap-2 mb-1.5">
                                    <h5 className={`font-medium text-main ${role.disabled ? 'line-through text-muted' : ''}`}>{role.name}</h5>
                                    {role.sect ? (
                                      <span className="text-xs bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-400 px-2 py-1 rounded-md font-medium">{role.sect}</span>
                                    ) : (
                                      <span className="text-xs text-muted italic">未设置门派</span>
                                    )}
                                    {role.equipmentScore !== undefined && role.equipmentScore !== null && (
                                      <span className="text-xs bg-blue-100 text-blue-700 px-2 py-1 rounded-md font-medium">
                                        {role.equipmentScore.toLocaleString()}
                                      </span>
                                    )}
                                  </div>
                                  <div className="flex flex-wrap items-center gap-2">
                                    <span className="text-xs text-muted">{role.region}</span>
                                    <span className="text-xs text-muted">·</span>
                                    <span className="text-xs text-muted">{role.server}</span>
                                  </div>
                                </div>
                                <div className="flex gap-1">
                                  <button
                                    onClick={() => handleOpenEditRoleModal(account.id, role)}
                                    className="text-slate-400 hover:text-indigo-600 transition-colors p-1 rounded-full hover:bg-indigo-50"
                                    title="修改角色信息"
                                  >
                                    <Settings size={14} />
                                  </button>
                                  <button
                                    onClick={() => {
                                      setAccounts(prev => prev.map(a => {
                                        if (a.id === account.id) {
                                          return {
                                            ...a,
                                            roles: a.roles.map(r => {
                                              if (r.id === role.id) {
                                                return { ...r, disabled: !r.disabled };
                                              }
                                              return r;
                                            })
                                          };
                                        }
                                        return a;
                                      }));
                                    }}
                                    className={`text-slate-400 hover:text-indigo-600 transition-colors p-1 rounded-full hover:bg-indigo-50`}
                                    title={role.disabled ? '启用角色' : '禁用角色'}
                                  >
                                    {role.disabled ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
                                  </button>
                                  <button
                                    onClick={() => handleDeleteRoleClick(account.id, role.id)}
                                    className="text-slate-400 hover:text-red-500 transition-colors p-1 rounded-full hover:bg-red-50"
                                    title="删除角色"
                                  >
                                    <Trash2 size={16} />
                                  </button>
                                </div>
                              </div>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Batch Delete Confirmation Dialog */}
      {
        showBatchDeleteConfirm && (
          <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50">
            <div className="bg-surface p-6 rounded-lg shadow-xl border border-base max-w-sm w-full mx-4">
              <h3 className="text-lg font-semibold text-main mb-4">确认删除</h3>
              <p className="text-slate-600 mb-6">确认删除选中的 {selectedAccounts.size} 个账号？关联的角色和副本记录不会被删除，但账号列表将不再显示。</p>
              <div className="flex justify-end gap-3">
                <button
                  onClick={handleBatchDeleteCancel}
                  className="px-4 py-2 text-main border border-base rounded-lg hover:bg-base transition-colors"
                >
                  取消
                </button>
                <button
                  onClick={handleBatchDeleteConfirm}
                  className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors"
                >
                  确定
                </button>
              </div>
            </div>
          </div>
        )
      }



      {/* Single Account Delete Confirmation Dialog */}
      {
        confirmDeleteAccountId && (
          <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50">
            <div className="bg-surface p-6 rounded-lg shadow-xl border border-base max-w-sm w-full mx-4">
              <h3 className="text-lg font-semibold text-main mb-4">确认删除</h3>
              <p className="text-slate-600 mb-6">确认删除此账号？关联的角色和副本记录不会被删除，但账号列表将不再显示。</p>
              <div className="flex justify-end gap-3">
                <button
                  onClick={handleDeleteAccountCancel}
                  className="px-4 py-2 text-main border border-base rounded-lg hover:bg-base transition-colors"
                >
                  取消
                </button>
                <button
                  onClick={handleDeleteAccountConfirm}
                  className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors"
                >
                  确定
                </button>
              </div>
            </div>
          </div>
        )
      }

      {/* Single Role Delete Confirmation Dialog */}
      {
        confirmDeleteRole && (
          <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50">
            <div className="bg-surface p-6 rounded-lg shadow-xl border border-base max-w-sm w-full mx-4">
              <h3 className="text-lg font-semibold text-main mb-4">确认删除</h3>
              <p className="text-slate-600 mb-6">确认删除此角色？</p>
              <div className="flex justify-end gap-3">
                <button
                  onClick={handleDeleteRoleCancel}
                  className="px-4 py-2 text-main border border-base rounded-lg hover:bg-base transition-colors"
                >
                  取消
                </button>
                <button
                  onClick={handleDeleteRoleConfirm}
                  className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors"
                >
                  确定
                </button>
              </div>
            </div>
          </div>
        )
      }

      {/* Edit Role Info Modal */}
      {
        editRoleModal && editRoleModal.open && (
          <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50">
            <div className="bg-surface p-6 rounded-lg shadow-xl border border-base max-w-md w-full mx-4">
              <h3 className="text-lg font-semibold text-main mb-4">修改角色信息</h3>

              <div className="space-y-4">
                {/* 门派选择 */}
                <div>
                  <label className="block text-sm font-medium text-slate-700 mb-1">
                    门派 <span className="text-red-500">*</span>
                  </label>
                  <select
                    className={`w-full px-3 py-2 border rounded-lg bg-surface text-main focus:ring-1 focus:ring-primary focus:border-primary transition-all ${roleFormErrors.sect ? 'border-red-300 bg-red-50' : 'border-base'
                      }`}
                    value={editRoleModal.sect}
                    onChange={(e) => {
                      setEditRoleModal(prev => prev ? { ...prev, sect: e.target.value } : null);
                      if (e.target.value.trim()) {
                        setRoleFormErrors(prev => ({ ...prev, sect: undefined }));
                      }
                    }}
                  >
                    <option value="">请选择门派</option>
                    {SECTS.map(s => (
                      <option key={s} value={s}>{s}</option>
                    ))}
                  </select>
                  {roleFormErrors.sect && (
                    <p className="text-xs text-red-500 mt-1">{roleFormErrors.sect}</p>
                  )}
                </div>

                {/* 装分输入 */}
                <div>
                  <label className="block text-sm font-medium text-slate-700 mb-1">
                    装分
                  </label>
                  <input
                    type="number"
                    min="0"
                    className={`w-full px-3 py-2 border rounded-lg bg-surface text-main focus:ring-1 focus:ring-primary focus:border-primary transition-all ${roleFormErrors.equipmentScore ? 'border-red-300 bg-red-50' : 'border-base'
                      }`}
                    value={editRoleModal.equipmentScore === undefined ? '' : editRoleModal.equipmentScore}
                    onChange={(e) => handleEquipmentScoreChange(e.target.value)}
                    placeholder="请输入装备分数"
                  />
                  {roleFormErrors.equipmentScore ? (
                    <p className="text-xs text-red-500 mt-1">{roleFormErrors.equipmentScore}</p>
                  ) : (
                    <p className="text-xs text-slate-500 mt-1">留空表示不设置装分</p>
                  )}
                </div>
              </div>

              <div className="flex justify-end gap-3 mt-6 pt-4 border-t border-slate-200">
                <button
                  onClick={handleCloseEditRoleModal}
                  className="px-4 py-2 text-main border border-base rounded-lg hover:bg-base transition-colors"
                >
                  取消
                </button>
                <button
                  onClick={handleSaveRoleInfo}
                  className="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover transition-colors shadow-sm"
                >
                  保存
                </button>
              </div>
            </div>
          </div>
        )
      }



      {/* Add Account Modal */}
      < AddAccountModal
        isOpen={isAddAccountModalOpen}
        onClose={() => setIsAddAccountModalOpen(false)}
        onSubmit={handleAddAccountSubmit}
        existingAccountNames={safeAccounts.map(a => a.accountName)}
      />

      {/* Add Role Modal */}
      < AddRoleModal
        isOpen={!!addingRoleToAccountId}
        onClose={() => setAddingRoleToAccountId(null)}
        onSubmit={handleAddRoleSubmit}
        accountTypeIsClient={
          addingRoleToAccountId
            ? safeAccounts.find(a => a.id === addingRoleToAccountId)?.type === AccountType.CLIENT
            : false
        }
      />
    </div>
  );
};