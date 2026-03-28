import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Account, AccountType, Role, Config, InstanceType } from '../types';
import { SECTS } from '../constants';
import { Plus, Trash2, User, UserCheck, Eye, EyeOff, Clipboard, Check, Loader2, AlertCircle, CheckCircle2, XCircle, Search, X, Settings, ChevronDown, ChevronRight, Key, FileText } from 'lucide-react';
import { convertToSystemAccounts } from '../services/directoryParser';
import {
  canStartAccountDrag,
  getAccountReorderAnimationDuration,
  reorderAccounts,
  sortRoles,
} from '../utils/accountUtils';
import { generateUUID } from '../utils/uuid';
import { scanGameDirectory, ScanProgress } from '../services/gameDirectoryScanner';
import { toast } from '../utils/toastManager';
import { AddAccountModal } from './AddAccountModal';
import { AddRoleModal } from './AddRoleModal';
import { SectIcon } from './SectIcon';
import { db } from '../services/db';
import { deleteAccountDirectory, deleteRoleDirectory } from '../services/accountDirectoryCleanup';
import { getClientAccountNote } from '../utils/raidRoleUtils';
import { getBaseServerName } from '../utils/serverUtils';


interface AccountManagerProps {
  accounts: Account[];
  setAccounts: React.Dispatch<React.SetStateAction<Account[]>>;
  config?: Config;
  instanceTypes: InstanceType[];
}

interface AccountDragPointerPosition {
  clientX: number;
  clientY: number;
}

interface AccountDragPointerPayload extends AccountDragPointerPosition {
  pointerId: number;
}

