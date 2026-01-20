import React, { useState, useMemo } from 'react';
import { Account, AccountType, Role, Config } from '../types';
import { SECTS } from '../constants';
import { Plus, Trash2, User, UserCheck, Eye, EyeOff, Clipboard, Check, Loader2, AlertCircle, CheckCircle2, XCircle, Search, X, Settings } from 'lucide-react';
import { convertToSystemAccounts } from '../services/directoryParser';
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

  const [visiblePasswords, setVisiblePasswords] = useState<Set<string>>(new Set());
  const [copySuccess, setCopySuccess] = useState<string | null>(null);

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

  // 操作结果详情
  const [operationResult, setOperationResult] = useState<{
    successCount: number;
    failureCount: number;
    successDetails: string[];
    failureDetails: Array<{ accountId: string; accountName: string; reason: string }>;
  } | null>(null);

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

      return {
        ...account,
        roles: account.roles.map(role => {
          if (role.id !== roleId) return role;
          return {
            ...role,
            sect: sect.trim(),
            equipmentScore: equipmentScore !== undefined && equipmentScore !== null ? equipmentScore : undefined
          };
        })
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
          roles: [...account.roles, role]
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
      <div className="sticky top-0 z-20 bg-slate-50 pt-2 pb-4">
        <div className="flex justify-between items-center flex-wrap gap-3">
          <h2 className="text-2xl font-bold text-slate-800">账号管理</h2>
          <div className="flex gap-2 items-center flex-wrap">
            {/* 搜索框 */}
            <div className="relative">
              <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
              <input
                type="text"
                placeholder="搜索账号或角色..."
                value={searchTerm}
                onChange={e => setSearchTerm(e.target.value)}
                className="pl-9 pr-8 py-2 border border-slate-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-emerald-400 w-48 placeholder:text-slate-400"
              />
              {searchTerm && (
                <button
                  onClick={clearSearch}
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600 p-0.5 rounded hover:bg-slate-100"
                >
                  <X className="w-4 h-4" />
                </button>
              )}
            </div>

            {/* 搜索结果提示 */}
            {searchTerm && (
              <span className="text-sm text-slate-500">
                找到 <span className="font-medium text-emerald-600">{filteredAccounts.length}</span> 个匹配
              </span>
            )}

            {/* 使用配置目录解析按钮 */}
            {config?.game?.gameDirectory && (
              <button
                onClick={handleUseConfigDirectory}
                className="bg-green-600 hover:bg-green-700 text-white px-4 py-2 rounded-lg flex items-center gap-2 transition-colors"
                disabled={isScanning}
              >
                {isScanning ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    解析中...
                  </>
                ) : (
                  <>
                    <Loader2 className="w-4 h-4" />
                    使用配置目录解析
                  </>
                )}
              </button>
            )}
            <button
              onClick={() => setIsAddAccountModalOpen(true)}
              className="bg-emerald-600 hover:bg-emerald-700 text-white px-4 py-2 rounded-lg flex items-center gap-2 transition-colors"
            >
              <Plus className="w-4 h-4" /> 新增账号
            </button>
            {/* 批量删除按钮 - 仅在选中账号时显示 */}
            {selectedAccounts.size > 0 && (
              <button
                onClick={handleBatchDeleteClick}
                className="bg-red-600 hover:bg-red-700 text-white px-4 py-2 rounded-lg flex items-center gap-2 transition-colors shadow-sm hover:shadow-md"
              >
                <Trash2 className="w-4 h-4" />
                批量删除 ({selectedAccounts.size})
              </button>
            )}
          </div>
        </div>
        {/* 全选功能 */}
        {safeAccounts.length > 0 && (
          <div className="flex items-center gap-2 mt-3">
            <input
              type="checkbox"
              checked={isAllSelected}
              onChange={handleSelectAll}
              className="w-4 h-4 text-emerald-600 border-gray-300 rounded focus:ring-emerald-500"
            />
            <label className="text-sm font-medium text-slate-700">全选 ({safeAccounts.length} 个账户)</label>
          </div>
        )}
      </div>

      {/* 解析错误显示 */}
      {parseError && (
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
      )}

      {/* 扫描进度显示 */}
      {scanProgress && (
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
      )}



      {/* 无搜索结果提示 */}
      {searchTerm && filteredAccounts.length === 0 && (
        <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-8 text-center">
          <Search className="w-12 h-12 text-slate-200 mx-auto mb-3" />
          <p className="text-slate-500 mb-2">未找到匹配的结果</p>
          <p className="text-sm text-slate-400">
            搜索关键词: <span className="font-medium text-slate-600">"{searchTerm}"</span>
          </p>
          <button
            onClick={clearSearch}
            className="mt-4 px-4 py-2 bg-slate-100 hover:bg-slate-200 text-slate-600 rounded-lg transition-colors text-sm"
          >
            清除搜索
          </button>
        </div>
      )}

      <div className="space-y-6">
        {filteredAccounts.map(account => (
          <div key={account.id} className={`bg-white p-5 rounded-xl shadow-sm border border-slate-200 hover:shadow-md transition-shadow ${selectedAccounts.has(account.id) ? 'ring-2 ring-emerald-500' : ''} ${account.disabled ? 'opacity-60' : ''}`}>
            <div className="flex justify-between items-start mb-4">
              <div className="flex items-center gap-2">
                {/* 复选框 */}
                <input
                  type="checkbox"
                  checked={selectedAccounts.has(account.id)}
                  onChange={() => handleSelectAccount(account.id)}
                  className="w-4 h-4 text-indigo-600 border-gray-300 rounded focus:ring-indigo-500"
                />
                <span className={`p-2 rounded-full ${account.type === AccountType.OWN ? 'bg-blue-100 text-blue-600' : 'bg-green-100 text-green-600'}`}>
                  {account.type === AccountType.OWN ? <User size={18} /> : <UserCheck size={18} />}
                </span>
                <div>
                  <h3 className={`font-bold text-slate-800 ${account.disabled ? 'line-through text-slate-400' : ''}`}>{account.accountName}</h3>
                  <p className="text-xs text-slate-500">{Array.isArray(account.roles) ? account.roles.length : 0} 个角色</p>
                </div>
              </div>
              <div className="flex gap-2">
                {/* 代清状态切换按钮 */}
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
                  className={`text-slate-300 hover:text-emerald-600 transition-colors p-1 rounded-full hover:bg-emerald-50`}
                  title={account.type === AccountType.CLIENT ? '取消代清' : '标记代清'}
                >
                  {account.type === AccountType.CLIENT ? <CheckCircle2 size={16} className="text-green-600" /> : <UserCheck size={16} />}
                </button>
                {/* 禁用/启用切换按钮 */}
                <button
                  onClick={() => {
                    setAccounts(prev => prev.map(a => {
                      if (a.id === account.id) {
                        return { ...a, disabled: !a.disabled };
                      }
                      return a;
                    }));
                  }}
                  className={`text-slate-300 hover:text-emerald-600 transition-colors p-1 rounded-full hover:bg-emerald-50`}
                  title={account.disabled ? '启用账号' : '禁用账号'}
                >
                  {account.disabled ? <CheckCircle2 size={16} /> : <XCircle size={16} />}
                </button>
                <button onClick={() => handleDeleteAccountClick(account.id)} className="text-slate-300 hover:text-red-500 transition-colors">
                  <Trash2 size={16} />
                </button>
              </div>
            </div>

            {/* 账号信息编辑区域 */}
            <div className="bg-white rounded-lg shadow-sm border border-slate-200 p-4 mb-4">
              <h4 className="font-semibold text-slate-700 mb-3 flex items-center gap-2">
                <User className="w-4 h-4" />
                账号信息
              </h4>

              <div className="space-y-4">
                {/* 登录账号 - 自动从账户名称获取，不可编辑 */}
                <div className="flex items-center gap-3">
                  <label className="text-sm font-semibold text-slate-700 w-24 flex-shrink-0">
                    登录账号
                  </label>
                  <div className="flex-1 px-4 py-3 bg-slate-100 border border-slate-300 rounded-xl text-slate-800 font-medium">
                    {account.username || account.accountName}
                  </div>
                </div>

                {/* 密码 */}
                <div className="flex items-center gap-3">
                  <label className="text-sm font-semibold text-slate-700 w-24 flex-shrink-0">
                    密码
                  </label>
                  <div className="flex-1 flex gap-2">
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
                      className="flex-1 px-4 py-3 border-2 border-slate-300 rounded-xl text-slate-800 font-medium focus:ring-2 focus:ring-emerald-500 focus:border-emerald-500 transition-all placeholder:text-slate-400"
                      placeholder="输入游戏密码"
                    />
                    <div className="flex gap-1.5">
                      <button
                        onClick={() => togglePasswordVisibility(account.id)}
                        className="p-2.5 rounded-lg bg-slate-100 hover:bg-slate-200 text-slate-600 hover:text-indigo-600 transition-colors"
                        title={visiblePasswords.has(account.id) ? '隐藏密码' : '显示密码'}
                      >
                        {visiblePasswords.has(account.id) ? <EyeOff size={18} /> : <Eye size={18} />}
                      </button>
                      {account.password && (
                        <button
                          onClick={() => copyPassword(account.password, account.id)}
                          className={`p-2.5 rounded-lg bg-slate-100 hover:bg-slate-200 text-slate-600 hover:text-emerald-600 transition-colors ${copySuccess === account.id ? 'bg-green-100 text-green-700' : ''}`}
                          title="复制密码"
                        >
                          {copySuccess === account.id ? <Check size={18} /> : <Clipboard size={18} />}
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            </div>

            {/* Roles Section */}
            <div className="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
              <div className="flex justify-between items-center mb-3">
                <h4 className="font-semibold text-slate-700 flex items-center gap-2">
                  <User className="w-4 h-4" />
                  角色列表
                </h4>
                <button
                  onClick={() => setAddingRoleToAccountId(account.id)}
                  className="flex items-center gap-2 bg-emerald-600 hover:bg-emerald-700 text-white px-3 py-2 rounded-lg transition-colors shadow-sm hover:shadow-md"
                >
                  <Plus className="w-4 h-4" />
                  添加角色
                </button>
              </div>

              <div className="space-y-2">
                {!Array.isArray(account.roles) || account.roles.length === 0 ? (
                  <div className="text-center py-5 text-slate-400">
                    <p className="flex items-center justify-center gap-2">
                      <User className="w-5 h-5" />
                      暂无角色，点击上方按钮添加
                    </p>
                  </div>
                ) : (
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                    {account.roles.map(role => (
                      <div key={role.id} className={`bg-slate-50 border border-slate-200 p-3 rounded-lg hover:shadow-sm transition-shadow ${role.disabled ? 'opacity-60' : ''}`}>
                        <div className="flex justify-between items-start">
                          <div className="flex-1">
                            <div className="flex flex-wrap items-center gap-2 mb-1.5">
                              <h5 className={`font-medium text-slate-800 ${role.disabled ? 'line-through text-slate-400' : ''}`}>{role.name}</h5>
                              {role.sect ? (
                                <span className="text-xs bg-emerald-100 text-emerald-700 px-2 py-1 rounded-md font-medium">{role.sect}</span>
                              ) : (
                                <span className="text-xs text-slate-400 italic">未设置门派</span>
                              )}
                              {role.equipmentScore !== undefined && role.equipmentScore !== null && (
                                <span className="text-xs bg-blue-100 text-blue-700 px-2 py-1 rounded-md font-medium">
                                  {role.equipmentScore.toLocaleString()}
                                </span>
                              )}
                            </div>
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="text-xs text-slate-600">{role.region}</span>
                              <span className="text-xs text-slate-400">·</span>
                              <span className="text-xs text-slate-600">{role.server}</span>
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

            {
              account.type === AccountType.CLIENT && (
                <div className="absolute top-2 right-10">
                  <span className="text-[10px] font-bold uppercase tracking-wider text-green-600 bg-green-50 px-2 py-1 rounded-full border border-green-100">代清</span>
                </div>
              )
            }
          </div>
        ))}
      </div>

      {/* Batch Delete Confirmation Dialog */}
      {
        showBatchDeleteConfirm && (
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-white p-6 rounded-xl shadow-lg max-w-sm w-full mx-4">
              <h3 className="text-lg font-semibold text-slate-800 mb-4">确认删除</h3>
              <p className="text-slate-600 mb-6">确认删除选中的 {selectedAccounts.size} 个账号？关联的角色和副本记录不会被删除，但账号列表将不再显示。</p>
              <div className="flex justify-end gap-3">
                <button
                  onClick={handleBatchDeleteCancel}
                  className="px-4 py-2 text-slate-600 border border-slate-300 rounded-lg hover:bg-slate-50 transition-colors"
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
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-white p-6 rounded-xl shadow-lg max-w-sm w-full mx-4">
              <h3 className="text-lg font-semibold text-slate-800 mb-4">确认删除</h3>
              <p className="text-slate-600 mb-6">确认删除此账号？关联的角色和副本记录不会被删除，但账号列表将不再显示。</p>
              <div className="flex justify-end gap-3">
                <button
                  onClick={handleDeleteAccountCancel}
                  className="px-4 py-2 text-slate-600 border border-slate-300 rounded-lg hover:bg-slate-50 transition-colors"
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
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-white p-6 rounded-xl shadow-lg max-w-sm w-full mx-4">
              <h3 className="text-lg font-semibold text-slate-800 mb-4">确认删除</h3>
              <p className="text-slate-600 mb-6">确认删除此角色？</p>
              <div className="flex justify-end gap-3">
                <button
                  onClick={handleDeleteRoleCancel}
                  className="px-4 py-2 text-slate-600 border border-slate-300 rounded-lg hover:bg-slate-50 transition-colors"
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
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-white p-6 rounded-xl shadow-lg max-w-md w-full mx-4">
              <h3 className="text-lg font-semibold text-slate-800 mb-4">修改角色信息</h3>

              <div className="space-y-4">
                {/* 门派选择 */}
                <div>
                  <label className="block text-sm font-medium text-slate-700 mb-1">
                    门派 <span className="text-red-500">*</span>
                  </label>
                  <select
                    className={`w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-transparent transition-all ${roleFormErrors.sect ? 'border-red-300 bg-red-50' : 'border-slate-300'
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
                    className={`w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-transparent transition-all ${roleFormErrors.equipmentScore ? 'border-red-300 bg-red-50' : 'border-slate-300'
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
                  className="px-4 py-2 text-slate-600 border border-slate-300 rounded-lg hover:bg-slate-50 transition-colors"
                >
                  取消
                </button>
                <button
                  onClick={handleSaveRoleInfo}
                  className="px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 transition-colors"
                >
                  保存
                </button>
              </div>
            </div>
          </div>
        )
      }

      {/* Operation Result Dialog */}
      {
        operationResult && (
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 overflow-y-auto">
            <div className="bg-white p-6 rounded-xl shadow-lg max-w-2xl w-full mx-4 max-h-[80vh] overflow-y-auto">
              <h3 className="text-lg font-semibold text-slate-800 mb-4">操作结果详情</h3>

              <div className="space-y-4">
                {/* 统计信息 */}
                <div className="grid grid-cols-2 gap-4">
                  <div className="bg-green-50 border border-green-200 rounded-lg p-4">
                    <h4 className="text-sm font-medium text-green-700 mb-1">成功数量</h4>
                    <p className="text-2xl font-bold text-green-600">{operationResult.successCount}</p>
                  </div>
                  <div className="bg-red-50 border border-red-200 rounded-lg p-4">
                    <h4 className="text-sm font-medium text-red-700 mb-1">失败数量</h4>
                    <p className="text-2xl font-bold text-red-600">{operationResult.failureCount}</p>
                  </div>
                </div>

                {/* 成功详情 */}
                {operationResult.successCount > 0 && (
                  <div>
                    <h4 className="text-sm font-medium text-slate-700 mb-2">成功列表</h4>
                    <div className="bg-slate-50 border border-slate-200 rounded-lg p-4 max-h-60 overflow-y-auto">
                      <ul className="space-y-1">
                        {operationResult.successDetails.map((detail, index) => (
                          <li key={index} className="flex items-center gap-2 text-sm text-green-600">
                            <CheckCircle2 className="w-4 h-4" />
                            <span>{detail}</span>
                          </li>
                        ))}
                      </ul>
                    </div>
                  </div>
                )}

                {/* 失败详情 */}
                {operationResult.failureCount > 0 && (
                  <div>
                    <h4 className="text-sm font-medium text-slate-700 mb-2">失败列表</h4>
                    <div className="bg-slate-50 border border-slate-200 rounded-lg p-4 max-h-60 overflow-y-auto">
                      <ul className="space-y-2">
                        {operationResult.failureDetails.map((detail, index) => (
                          <li key={index} className="text-sm">
                            <div className="flex items-start gap-2 text-red-600">
                              <XCircle className="w-4 h-4 mt-0.5" />
                              <div>
                                <div className="font-medium">{detail.accountName}</div>
                                <div className="text-xs text-red-500">{detail.reason}</div>
                              </div>
                            </div>
                          </li>
                        ))}
                      </ul>
                    </div>
                  </div>
                )}

                {/* 操作按钮 */}
                <div className="flex justify-end gap-3 pt-2 border-t border-slate-200">
                  {operationResult.failureCount > 0 && (
                    <button
                      onClick={() => {
                        // 导出失败记录为CSV
                        const csvContent = [
                          ['账号ID', '账号名称', '失败原因'],
                          ...operationResult.failureDetails.map(detail => [detail.accountId, detail.accountName, detail.reason])
                        ]
                          .map(row => row.join(','))
                          .join('\n');

                        const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
                        const link = document.createElement('a');
                        link.href = URL.createObjectURL(blob);
                        link.download = `operation_failures_${new Date().toISOString().slice(0, 10)}.csv`;
                        link.click();
                      }}
                      className="px-4 py-2 bg-slate-200 text-slate-700 rounded-lg hover:bg-slate-300 transition-colors"
                    >
                      导出失败记录
                    </button>
                  )}
                  <button
                    onClick={() => setOperationResult(null)}
                    className="px-4 py-2 bg-emerald-600 text-white rounded-lg hover:bg-emerald-700 transition-colors"
                  >
                    关闭
                  </button>
                </div>
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
    </div >
  );
};