export const AccountManager: React.FC<AccountManagerProps> = ({ accounts, setAccounts, config, instanceTypes }) => {
  const ACCOUNT_DRAG_START_DISTANCE = 6;
  const ACCOUNT_REORDER_EASING = 'cubic-bezier(0.22, 0.8, 0.2, 1)';
  const ACCOUNT_DRAG_SURFACE_EASING = 'cubic-bezier(0.2, 0.85, 0.2, 1)';
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
  const [isDeleting, setIsDeleting] = useState(false);
  const [batchDeleteDirectoryChecked, setBatchDeleteDirectoryChecked] = useState(true);
  const [accountDeleteDirectoryChecked, setAccountDeleteDirectoryChecked] = useState(true);
  const [roleDeleteDirectoryChecked, setRoleDeleteDirectoryChecked] = useState(true);

  // 搜索相关状态
  const [searchTerm, setSearchTerm] = useState('');
  // 账号类型筛选状态：'all' | 'own' | 'client'
  const [accountTypeFilter, setAccountTypeFilter] = useState<'all' | 'own' | 'client'>('all');
  // 防抖搜索词（用于实际的筛选逻辑）
  const [debouncedSearchTerm, setDebouncedSearchTerm] = useState('');
  // 防抖定时器引用
  const debounceTimerRef = useRef<number | null>(null);
  const [dragPreviewAccounts, setDragPreviewAccounts] = useState<Account[] | null>(null);
  const [draggedAccountId, setDraggedAccountId] = useState<string | null>(null);
  const [dragOverAccountId, setDragOverAccountId] = useState<string | null>(null);
  const [isAccountDragActive, setIsAccountDragActive] = useState(false);
  const accountCardRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const previousAccountPositions = useRef<Map<string, number>>(new Map());
  const pendingAccountDragRef = useRef<{
    accountId: string;
    pointerId: number;
    startX: number;
    startY: number;
    anchorOffsetY: number;
    sourceElement: HTMLDivElement;
  } | null>(null);
  const dragPreviewAccountsRef = useRef<Account[] | null>(null);
  const draggedAccountIdRef = useRef<string | null>(null);
  const dragOverAccountIdRef = useRef<string | null>(null);
  const isAccountDragActiveRef = useRef(false);
  const dragOffsetYRef = useRef(0);
  const dragPreviewFrameRef = useRef<number | null>(null);
  const latestDragPointerRef = useRef<AccountDragPointerPosition | null>(null);
  const pendingDragPointerRef = useRef<AccountDragPointerPosition | null>(null);
  const dragAnchorOffsetYRef = useRef(0);
  const suppressNextAccountClickRef = useRef(false);
  const latestAccountsRef = useRef<Account[]>(safeAccounts);
  const latestSearchTermRef = useRef(searchTerm);
  const latestSetAccountsRef = useRef(setAccounts);

  useEffect(() => {
    latestAccountsRef.current = safeAccounts;
  }, [safeAccounts]);

  useEffect(() => {
    latestSearchTermRef.current = searchTerm;
  }, [searchTerm]);

  useEffect(() => {
    latestSetAccountsRef.current = setAccounts;
  }, [setAccounts]);

  useEffect(() => {
    // 监听全局指针事件，彻底防止因 DOM 元素本身因排序复用而被浏览器切断 Pointer Capture 导致的卡手/丢失事件
    const handleMove = (event: PointerEvent) => handleAccountPointerMoveInternal(event);
    const handleUp = (event: PointerEvent) => handleAccountPointerUpInternal(event);
    const handleCancel = (event: PointerEvent) => handleAccountPointerCancelInternal(event);

    window.addEventListener('pointermove', handleMove);
    window.addEventListener('pointerup', handleUp);
    window.addEventListener('pointercancel', handleCancel);
    return () => {
      window.removeEventListener('pointermove', handleMove);
      window.removeEventListener('pointerup', handleUp);
      window.removeEventListener('pointercancel', handleCancel);
    };
  }, []);

  useEffect(() => {
    return () => {
      if (dragPreviewFrameRef.current !== null) {
        cancelAnimationFrame(dragPreviewFrameRef.current);
      }
      // 清理搜索防抖定时器
      if (debounceTimerRef.current !== null) {
        window.clearTimeout(debounceTimerRef.current);
      }
    };
  }, []);

  const displayAccounts = dragPreviewAccounts ?? safeAccounts;

  // 应用账号类型筛选
  const typeFilteredAccounts = useMemo(() => {
    if (accountTypeFilter === 'all') {
      return displayAccounts;
    }
    const targetType = accountTypeFilter === 'own' ? AccountType.OWN : AccountType.CLIENT;
    return displayAccounts.filter(account => account.type === targetType);
  }, [displayAccounts, accountTypeFilter]);

  // 搜索筛选逻辑（使用防抖后的搜索词）
  const filteredAccounts = useMemo(() => {
    if (!debouncedSearchTerm.trim()) {
      return typeFilteredAccounts;
    }

    const term = debouncedSearchTerm.toLowerCase().trim();

    return typeFilteredAccounts.filter(account => {
      // 搜索账号名称
      if (account.accountName.toLowerCase().includes(term)) {
        return true;
      }

      if (account.username?.toLowerCase().includes(term)) {
        return true;
      }

      if (account.notes?.toLowerCase().includes(term)) {
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
  }, [typeFilteredAccounts, debouncedSearchTerm]);

  // 清空搜索
  const clearSearch = () => {
    setSearchTerm('');
    setDebouncedSearchTerm('');
  };

  // 处理搜索输入（带防抖）
  const handleSearchChange = (value: string) => {
    setSearchTerm(value);
    // 清除之前的定时器
    if (debounceTimerRef.current !== null) {
      window.clearTimeout(debounceTimerRef.current);
    }
    // 设置新的定时器，150ms 防抖
    debounceTimerRef.current = window.setTimeout(() => {
      setDebouncedSearchTerm(value);
    }, 150);
  };

  const pendingDeleteAccount = useMemo(() => {
    if (!confirmDeleteAccountId) return null;
    return safeAccounts.find(account => account.id === confirmDeleteAccountId) || null;
  }, [confirmDeleteAccountId, safeAccounts]);

  const pendingDeleteRole = useMemo(() => {
    if (!confirmDeleteRole) return null;

    const account = safeAccounts.find(item => item.id === confirmDeleteRole.accountId);
    if (!account) return null;

    const role = account.roles.find(item => item.id === confirmDeleteRole.roleId);
    if (!role) return null;

    return { account, role };
  }, [confirmDeleteRole, safeAccounts]);

  const getConfiguredGameDirectory = (): string | null => {
    const gameDirectory = config?.game?.gameDirectory?.trim();
    if (!gameDirectory) {
      toast.error('\u8bf7\u5148\u914d\u7f6e\u6e38\u620f\u76ee\u5f55');
      return null;
    }
    return gameDirectory;
  };

  const getErrorMessage = (error: unknown): string => {
    return error instanceof Error ? error.message : String(error);
  };

  // 编辑角色信息弹窗
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
    setBatchDeleteDirectoryChecked(true);
    setShowBatchDeleteConfirm(true);
  };

  const handleBatchDeleteConfirm = async () => {
    if (selectedAccounts.size === 0 || isDeleting) return;

    const gameDirectory = batchDeleteDirectoryChecked ? getConfiguredGameDirectory() : null;
    if (batchDeleteDirectoryChecked && !gameDirectory) return;

    const selectedAccountIds = new Set(selectedAccounts);
    const accountsToDelete = safeAccounts.filter(account => selectedAccountIds.has(account.id));
    if (accountsToDelete.length === 0) {
      setShowBatchDeleteConfirm(false);
      return;
    }

    setIsDeleting(true);
    try {
      if (gameDirectory) {
        for (const account of accountsToDelete) {
          await deleteAccountDirectory(gameDirectory, account.accountName);
        }
      }

      setAccounts(prev => prev.filter(account => !selectedAccountIds.has(account.id)));
      setSelectedAccounts(new Set());
      setExpandedAccountIds(new Set());
      setIsAllSelected(false);
      setShowBatchDeleteConfirm(false);
      toast.success(
        batchDeleteDirectoryChecked
          ? `\u6210\u529f\u5220\u9664 ${accountsToDelete.length} \u4e2a\u8d26\u53f7\u53ca\u5bf9\u5e94\u76ee\u5f55`
          : `\u6210\u529f\u5220\u9664 ${accountsToDelete.length} \u4e2a\u8d26\u53f7`
      );
    } catch (error) {
      console.error('Failed to delete account directory:', error);
      toast.error(`\u76ee\u5f55\u5220\u9664\u5931\u8d25\uff0c\u672a\u5220\u9664\u8d26\u53f7: ${getErrorMessage(error)}`);
    } finally {
      setIsDeleting(false);
    }
  };

  const handleBatchDeleteCancel = () => {
    setShowBatchDeleteConfirm(false);
    setBatchDeleteDirectoryChecked(true);
  };

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

  // 切换角色可见性
  const handleToggleVisibility = async (roleId: string, instanceType: string, currentVisible: boolean) => {
    const newVisible = !currentVisible;

    // 乐观更新 UI
    setAccounts(prev => prev.map(account => ({
      ...account,
      roles: account.roles.map(role => {
        if (role.id === roleId) {
          return {
            ...role,
            visibility: {
              ...role.visibility,
              [instanceType]: newVisible
            }
          };
        }
        return role;
      })
    })));

    // 保存到数据库
    try {
      await db.saveRoleVisibility(roleId, instanceType, newVisible);
    } catch (error) {
      console.error('保存可见性失败:', error);
      toast.error('保存失败');
      // 回滚 UI
      setAccounts(prev => prev.map(account => ({
        ...account,
        roles: account.roles.map(role => {
          if (role.id === roleId) {
            return {
              ...role,
              visibility: {
                ...role.visibility,
                [instanceType]: currentVisible
              }
            };
          }
          return role;
        })
      })));
    }
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

        const newAccounts: Account[] = [];
        const updatedAccountsMap = new Map<string, Account>();
        let updatedCount = 0;
        let newRoleCount = 0;

        for (const scannedAccount of systemAccounts) {
          const existingAccount = safeAccounts.find(acc => acc.accountName === scannedAccount.accountName);

          if (!existingAccount) {
            newAccounts.push(scannedAccount);
          } else {
            // MERGE & CLEANUP LOGIC
            let hasChanges = false;
            let currentRoles = [...existingAccount.roles];

            for (const scannedRole of scannedAccount.roles) {
              // 1. Try to find an exact correct match
              const perfectMatch = currentRoles.find(r =>
                r.name === scannedRole.name &&
                r.region === scannedRole.region &&
                r.server === scannedRole.server
              );

              // 2. Try to find the specific "Bad Mapping" match from previous version
              // Logic: "Previous Region is Current Server" -> Old.Region == New.Server (and Names match)
              const legacyBadMatch = currentRoles.find(r =>
                r.name === scannedRole.name &&
                r.region === scannedRole.server &&
                (!perfectMatch || r.id !== perfectMatch.id)
              );

              if (perfectMatch) {
                // Case: Perfect match exists. 
                // If we ALSO have a bad legacy match, it's a duplicate. Remove the bad one.
                if (legacyBadMatch) {
                  currentRoles = currentRoles.filter(r => r.id !== legacyBadMatch.id);
                  hasChanges = true;
                  updatedCount++;
                }
              } else if (legacyBadMatch) {
                // Case: Only bad legacy match exists. This is our data to migrate.
                // Fix the Region and Server to correct values from scan.
                currentRoles = currentRoles.map(r => {
                  if (r.id === legacyBadMatch.id) {
                    return {
                      ...r,
                      region: scannedRole.region,
                      server: scannedRole.server
                    };
                  }
                  return r;
                });
                hasChanges = true;
                updatedCount++;
              } else {
                // Case: No match at all. Add new.
                currentRoles.push(scannedRole);
                hasChanges = true;
                newRoleCount++;
              }
            }

            if (hasChanges) {
              updatedAccountsMap.set(existingAccount.id, {
                ...existingAccount,
                roles: sortRoles(currentRoles)
              });
            }
          }
        }

        if (newAccounts.length > 0 || updatedAccountsMap.size > 0) {
          setAccounts(prev => {
            const next = prev.map(acc => updatedAccountsMap.get(acc.id) || acc);
            return [...next, ...newAccounts];
          });

          const totalNewRoles = newAccounts.reduce((total, acc) => total + acc.roles.length, 0) + newRoleCount;
          toast.success(`扫描完成：新增 ${newAccounts.length} 个账号，更新 ${updatedCount} 个账号，共发现 ${totalNewRoles} 个新角色`);
        } else {
          toast.info('解析完成，所有账号和角色已是最新状态。');
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
    setAccountDeleteDirectoryChecked(true);
    setConfirmDeleteAccountId(id);
  };

  const handleDeleteAccountConfirm = async () => {
    if (!pendingDeleteAccount || isDeleting) return;

    const gameDirectory = accountDeleteDirectoryChecked ? getConfiguredGameDirectory() : null;
    if (accountDeleteDirectoryChecked && !gameDirectory) return;

    setIsDeleting(true);
    try {
      if (gameDirectory) {
        await deleteAccountDirectory(gameDirectory, pendingDeleteAccount.accountName);
      }

      setAccounts(prev => prev.filter(account => account.id !== pendingDeleteAccount.id));
      setSelectedAccounts(prev => {
        const next = new Set(prev);
        next.delete(pendingDeleteAccount.id);
        return next;
      });
      setExpandedAccountIds(prev => {
        const next = new Set(prev);
        next.delete(pendingDeleteAccount.id);
        return next;
      });
      setIsAllSelected(false);
      setConfirmDeleteAccountId(null);
      toast.success(accountDeleteDirectoryChecked ? '\u6210\u529f\u5220\u9664\u8d26\u53f7\u53ca\u5bf9\u5e94\u76ee\u5f55' : '\u6210\u529f\u5220\u9664\u8d26\u53f7');
    } catch (error) {
      console.error('Failed to delete account directory:', error);
      toast.error(`\u76ee\u5f55\u5220\u9664\u5931\u8d25\uff0c\u672a\u5220\u9664\u8d26\u53f7: ${getErrorMessage(error)}`);
    } finally {
      setIsDeleting(false);
    }
  };

  const handleDeleteAccountCancel = () => {
    setConfirmDeleteAccountId(null);
    setAccountDeleteDirectoryChecked(true);
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

  const handleDeleteRoleClick = (accountId: string, roleId: string) => {
    setRoleDeleteDirectoryChecked(true);
    setConfirmDeleteRole({ accountId, roleId });
  };

  const handleDeleteRoleConfirm = async () => {
    if (!pendingDeleteRole || isDeleting) return;

    const gameDirectory = roleDeleteDirectoryChecked ? getConfiguredGameDirectory() : null;
    if (roleDeleteDirectoryChecked && !gameDirectory) return;

    setIsDeleting(true);
    try {
      if (gameDirectory) {
        await deleteRoleDirectory(gameDirectory, pendingDeleteRole.account.accountName, pendingDeleteRole.role);
      }

      setAccounts(prev => prev.map(account => {
        if (account.id === pendingDeleteRole.account.id) {
          return {
            ...account,
            roles: account.roles.filter(role => role.id !== pendingDeleteRole.role.id)
          };
        }
        return account;
      }));
      setConfirmDeleteRole(null);
      toast.success(roleDeleteDirectoryChecked ? '\u6210\u529f\u5220\u9664\u89d2\u8272\u53ca\u5bf9\u5e94\u76ee\u5f55' : '\u6210\u529f\u5220\u9664\u89d2\u8272');
    } catch (error) {
      console.error('Failed to delete role directory:', error);
      toast.error(`\u76ee\u5f55\u5220\u9664\u5931\u8d25\uff0c\u672a\u5220\u9664\u89d2\u8272: ${getErrorMessage(error)}`);
    } finally {
      setIsDeleting(false);
    }
  };

  const handleDeleteRoleCancel = () => {
    setConfirmDeleteRole(null);
    setRoleDeleteDirectoryChecked(true);
  };

  useEffect(() => {
    if (!isAccountDragActive) {
      return;
    }

    const previousUserSelect = document.body.style.userSelect;
    const previousCursor = document.body.style.cursor;
    document.body.style.userSelect = 'none';
    document.body.style.cursor = 'grabbing';

    return () => {
      document.body.style.userSelect = previousUserSelect;
      document.body.style.cursor = previousCursor;
    };
  }, [isAccountDragActive]);

  useLayoutEffect(() => {
    const nextPositions = new Map<string, number>();

    filteredAccounts.forEach(account => {
      const cardElement = accountCardRefs.current.get(account.id);
      if (!cardElement) {
        return;
      }

      const isDraggedCard = isAccountDragActiveRef.current && draggedAccountIdRef.current === account.id;
      const currentRectTop = cardElement.getBoundingClientRect().top;
      const currentTop = isDraggedCard ? currentRectTop - dragOffsetYRef.current : currentRectTop;
      nextPositions.set(account.id, currentTop);

      if (isDraggedCard) {
        return;
      }

      const previousTop = previousAccountPositions.current.get(account.id);
      if (previousTop === undefined) {
        return;
      }

      const deltaY = previousTop - currentTop;
      if (Math.abs(deltaY) < 1) {
        return;
      }

      cardElement.style.transition = 'none';
      cardElement.style.transform = `translate3d(0, ${deltaY}px, 0)`;

      requestAnimationFrame(() => {
        const duration = getAccountReorderAnimationDuration(deltaY);
        cardElement.style.transition = `transform ${duration}ms ${ACCOUNT_REORDER_EASING}`;
        cardElement.style.transform = '';
      });
    });

    previousAccountPositions.current = nextPositions;

    const latestPointer = latestDragPointerRef.current;
    if (isAccountDragActiveRef.current && latestPointer) {
      syncDraggedCardOffset(latestPointer.clientY);
    }
  }, [filteredAccounts]);

  const syncDraggedCardOffset = (clientY: number) => {
    const draggedId = draggedAccountIdRef.current;
    if (!draggedId) {
      return;
    }

    const draggedCard = accountCardRefs.current.get(draggedId);
    if (!draggedCard) {
      return;
    }

    const currentRectTop = draggedCard.getBoundingClientRect().top;
    const baseTop = currentRectTop - dragOffsetYRef.current;
    const nextOffsetY = clientY - baseTop - dragAnchorOffsetYRef.current;

    if (Math.abs(nextOffsetY - dragOffsetYRef.current) < 0.5) {
      return;
    }

    dragOffsetYRef.current = nextOffsetY;
    
    // 直接变异 DOM 以跳过 React 的重渲染（消除拖动顿挫感）
    draggedCard.style.transform = `translate3d(0, ${nextOffsetY}px, 0) scale(1.02)`;
  };

  const scheduleAccountDragFrame = () => {
    if (dragPreviewFrameRef.current !== null) {
      return;
    }

    dragPreviewFrameRef.current = requestAnimationFrame(() => {
      dragPreviewFrameRef.current = null;

      const pointer = pendingDragPointerRef.current;
      if (!pointer || !isAccountDragActiveRef.current) {
        return;
      }

      syncDraggedCardOffset(pointer.clientY);
      updateDragPreviewTarget(pointer.clientX, pointer.clientY);
    });
  };

  const resetAccountDragState = () => {
    const pendingDrag = pendingAccountDragRef.current;
    if (pendingDrag?.sourceElement.hasPointerCapture?.(pendingDrag.pointerId)) {
      pendingDrag.sourceElement.releasePointerCapture(pendingDrag.pointerId);
    }

    if (dragPreviewFrameRef.current !== null) {
      cancelAnimationFrame(dragPreviewFrameRef.current);
      dragPreviewFrameRef.current = null;
    }

    // 释放直接附加的 Transform，让其回推给正常布局或 FLIP 动画接管
    const draggedId = draggedAccountIdRef.current;
    if (draggedId) {
      const draggedCard = accountCardRefs.current.get(draggedId);
      if (draggedCard) {
        draggedCard.style.transform = '';
      }
    }

    pendingAccountDragRef.current = null;
    pendingDragPointerRef.current = null;
    latestDragPointerRef.current = null;
    dragPreviewAccountsRef.current = null;
    draggedAccountIdRef.current = null;
    dragOverAccountIdRef.current = null;
    isAccountDragActiveRef.current = false;
    dragAnchorOffsetYRef.current = 0;
    dragOffsetYRef.current = 0;

    setDragPreviewAccounts(null);
    setDraggedAccountId(null);
    setDragOverAccountId(null);
    setIsAccountDragActive(false);
  };

  const updateDragPreviewTarget = (clientX: number, clientY: number) => {
    const draggedId = draggedAccountIdRef.current;
    const currentPreview = dragPreviewAccountsRef.current;
    if (!draggedId || !currentPreview) {
      return;
    }

    const hoveredElement = document.elementFromPoint(clientX, clientY);
    const targetCard = hoveredElement instanceof HTMLElement
      ? hoveredElement.closest<HTMLElement>('[data-account-card-id]')
      : null;
    const targetAccountId = targetCard?.dataset.accountCardId;

    if (!targetAccountId || targetAccountId === draggedId || targetAccountId === dragOverAccountIdRef.current) {
      return;
    }

    const nextPreviewAccounts = reorderAccounts(currentPreview, draggedId, targetAccountId);
    dragPreviewAccountsRef.current = nextPreviewAccounts;
    dragOverAccountIdRef.current = targetAccountId;
    setDragPreviewAccounts(nextPreviewAccounts);
    setDragOverAccountId(targetAccountId);
  };

  const handleAccountHeaderPointerDown = (event: React.PointerEvent<HTMLDivElement>, accountId: string) => {
    if (event.button !== 0 || !canStartAccountDrag(event.target)) {
      return;
    }

    pendingAccountDragRef.current = {
      accountId,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      anchorOffsetY: event.clientY - event.currentTarget.getBoundingClientRect().top,
      sourceElement: event.currentTarget,
    };

    pendingDragPointerRef.current = {
      clientX: event.clientX,
      clientY: event.clientY,
    };
    latestDragPointerRef.current = pendingDragPointerRef.current;
  };

  const handleAccountPointerMoveInternal = (event: AccountDragPointerPayload) => {
    const pendingDrag = pendingAccountDragRef.current;
    if (!pendingDrag || pendingDrag.pointerId !== event.pointerId) {
      return;
    }

    const nextPointerPosition = {
      clientX: event.clientX,
      clientY: event.clientY,
    };
    pendingDragPointerRef.current = nextPointerPosition;
    latestDragPointerRef.current = nextPointerPosition;

    const movedDistance = Math.hypot(event.clientX - pendingDrag.startX, event.clientY - pendingDrag.startY);

    if (!isAccountDragActiveRef.current) {
      if (movedDistance < ACCOUNT_DRAG_START_DISTANCE) {
        return;
      }

      const initialPreviewAccounts = [...latestAccountsRef.current];
      dragPreviewAccountsRef.current = initialPreviewAccounts;
      draggedAccountIdRef.current = pendingDrag.accountId;
      dragOverAccountIdRef.current = pendingDrag.accountId;
      isAccountDragActiveRef.current = true;
      dragAnchorOffsetYRef.current = pendingDrag.anchorOffsetY;
      dragOffsetYRef.current = 0;
      suppressNextAccountClickRef.current = true;

      setDragPreviewAccounts(initialPreviewAccounts);
      setDraggedAccountId(pendingDrag.accountId);
      setDragOverAccountId(pendingDrag.accountId);
      setIsAccountDragActive(true);
    }

    syncDraggedCardOffset(event.clientY);
    scheduleAccountDragFrame();
  };

  const finalizeAccountDrag = () => {
    const latestPointer = pendingDragPointerRef.current;
    if (isAccountDragActiveRef.current && latestPointer) {
      updateDragPreviewTarget(latestPointer.clientX, latestPointer.clientY);
    }

    const previewAccounts = dragPreviewAccountsRef.current;
    const currentAccounts = latestAccountsRef.current;
    const hasPreviewOrderChange = Array.isArray(previewAccounts)
      && previewAccounts.length === currentAccounts.length
      && previewAccounts.some((account, index) => account.id !== currentAccounts[index]?.id);

    if (isAccountDragActiveRef.current && previewAccounts && hasPreviewOrderChange) {
      latestSetAccountsRef.current(previewAccounts);
      if (!latestSearchTermRef.current.trim()) {
        toast.success('账号顺序已更新');
      } else {
        toast.success('已按当前拖拽结果更新账号顺序');
      }
    }

    resetAccountDragState();
  };

  const handleAccountPointerUpInternal = (event: AccountDragPointerPayload) => {
    const pendingDrag = pendingAccountDragRef.current;
    if (!pendingDrag || pendingDrag.pointerId !== event.pointerId) {
      return;
    }

    finalizeAccountDrag();
  };

  const handleAccountPointerCancelInternal = (event: AccountDragPointerPayload) => {
    const pendingDrag = pendingAccountDragRef.current;
    if (!pendingDrag || pendingDrag.pointerId !== event.pointerId) {
      return;
    }

    resetAccountDragState();
  };

  const handleAccountHeaderClick = (accountId: string) => {
    if (suppressNextAccountClickRef.current) {
      suppressNextAccountClickRef.current = false;
      return;
    }

    toggleAccountExpansion(accountId);
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
              data-page-search-input="true"
              placeholder="搜索账号或角色"
              value={searchTerm}
              onChange={e => handleSearchChange(e.target.value)}
              className="pl-9 pr-8 py-1.5 w-full sm:w-48 bg-surface border border-base rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary placeholder:text-muted text-main transition-all"
            />
            {searchTerm && (
              <button
                onClick={clearSearch}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted hover:text-primary p-1 rounded-md hover:bg-base/80 transition-colors"
              >
                <X className="w-3 h-3" />
              </button>
            )}
          </div>

          {/* 筛选结果提示 */}
          {(searchTerm || accountTypeFilter !== 'all') && filteredAccounts.length > 0 && (
            <div className="flex items-center text-sm text-muted">
              {filteredAccounts.length === safeAccounts.length && accountTypeFilter === 'all' && !searchTerm ? (
                <>
                  共 <span className="font-medium text-main mx-1">{safeAccounts.length}</span> 个账户
                </>
              ) : (
                <>
                  找到 <span className="font-medium text-emerald-600 mx-1">{filteredAccounts.length}</span> 个匹配
                </>
              )}
            </div>
          )}

          {/* 原账号类型筛选区域，移至全选栏 */}

          {/* 使用配置目录解析按钮 */}
          {config?.game?.gameDirectory && (
            <button
              onClick={handleUseConfigDirectory}
              className="bg-surface border border-base text-emerald-600 hover:border-emerald-500 hover:text-emerald-700 hover:bg-emerald-50 active:scale-[0.98] px-3 py-1.5 rounded-lg flex items-center gap-2 transition-all text-sm font-medium shadow-sm"
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
            className="bg-primary hover:bg-primary-hover text-white px-3 py-1.5 rounded-lg flex items-center gap-2 transition-all shadow-sm hover:shadow active:scale-[0.98] text-sm font-medium duration-200"
          >
            <Plus className="w-4 h-4" /> 新增账号
          </button>

          {/* 批量删除按钮 */}
          {selectedAccounts.size > 0 && (
            <button
              onClick={handleBatchDeleteClick}
              className="bg-surface text-red-600 border border-red-200 hover:bg-red-50 hover:border-red-300 active:scale-[0.98] px-3 py-1.5 rounded-lg flex items-center gap-2 transition-all text-sm font-medium shadow-sm"
            >
              <Trash2 className="w-4 h-4" />
              删除 ({selectedAccounts.size})
            </button>
          )}
        </div>
      </div>


      {/* 列表控制栏：全选与筛选 */}
      {
        safeAccounts.length > 0 && (
          <div className="flex flex-col gap-3 px-1 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-center gap-4">
              {filteredAccounts.length > 0 && (
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={isAllSelected}
                    onChange={handleSelectAll}
                    className="w-4 h-4 text-primary border-base rounded focus:ring-primary"
                  />
                  <label className="text-sm font-medium text-main">
                    全选 ({filteredAccounts.length} 个账户)
                    {accountTypeFilter !== 'all' && filteredAccounts.length !== safeAccounts.length && (
                      <span className="text-muted ml-1">
                        / 共 {safeAccounts.length}
                      </span>
                    )}
                  </label>
                </div>
              )}
              
              {/* 账号类型筛选 */}
              <div className="flex items-center gap-1 bg-base rounded-lg p-0.5 border border-base">
                <button
                  onClick={() => setAccountTypeFilter('all')}
                  className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
                    accountTypeFilter === 'all'
                      ? 'bg-surface text-primary shadow-sm'
                      : 'text-muted hover:text-main'
                  }`}
                >
                  全部
                </button>
                <button
                  onClick={() => setAccountTypeFilter('own')}
                  className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
                    accountTypeFilter === 'own'
                      ? 'bg-surface text-primary shadow-sm'
                      : 'text-muted hover:text-main'
                  }`}
                >
                  本人
                </button>
                <button
                  onClick={() => setAccountTypeFilter('client')}
                  className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
                    accountTypeFilter === 'client'
                      ? 'bg-surface text-primary shadow-sm'
                      : 'text-muted hover:text-main'
                  }`}
                >
                  代清
                </button>
              </div>
            </div>

            {filteredAccounts.length > 0 && safeAccounts.length > 1 && (
              <span className="text-xs text-muted">
                拖动账号栏即可调整显示顺序
              </span>
            )}
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



      {/* 无筛选结果提示 */}
      {
        filteredAccounts.length === 0 && safeAccounts.length > 0 && (
          <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-8 text-center">
            <Search className="w-12 h-12 text-slate-200 mx-auto mb-3" />
            <p className="text-slate-500 mb-2">未找到匹配的结果</p>
            <p className="text-sm text-slate-400">
              {accountTypeFilter !== 'all' && (
                <>
                  已筛选{accountTypeFilter === 'own' ? '本人' : '代清'}账号
                  {searchTerm && (
                    <>
                      ，搜索 "<span className="font-medium text-slate-600">{searchTerm}</span>"
                    </>
                  )}
                </>
              )}
              {accountTypeFilter === 'all' && searchTerm && (
                <>
                  搜索关键词: <span className="font-medium text-slate-600">"{searchTerm}"</span>
                </>
              )}
            </p>
            <button
              onClick={() => {
                if (accountTypeFilter !== 'all') {
                  setAccountTypeFilter('all');
                }
                if (searchTerm) {
                  clearSearch();
                }
              }}
              className="mt-4 px-4 py-2 bg-surface border border-base hover:border-primary hover:text-primary hover:bg-base/50 active:scale-[0.98] rounded-lg transition-all text-sm font-medium"
            >
              {accountTypeFilter !== 'all' && searchTerm ? '清除筛选和搜索' : accountTypeFilter !== 'all' ? '清除筛选' : '清除搜索'}
            </button>
          </div>
        )
      }

      <div className="space-y-6">
        {filteredAccounts.map(account => {
          const clientNote = getClientAccountNote(account.type, account.notes);
          const isExpanded = expandedAccountIds.has(account.id);
          const isDragging = isAccountDragActive && draggedAccountId === account.id;
          const isDragTarget = isAccountDragActive && dragOverAccountId === account.id && draggedAccountId !== account.id;

          const cardStyle: React.CSSProperties | undefined = isDragging
            ? {
              transformOrigin: 'top center',
              transition: `background-color 180ms ${ACCOUNT_DRAG_SURFACE_EASING}, border-color 180ms ${ACCOUNT_DRAG_SURFACE_EASING}, box-shadow 180ms ${ACCOUNT_DRAG_SURFACE_EASING}, opacity 180ms ${ACCOUNT_DRAG_SURFACE_EASING}, backdrop-filter 180ms ${ACCOUNT_DRAG_SURFACE_EASING}`,
              willChange: 'transform',
              pointerEvents: 'none',
              zIndex: 30,
            }
            : undefined;
          return (
            <div
              key={account.id}
              data-account-card-id={account.id}
              ref={node => {
                if (node) {
                  accountCardRefs.current.set(account.id, node);
                } else {
                  accountCardRefs.current.delete(account.id);
                }
              }}
              style={cardStyle}
              className={`relative bg-surface rounded-lg border transition-[background-color,border-color,box-shadow,opacity,backdrop-filter] duration-200 ease-out ${isExpanded ? 'ring-1 ring-primary/20 shadow-sm' : 'hover:border-primary/30'} ${account.disabled ? 'opacity-60' : ''} ${isDragging ? 'z-30 border-primary/40 bg-white/90 dark:bg-slate-800/90 backdrop-blur-sm shadow-md' : 'border-base'} ${isDragTarget ? '' : ''}`}
            >
              {/* 可点击的头部区域 */}
              <div
                className={`flex items-center justify-between p-4 select-none transition-colors ${isDragging ? 'cursor-grabbing' : 'cursor-grab'} ${isExpanded ? 'bg-base/50' : 'hover:bg-base/30'}`}
                onClick={() => handleAccountHeaderClick(account.id)}
                onPointerDown={(event) => handleAccountHeaderPointerDown(event, account.id)}
              >
                <div className="flex items-center gap-3 overflow-hidden">
                  {/* 展开/折叠图标 */}
                  <div className="text-muted shrink-0 transition-transform duration-200">
                    {isExpanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
                  </div>

                  {/* 复选框 - 阻止冒泡 */}
                  <div onClick={e => e.stopPropagation()} className="flex items-center" data-no-account-drag="true">
                    <input
                      type="checkbox"
                      checked={selectedAccounts.has(account.id)}
                      onChange={() => handleSelectAccount(account.id)}
                      className="w-4 h-4 text-primary border-base rounded focus:ring-primary cursor-pointer"
                    />
                  </div>

                  {/* 账号类型图标 */}
                  <span className={`p-2 rounded-xl shrink-0 border ${account.type === AccountType.OWN ? 'bg-primary/5 text-primary border-primary/10' : 'bg-emerald-50 text-emerald-600 border-emerald-100'}`}>
                    {account.type === AccountType.OWN ? <User size={16} /> : <UserCheck size={16} />}
                  </span>

                  {/* 账号信息概要 */}
                  <div className="min-w-0 flex flex-col">
                    <div className="flex items-center gap-2 min-w-0">
                      <h3 className={`text-[1rem] font-semibold text-main truncate ${account.disabled ? 'line-through text-muted' : ''}`}>{account.accountName}</h3>
                      {account.type === AccountType.CLIENT && (
                        <span className="text-[10px] font-medium text-emerald-700 bg-emerald-50 px-1.5 py-0.5 rounded-full border border-emerald-100 shrink-0">代清</span>
                      )}
                      {account.disabled && (
                        <span className="text-[10px] font-medium text-amber-700 bg-amber-50 px-1.5 py-0.5 rounded-full border border-amber-100 shrink-0">已禁用</span>
                      )}
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          copyUsername(account.username || account.accountName, account.id);
                        }}
                        data-no-account-drag="true"
                        className={`p-1.5 rounded-lg transition-colors shrink-0 ${copyUsernameSuccess === account.id ? 'text-emerald-600 bg-emerald-50' : 'text-muted/60 hover:text-primary hover:bg-base'}`}
                        title="复制账号"
                      >
                        {copyUsernameSuccess === account.id ? <Check size={14} /> : <Clipboard size={14} />}
                      </button>
                      {account.password && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            copyPassword(account.password!, account.id);
                          }}
                          data-no-account-drag="true"
                          className={`p-1.5 rounded-lg transition-colors shrink-0 ${copySuccess === account.id ? 'text-emerald-600 bg-emerald-50' : 'text-muted/60 hover:text-primary hover:bg-base'}`}
                          title="复制密码"
                        >
                          {copySuccess === account.id ? <Check size={14} /> : <Key size={14} />}
                        </button>
                      )}
                    </div>
                    {!isExpanded && (
                      <div className="flex items-center gap-1.5 mt-1 min-w-0 text-xs text-muted">
                        <span className="shrink-0">{Array.isArray(account.roles) ? account.roles.length : 0} 个角色</span>
                        {account.username && account.username !== account.accountName && (
                          <>
                            <span className="text-muted/50 shrink-0">·</span>
                            <span className="truncate">{account.username}</span>
                          </>
                        )}
                        {clientNote && (
                          <>
                            <span className="text-muted/50 shrink-0">·</span>
                            <span className="truncate text-emerald-700">备注：{clientNote}</span>
                          </>
                        )}
                      </div>
                    )}
                  </div>
                </div>

                {/* 操作按钮区域 - 阻止冒泡 */}
                <div className="flex gap-1 shrink-0 ml-2" onClick={e => e.stopPropagation()} data-no-account-drag="true">
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
                    className={`p-2 rounded-xl transition-all active:scale-95 duration-200 ${account.type === AccountType.CLIENT ? 'text-emerald-600 bg-emerald-50 hover:bg-emerald-100' : 'text-muted hover:text-primary hover:bg-base/80'}`}
                    title={account.type === AccountType.CLIENT ? '取消代清' : '标记代清'}
                  >
                    {account.type === AccountType.CLIENT ? <UserCheck size={16} /> : <User size={16} />}
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
                    className={`p-2 rounded-xl transition-all active:scale-95 duration-200 ${account.disabled ? 'text-amber-600 bg-amber-50 hover:bg-amber-100' : 'text-emerald-600 bg-emerald-50 hover:bg-emerald-100'}`}
                    title={account.disabled ? '启用账号' : '禁用账号'}
                  >
                    {account.disabled ? <XCircle size={16} /> : <CheckCircle2 size={16} />}
                  </button>
                  <button
                    onClick={() => handleDeleteAccountClick(account.id)}
                    className="p-2 rounded-xl text-muted hover:text-red-600 hover:bg-red-50 active:scale-95 transition-all duration-200"
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>

              {/* 折叠内容区域 */}
              {isExpanded && (
                <div className="p-4 border-t border-base animate-in slide-in-from-top-2 duration-200 fade-in cursor-default">
                  <div className="space-y-4">
                  {/* 账号信息编辑区域 */}
                  <div className="bg-surface rounded-xl border border-base p-4">
                    <div className="flex items-start justify-between gap-3 mb-4">
                      <div className="flex items-start gap-3 min-w-0">
                        <span className="w-9 h-9 rounded-xl bg-base/60 border border-base flex items-center justify-center shrink-0">
                          <User className="w-4 h-4 text-primary" />
                        </span>
                        <div className="min-w-0">
                          <h4 className="text-[1rem] font-semibold text-main">账号信息</h4>
                        </div>
                      </div>
                      <span className="text-[11px] text-muted bg-base/60 border border-base rounded-full px-2.5 py-1 shrink-0">
                        {account.type === AccountType.CLIENT ? '代清账号' : '本人账号'}
                      </span>
                    </div>

                    <div className="space-y-4">


                      {/* 密码 */}
                      <div className="flex items-start gap-4">
                        <label className="text-sm font-medium text-muted w-20 flex-shrink-0 pt-3">
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
                              className="w-full px-4 py-3 pr-10 border border-base bg-base/40 rounded-lg text-main text-sm focus:ring-1 focus:ring-primary focus:border-primary transition-all placeholder:text-muted"
                              placeholder="输入游戏密码"
                            />
                            <button
                              onClick={() => togglePasswordVisibility(account.id)}
                              className="absolute right-2 top-1/2 -translate-y-1/2 p-1.5 text-muted hover:text-primary transition-colors active:scale-95 rounded-lg"
                              title={visiblePasswords.has(account.id) ? '隐藏密码' : '显示密码'}
                            >
                              {visiblePasswords.has(account.id) ? <EyeOff size={16} /> : <Eye size={16} />}
                            </button>
                          </div>
                        </div>
                      </div>

                      <div className="flex items-start gap-4">
                        <label className="text-sm font-medium text-muted w-20 flex-shrink-0 pt-3">
                          备注
                        </label>
                        <div className="flex-1 relative">
                          <FileText className="absolute left-3 top-3.5 w-4 h-4 text-muted" />
                          <textarea
                            value={account.notes || ''}
                            onChange={(e) => {
                              setAccounts(prev => prev.map(a => {
                                if (a.id === account.id) {
                                  return { ...a, notes: e.target.value };
                                }
                                return a;
                              }));
                            }}
                            className="w-full min-h-[88px] pl-10 pr-4 py-3 border border-base bg-base/40 rounded-lg text-main text-sm leading-6 focus:ring-1 focus:ring-primary focus:border-primary transition-all resize-y placeholder:text-muted"
                            placeholder={account.type === AccountType.CLIENT ? '可填写老板、代清要求等备注' : '可填写账号备注'}
                          />
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Roles Section */}
                  <div className="bg-surface rounded-xl border border-base p-4">
                    <div className="flex justify-between items-start gap-3 mb-4">
                      <div className="flex items-start gap-3 min-w-0">
                        <span className="w-9 h-9 rounded-xl bg-base/60 border border-base flex items-center justify-center shrink-0">
                          <User className="w-4 h-4 text-primary" />
                        </span>
                        <div className="min-w-0">
                          <h4 className="text-[1rem] font-semibold text-main">角色列表</h4>
                          <p className="text-xs text-muted mt-1">
                            共 {Array.isArray(account.roles) ? account.roles.length : 0} 个角色，可管理参与玩法和启用状态
                          </p>
                        </div>
                      </div>
                      <button
                        onClick={() => setAddingRoleToAccountId(account.id)}
                        className="flex items-center gap-1.5 bg-base/60 hover:bg-base border border-base hover:border-primary/50 text-main hover:text-primary px-3 py-2 rounded-lg transition-all active:scale-[0.98] text-sm font-medium shrink-0"
                      >
                        <Plus className="w-4 h-4" />
                        添加角色
                      </button>
                    </div>

                    <div className="space-y-3">
                      {!Array.isArray(account.roles) || account.roles.length === 0 ? (
                        <div className="text-center py-6 text-muted bg-base/30 rounded-lg border border-dashed border-base">
                          <p className="flex items-center justify-center gap-2">
                            <User className="w-5 h-5" />
                            暂无角色，点击上方按钮添加
                          </p>
                        </div>
                      ) : (
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                          {account.roles.map(role => (
                            <div key={role.id} className={`bg-base/30 border border-base px-3.5 py-3 rounded-lg transition-colors ${role.disabled ? 'opacity-60' : 'hover:border-primary/30 hover:bg-base/50'}`}>
                              <div className="flex justify-between items-start gap-3">
                                <div className="min-w-0 flex-1">
                                  <div className="flex flex-wrap items-center gap-2">
                                    <span className={`text-sm font-medium text-main truncate ${role.disabled ? 'line-through text-muted' : ''}`}>
                                      {role.name}·{getBaseServerName(role.server)}
                                    </span>
                                    {role.sect && (
                                      <SectIcon sectName={role.sect} variant="short" />
                                    )}
                                    {role.equipmentScore !== undefined && role.equipmentScore !== null && (
                                      <span className="text-[11px] bg-blue-50 text-blue-700 border border-blue-100 px-2 py-0.5 rounded-md font-medium">
                                        装分 {role.equipmentScore.toLocaleString()}
                                      </span>
                                    )}
                                  </div>
                                  {role.disabled && (
                                    <p className="text-xs text-muted mt-2">
                                      该角色当前已禁用
                                    </p>
                                  )}
                                </div>
                                <div className="flex gap-1 shrink-0">
                                  <button
                                    onClick={() => handleOpenEditRoleModal(account.id, role)}
                                    className="p-1.5 rounded-lg text-muted hover:text-primary transition-all duration-200 active:scale-95 hover:bg-surface"
                                    title="修改角色信息"
                                  >
                                    <Settings size={15} />
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
                                    className={`p-1.5 rounded-lg transition-all active:scale-95 duration-200 ${role.disabled ? 'text-amber-600 bg-amber-50 hover:bg-amber-100' : 'text-emerald-600 bg-emerald-50 hover:bg-emerald-100'}`}
                                    title={role.disabled ? '启用角色' : '禁用角色'}
                                  >
                                    {role.disabled ? <XCircle size={15} /> : <CheckCircle2 size={15} />}
                                  </button>
                                  <button
                                    onClick={() => handleDeleteRoleClick(account.id, role.id)}
                                    className="p-1.5 rounded-lg text-muted hover:text-red-600 hover:bg-red-50 active:scale-95 transition-all duration-200"
                                    title="删除角色"
                                  >
                                    <Trash2 size={15} />
                                  </button>
                                </div>
                              </div>

                              <div className="mt-3 pt-3 border-t border-base/80 flex items-start gap-3">
                                <span className="text-xs font-medium text-muted shrink-0 pt-1">
                                  玩法
                                </span>
                                <div className="flex flex-wrap gap-2 flex-1">
                                  {instanceTypes.map(type => {
                                    const isVisible = role.visibility?.[type.type] !== false;
                                    const displayName = type.name === '团队副本' ? '副本' : type.name === '试炼之地' ? '试炼' : type.name;
                                    return (
                                      <button
                                        key={type.id}
                                        onClick={() => !role.disabled && handleToggleVisibility(role.id, type.type, isVisible)}
                                        disabled={role.disabled}
                                        className={`px-2.5 py-1 text-[11px] font-medium rounded-md border cursor-pointer transition-all min-w-[58px] text-center ${role.disabled
                                          ? 'bg-base text-muted border-base opacity-40 cursor-not-allowed'
                                          : isVisible
                                            ? 'bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-900/30 dark:text-emerald-400 dark:border-emerald-800'
                                            : 'bg-base/60 text-muted border-base hover:bg-base/80'
                                          }`}
                                        title={isVisible ? `在${type.name}中显示 (点击隐藏)` : `在${type.name}中隐藏 (点击显示)`}
                                      >
                                        {displayName}
                                      </button>
                                    );
                                  })}
                                </div>
                              </div>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
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
              <h3 className="text-lg font-semibold text-main mb-4">{'\u786e\u8ba4\u5220\u9664'}</h3>
              <p className="text-slate-600 mb-4">{'\u786e\u8ba4\u5220\u9664\u9009\u4e2d\u7684 '}{selectedAccounts.size}{' \u4e2a\u8d26\u53f7\uff1f\u5173\u8054\u7684\u89d2\u8272\u548c\u526f\u672c\u8bb0\u5f55\u4e0d\u4f1a\u88ab\u5220\u9664\u3002'}</p>
              <label className="mb-6 flex items-start gap-3 rounded-lg border border-base bg-base/40 px-3 py-2 text-sm text-main">
                <input
                  type="checkbox"
                  checked={batchDeleteDirectoryChecked}
                  onChange={(event) => setBatchDeleteDirectoryChecked(event.target.checked)}
                  disabled={isDeleting}
                  className="mt-0.5 h-4 w-4 rounded border-base text-primary focus:ring-primary disabled:cursor-not-allowed"
                />
                <span>{'\u540c\u65f6\u5220\u9664\u5bf9\u5e94\u8d26\u53f7\u76ee\u5f55'}</span>
              </label>
              <div className="flex justify-end gap-3">
                <button
                  onClick={handleBatchDeleteCancel}
                  disabled={isDeleting}
                  className="px-4 py-2 bg-surface hover:bg-base border border-base text-main hover:border-primary hover:text-primary active:scale-[0.98] rounded-lg transition-all duration-200 font-medium disabled:opacity-60 disabled:cursor-not-allowed"
                >
                  {'\u53d6\u6d88'}
                </button>
                <button
                  onClick={handleBatchDeleteConfirm}
                  disabled={isDeleting}
                  className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white shadow-sm hover:shadow active:scale-[0.98] rounded-lg transition-all duration-200 font-medium disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-2"
                >
                  {isDeleting && <Loader2 className="w-4 h-4 animate-spin" />}
                  {isDeleting ? '\u5220\u9664\u4e2d...' : '\u786e\u5b9a'}
                </button>
              </div>
            </div>
          </div>
        )
      }

      {/* Single Account Delete Confirmation Dialog */}
      {
        pendingDeleteAccount && (
          <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50">
            <div className="bg-surface p-6 rounded-lg shadow-xl border border-base max-w-sm w-full mx-4">
              <h3 className="text-lg font-semibold text-main mb-4">{'\u786e\u8ba4\u5220\u9664'}</h3>
              <p className="text-slate-600 mb-4">{'\u786e\u8ba4\u5220\u9664\u8d26\u53f7\u201c'}{pendingDeleteAccount.accountName}{'\u201d\uff1f\u5173\u8054\u7684\u89d2\u8272\u548c\u526f\u672c\u8bb0\u5f55\u4e0d\u4f1a\u88ab\u5220\u9664\u3002'}</p>
              <label className="mb-6 flex items-start gap-3 rounded-lg border border-base bg-base/40 px-3 py-2 text-sm text-main">
                <input
                  type="checkbox"
                  checked={accountDeleteDirectoryChecked}
                  onChange={(event) => setAccountDeleteDirectoryChecked(event.target.checked)}
                  disabled={isDeleting}
                  className="mt-0.5 h-4 w-4 rounded border-base text-primary focus:ring-primary disabled:cursor-not-allowed"
                />
                <span>{'\u540c\u65f6\u5220\u9664\u5bf9\u5e94\u8d26\u53f7\u76ee\u5f55'}</span>
              </label>
              <div className="flex justify-end gap-3">
                <button
                  onClick={handleDeleteAccountCancel}
                  disabled={isDeleting}
                  className="px-4 py-2 bg-surface hover:bg-base border border-base text-main hover:border-primary hover:text-primary active:scale-[0.98] rounded-lg transition-all duration-200 font-medium disabled:opacity-60 disabled:cursor-not-allowed"
                >
                  {'\u53d6\u6d88'}
                </button>
                <button
                  onClick={handleDeleteAccountConfirm}
                  disabled={isDeleting}
                  className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white shadow-sm hover:shadow active:scale-[0.98] rounded-lg transition-all duration-200 font-medium disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-2"
                >
                  {isDeleting && <Loader2 className="w-4 h-4 animate-spin" />}
                  {isDeleting ? '\u5220\u9664\u4e2d...' : '\u786e\u5b9a'}
                </button>
              </div>
            </div>
          </div>
        )
      }

      {/* Single Role Delete Confirmation Dialog */}
      {
        pendingDeleteRole && (
          <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50">
            <div className="bg-surface p-6 rounded-lg shadow-xl border border-base max-w-sm w-full mx-4">
              <h3 className="text-lg font-semibold text-main mb-4">{'\u786e\u8ba4\u5220\u9664'}</h3>
              <p className="text-slate-600 mb-4">{'\u786e\u8ba4\u5220\u9664\u89d2\u8272\u201c'}{pendingDeleteRole.role.name}{'\u201d\uff1f\u8be5\u64cd\u4f5c\u4e0d\u4f1a\u5220\u9664\u526f\u672c\u8bb0\u5f55\u3002'}</p>
              <label className="mb-6 flex items-start gap-3 rounded-lg border border-base bg-base/40 px-3 py-2 text-sm text-main">
                <input
                  type="checkbox"
                  checked={roleDeleteDirectoryChecked}
                  onChange={(event) => setRoleDeleteDirectoryChecked(event.target.checked)}
                  disabled={isDeleting}
                  className="mt-0.5 h-4 w-4 rounded border-base text-primary focus:ring-primary disabled:cursor-not-allowed"
                />
                <span>{'\u540c\u65f6\u5220\u9664\u5bf9\u5e94\u89d2\u8272\u76ee\u5f55'}</span>
              </label>
              <div className="flex justify-end gap-3">
                <button
                  onClick={handleDeleteRoleCancel}
                  disabled={isDeleting}
                  className="px-4 py-2 bg-surface hover:bg-base border border-base text-main hover:border-primary hover:text-primary active:scale-[0.98] rounded-lg transition-all duration-200 font-medium disabled:opacity-60 disabled:cursor-not-allowed"
                >
                  {'\u53d6\u6d88'}
                </button>
                <button
                  onClick={handleDeleteRoleConfirm}
                  disabled={isDeleting}
                  className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white shadow-sm hover:shadow active:scale-[0.98] rounded-lg transition-all duration-200 font-medium disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-2"
                >
                  {isDeleting && <Loader2 className="w-4 h-4 animate-spin" />}
                  {isDeleting ? '\u5220\u9664\u4e2d...' : '\u786e\u5b9a'}
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
                  className="px-4 py-2 bg-surface hover:bg-base border border-base text-main hover:border-primary hover:text-primary active:scale-[0.98] rounded-lg transition-all duration-200 font-medium"
                >
                  取消
                </button>
                <button
                  onClick={handleSaveRoleInfo}
                  className="px-4 py-2 bg-primary hover:bg-primary-hover text-white shadow-sm hover:shadow active:scale-[0.98] rounded-lg transition-all duration-200 font-medium"
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
