import React, { useState, useMemo, useEffect, useRef } from 'react';
import { Account, Raid, RaidRecord } from '../types';
import { Shield, Sword, Calendar, TrendingUp, TrendingDown, Wallet, RefreshCw, Clock, Copy, Check } from 'lucide-react';
import { AddRecordModal } from './AddRecordModal';
import { RoleRecordsModal } from './RoleRecordsModal';
import { deduplicateRecords, formatGoldAmount } from '../utils/recordUtils';
import { calculateCooldown, formatCountdown, getRaidRefreshInfo, CooldownInfo } from '../utils/cooldownManager';
import { STATIC_RAIDS } from '../data/staticRaids';
import { shouldShowClientRoleInRaid } from '../utils/raidVersionUtils';

interface RaidDetailProps {
  raid: Raid;
  accounts: Account[];
  records: RaidRecord[];
  onBack: () => void;
  setRecords?: React.Dispatch<React.SetStateAction<RaidRecord[]>>;
  onEditRecord?: (record: RaidRecord) => void;
}

interface RoleWithStatus {
  id: string;
  name: string;
  server: string;
  region: string;
  sect: string;
  equipmentScore?: number;
  accountId: string;
  accountName: string;
  password?: string;
  canRun: boolean;
  canAddMore: boolean;
  recordCount: number;
  maxRecords: number;
  cooldownInfo: CooldownInfo;
  lastRunDate?: string;
  lastRunGold?: number;
  lastRunIncome?: number;
  lastRunExpense?: number;
  cooldownDays?: number;
}

interface CountdownState {
  formatted: string;
  remainingMs: number;
  isExpired: boolean;
}

const useRaidRefreshCountdown = (raid: Raid): CountdownState => {
  const [state, setState] = useState<CountdownState>(() => {
    const refreshInfo = getRaidRefreshInfo(raid);
    if (!refreshInfo.nextRefreshTime) {
      return { formatted: '已刷新', remainingMs: 0, isExpired: true };
    }
    const remaining = Math.max(0, refreshInfo.nextRefreshTime.getTime() - Date.now());
    return {
      formatted: formatCountdown(remaining),
      remainingMs: remaining,
      isExpired: remaining <= 0
    };
  });

  useEffect(() => {
    const updateCountdown = () => {
      const refreshInfo = getRaidRefreshInfo(raid);
      if (!refreshInfo.nextRefreshTime) {
        setState({ formatted: '已刷新', remainingMs: 0, isExpired: true });
        return;
      }
      const remaining = Math.max(0, refreshInfo.nextRefreshTime.getTime() - Date.now());
      setState({
        formatted: formatCountdown(remaining),
        remainingMs: remaining,
        isExpired: remaining <= 0
      });
    };

    updateCountdown();
    const interval = setInterval(updateCountdown, 1000);
    return () => clearInterval(interval);
  }, [raid]);

  return state;
};

interface RaidRefreshCountdownProps {
  raid: Raid;
}

const RaidRefreshCountdown: React.FC<RaidRefreshCountdownProps> = ({ raid }) => {
  const countdown = useRaidRefreshCountdown(raid);

  return (
    <div className={`flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-sm font-medium font-mono ${countdown.isExpired
      ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400'
      : 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400'
      }`}>
      <RefreshCw className={`w-3.5 h-3.5 ${countdown.isExpired ? '' : 'animate-spin'}`} />
      <span>{countdown.formatted}</span>
    </div>
  );
};

export const RaidDetail: React.FC<RaidDetailProps> = ({ raid, accounts, records, onBack, setRecords, onEditRecord }) => {

  const [showAddRecordModal, setShowAddRecordModal] = useState(false);
  const [showRoleRecordsModal, setShowRoleRecordsModal] = useState(false);
  const [selectedRoleForModal, setSelectedRoleForModal] = useState<RoleWithStatus | null>(null);
  const [successToast, setSuccessToast] = useState<{ visible: boolean; message: string }>({ visible: false, message: '' });

  const [copiedField, setCopiedField] = useState<string | null>(null);
  const toastTimerRef = useRef<number | null>(null);

  const cleanRoleName = (name: string) => {
    const parts = name.trim().split(/\s+/);
    if (parts.length > 1 && parts[0] === parts[1]) {
      return parts[0];
    }
    return name;
  };



  const showToast = (message: string, duration: number = 3000) => {
    console.log('[Toast] showToast called with message:', message);

    if (toastTimerRef.current !== null) {
      toastTimerRef.current = null;
    }

    setSuccessToast({ visible: true, message });

    toastTimerRef.current = window.setTimeout(() => {
      console.log('[Toast] setTimeout callback executing');
      setSuccessToast(prev => {
        console.log('[Toast] Setting visible to false, current state:', prev);
        return { ...prev, visible: false };
      });
      toastTimerRef.current = null;
    }, duration);

    console.log('[Toast] Timer set with duration:', duration);
  };

  const copyToClipboard = async (text: string, fieldId: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedField(fieldId);
      setTimeout(() => setCopiedField(null), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  const getMaskedPassword = (password: string | undefined) => {
    if (!password) return '••••••';
    return '••••••';
  };

  const handleAddRecordClick = (role: RoleWithStatus) => {
    setSelectedRoleForModal(role);
    setShowAddRecordModal(true);
  };

  const handleViewRecordsClick = (role: RoleWithStatus) => {
    setSelectedRoleForModal(role);
    setShowRoleRecordsModal(true);
  };

  const handleAddRecord = (recordData: Partial<RaidRecord>) => {
    console.log('[RaidDetail] handleAddRecord called with:', recordData);
    if (setRecords && recordData) {
      console.log('[RaidDetail] Setting records...');
      setRecords(prev => {
        const newRecords = [recordData as RaidRecord, ...prev];
        const result = deduplicateRecords(newRecords);
        console.log('[RaidDetail] Records updated, count:', result.length);
        return result;
      });
    } else {
      console.log('[RaidDetail] setRecords is undefined or recordData is null');
    }
    console.log('[RaidDetail] Calling showToast...');
    showToast('记录添加成功', 3000);
    console.log('[RaidDetail] Closing modal...');
    setShowAddRecordModal(false);
  };

  const weekInfo = useMemo(() => {
    const now = new Date();
    const dayOfWeek = now.getDay();
    const mondayOffset = dayOfWeek === 0 ? -6 : 1 - dayOfWeek;
    const weekStart = new Date(now);
    weekStart.setDate(now.getDate() + mondayOffset);
    weekStart.setHours(0, 0, 0, 0);

    const weekEnd = new Date(weekStart);
    weekEnd.setDate(weekStart.getDate() + 6);
    weekEnd.setHours(23, 59, 59, 999);

    return {
      start: weekStart,
      end: weekEnd
    };
  }, []);

  const rolesWithStatus = useMemo(() => {
    const safeAccounts = Array.isArray(accounts) ? accounts : [];
    const safeRecords = Array.isArray(records) ? records : [];

    const roles: RoleWithStatus[] = [];
    const processedRoleIds = new Set<string>();

    const periodStartTime = weekInfo.start.getTime();
    const periodEndTime = weekInfo.end.getTime();

    const thisWeekRecords = safeRecords.filter(r => {
      const matchesName = r.raidName.includes(raid.name);
      const difficultyText = raid.difficulty === 'HEROIC' ? '英雄' : raid.difficulty === 'CHALLENGE' ? '挑战' : '普通';
      const matchesDifficulty = r.raidName.includes(difficultyText);
      const matchesPlayerCount = r.raidName.includes(`${raid.playerCount}人`);

      return matchesName && matchesDifficulty && matchesPlayerCount &&
        new Date(r.date).getTime() >= periodStartTime &&
        new Date(r.date).getTime() <= periodEndTime;
    });

    const shouldShowClientRoles = shouldShowClientRoleInRaid(raid.playerCount, raid.version || '', STATIC_RAIDS);

    thisWeekRecords.forEach(record => {
      if (processedRoleIds.has(record.roleId)) return;

      let roleName = '未知角色';
      let sect = '';
      let region = '';
      let server = record.server || '未知服务器';
      let accountName = record.accountId || '未知账号';
      let password: string | undefined;
      let isClientAccount = false;

      safeAccounts.forEach(account => {
        if (account.id === record.accountId) {
          accountName = account.accountName;
          password = account.password;
          isClientAccount = account.type === 'CLIENT';
          const role = account.roles?.find(r => r.id === record.roleId);
          if (role) {
            roleName = role.name;
            sect = role.sect || '';
            region = role.region || '';
            server = role.server;
          }
        }
      });

      if (!shouldShowClientRoles && isClientAccount) {
        return;
      }

      const roleRecords = thisWeekRecords.filter(r => r.roleId === record.roleId);

      const lastRunRecord = roleRecords.sort((a, b) =>
        new Date(b.date).getTime() - new Date(a.date).getTime()
      )[0];

      let lastRunDate = undefined;
      let lastRunGold = undefined;
      let lastRunIncome = undefined;
      let lastRunExpense = undefined;

      if (lastRunRecord) {
        lastRunDate = lastRunRecord.date;
        lastRunIncome = lastRunRecord.goldIncome;
        lastRunExpense = lastRunRecord.goldExpense || 0;
        lastRunGold = lastRunIncome - lastRunExpense;
      }

      const cooldownDays = raid.playerCount === 25 ? 7 : raid.playerCount === 10 ? 3 : 7;
      const maxRecords = raid.playerCount === 10 ? 2 : 1;
      const recordCount = roleRecords.length;
      const roleRecordDates = roleRecords.map(r => ({ date: r.date }));
      const cooldownInfo = calculateCooldown(raid, roleRecordDates);

      roles.push({
        id: record.roleId,
        name: roleName,
        server: server,
        region: region || '未知',
        sect: sect || '未知',
        accountId: record.accountId,
        accountName,
        password,
        canRun: false,
        canAddMore: cooldownInfo.canAdd,
        recordCount,
        maxRecords,
        cooldownInfo,
        lastRunDate,
        lastRunGold,
        lastRunIncome,
        lastRunExpense,
        cooldownDays
      });

      processedRoleIds.add(record.roleId);
    });

    safeAccounts.forEach(account => {
      if (account.disabled) return;

      const safeRoles = Array.isArray(account.roles) ? account.roles : [];

      safeRoles.forEach(role => {
        if (role.disabled) return;
        if (processedRoleIds.has(role.id)) return;

        if (!shouldShowClientRoles && account.type === 'CLIENT') {
          return;
        }

        const cooldownDays = raid.playerCount === 25 ? 7 : raid.playerCount === 10 ? 3 : 7;
        const maxRecords = raid.playerCount === 10 ? 2 : 1;
        const cooldownInfo = calculateCooldown(raid, []);

        roles.push({
          id: role.id,
          name: role.name,
          server: role.server,
          region: role.region,
          sect: role.sect,
          equipmentScore: role.equipmentScore,
          accountId: account.id,
          accountName: account.accountName,
          password: account.password,
          canRun: true,
          canAddMore: true,
          recordCount: 0,
          maxRecords,
          cooldownInfo,
          lastRunDate: undefined,
          lastRunGold: undefined,
          lastRunIncome: undefined,
          lastRunExpense: undefined,
          cooldownDays
        });
      });
    });

    return roles;
  }, [accounts, records, raid, weekInfo]);

  const sortedRoles = useMemo(() => {
    const sorted = [...rolesWithStatus];

    sorted.sort((a, b) => {
      // 1. Availability first (canRun)
      const aCanRun = a.canRun ? 0 : 1;
      const bCanRun = b.canRun ? 0 : 1;
      if (aCanRun !== bCanRun) return aCanRun - bCanRun;

      // 2. Equipment Score (Desc)
      const aScore = a.equipmentScore || 0;
      const bScore = b.equipmentScore || 0;
      if (aScore !== bScore) return bScore - aScore;

      // 3. Last Record Date (Desc - Newest first)
      const aDate = a.lastRunDate ? new Date(a.lastRunDate).getTime() : 0;
      const bDate = b.lastRunDate ? new Date(b.lastRunDate).getTime() : 0;
      if (aDate !== bDate) return bDate - aDate;

      // 4. Server (Asc)
      if (a.server !== b.server) return a.server.localeCompare(b.server); // Corrected property access

      // 5. Name (Asc)
      return a.name.localeCompare(b.name);
    });

    return sorted;
  }, [rolesWithStatus]);

  const availableCount = rolesWithStatus.filter(r => r.canRun).length;
  const unavailableCount = rolesWithStatus.filter(r => !r.canRun).length;

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    return date.toLocaleDateString('zh-CN', {
      month: '2-digit',
      day: '2-digit'
    });
  };

  return (
    <div className="space-y-6">
      <div className="bg-surface rounded-xl shadow-sm border border-base p-6 relative z-10">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <button
              onClick={onBack}
              className="px-4 py-2 bg-base text-main rounded-lg hover:bg-base/80 transition-colors flex items-center gap-2 text-sm font-medium"
            >
              <Sword className="w-4 h-4" />
              返回
            </button>
            <div className="flex items-center gap-3">
              <h2 className="text-xl font-bold text-main">{raid.name}</h2>
              <RaidRefreshCountdown raid={raid} />
              <span className="px-2.5 py-1 bg-base text-muted rounded-lg text-sm font-medium">
                {raid.playerCount}人 • {raid.difficulty === 'HEROIC' ? '英雄' : raid.difficulty === 'CHALLENGE' ? '挑战' : '普通'}
              </span>
            </div>
          </div>

          <div className="flex items-center gap-6">
            <div className="text-center">
              <div className="text-2xl font-bold text-emerald-600">{availableCount}</div>
              <div className="text-xs text-muted">可打</div>
            </div>
            <div className="text-center">
              <div className="text-2xl font-bold text-amber-600">{unavailableCount}</div>
              <div className="text-xs text-muted">已打</div>
            </div>
          </div>
        </div>



        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {sortedRoles.map((role) => {
            const isAtLimit = !role.cooldownInfo.canAdd;

            return (
              <div
                key={role.id}
                className={`p-4 rounded-xl border-2 transition-all duration-300 ${role.canRun
                  ? isAtLimit
                    ? 'bg-gradient-to-br from-base to-base border-base hover:shadow-md'
                    : 'bg-gradient-to-br from-emerald-50 to-teal-50 dark:from-emerald-900/10 dark:to-teal-900/10 border-emerald-200 dark:border-emerald-800 hover:shadow-lg hover:border-emerald-300'
                  : 'bg-gradient-to-br from-amber-50 to-orange-50 dark:from-amber-900/10 dark:to-orange-900/10 border-amber-200 dark:border-amber-800 hover:shadow-lg hover:border-amber-300'
                  }`}
              >
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center gap-2 flex-1 min-w-0">
                    {isAtLimit ? (
                      <div className="w-6 h-6 rounded-full bg-base flex items-center justify-center flex-shrink-0">
                        <RefreshCw className="w-4 h-4 text-muted" />
                      </div>
                    ) : role.canRun ? (
                      <div className="w-6 h-6 rounded-full bg-emerald-100 dark:bg-emerald-900/30 flex items-center justify-center flex-shrink-0">
                        <div className="w-2.5 h-2.5 rounded-full bg-emerald-500 animate-pulse" />
                      </div>
                    ) : (
                      <div className="w-6 h-6 rounded-full bg-amber-100 dark:bg-amber-900/30 flex items-center justify-center flex-shrink-0">
                        <div className="w-2.5 h-2.5 rounded-full bg-amber-500" />
                      </div>
                    )}
                    <div className="min-w-0 flex-1">
                      <div className="font-semibold text-main truncate flex items-center gap-2 flex-wrap">
                        <span className="truncate">{cleanRoleName(role.name)}@{role.server}</span>
                        {role.sect && role.sect !== '未知' && (
                          <span className={`text-xs px-2 py-1 rounded-md font-medium flex-shrink-0 ${isAtLimit
                            ? 'bg-base text-muted'
                            : role.canRun
                              ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400'
                              : 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400'
                            }`}>{role.sect}</span>
                        )}
                        {role.equipmentScore !== undefined && role.equipmentScore !== null && (
                          <span className="text-xs bg-blue-100 text-blue-700 px-2 py-1 rounded-md font-medium flex-shrink-0">
                            {role.equipmentScore.toLocaleString()}
                          </span>
                        )}
                      </div>
                    </div>
                  </div>
                </div>

                <div className="space-y-2 text-sm">


                  {role.lastRunDate ? (
                    <div className="space-y-1">
                      <div className="flex items-center gap-2 text-muted">
                        <Clock className={`w-4 h-4 flex-shrink-0 ${isAtLimit ? 'text-muted' : role.canRun ? 'text-emerald-500' : 'text-amber-500'}`} />
                        <span className="text-xs">{formatDate(role.lastRunDate)}</span>
                      </div>
                      <div className="flex items-center gap-2 ml-6">
                        {role.lastRunIncome !== undefined && role.lastRunIncome > 0 && (
                          <div className="flex items-center gap-1 text-emerald-600">
                            <TrendingUp className="w-3 h-3" />
                            <span className="text-xs font-medium">{formatGoldAmount(role.lastRunIncome)}金</span>
                          </div>
                        )}
                        {role.lastRunExpense !== undefined && role.lastRunExpense > 0 && (
                          <div className="flex items-center gap-1 text-amber-600">
                            <TrendingDown className="w-3 h-3" />
                            <span className="text-xs font-medium">{formatGoldAmount(role.lastRunExpense)}金</span>
                          </div>
                        )}
                        {role.lastRunGold !== undefined && (
                          <div className="flex items-center gap-1 text-main">
                            <Wallet className="w-3 h-3" />
                            <span className="text-xs font-medium">{formatGoldAmount(role.lastRunGold)}金</span>
                          </div>
                        )}
                      </div>
                    </div>
                  ) : (
                    <div className="flex items-center gap-2 text-muted">
                      <Calendar className={`w-4 h-4 flex-shrink-0 ${isAtLimit ? 'text-muted' : role.canRun ? 'text-emerald-400' : 'text-amber-400'}`} />
                      <span className="text-xs">暂无记录</span>
                    </div>
                  )}
                </div>

                <div className="mt-3 pt-3 border-t border-base space-y-2">
                  <div className="flex items-center gap-2">
                    <div className="text-xs text-muted flex-shrink-0">账号</div>
                    <div className="flex items-center gap-1 flex-1 min-w-0 bg-base rounded px-2 py-1">
                      <span className="text-xs text-main truncate flex-1">{role.accountName}</span>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          copyToClipboard(role.accountName, `account-${role.id}`);
                        }}
                        className={`flex-shrink-0 p-1 rounded transition-colors ${copiedField === `account-${role.id}`
                          ? 'text-emerald-600'
                          : 'text-muted hover:text-main hover:bg-surface'
                          }`}
                        title={copiedField === `account-${role.id}` ? '已复制!' : '复制账号'}
                      >
                        {copiedField === `account-${role.id}` ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                      </button>
                    </div>
                  </div>

                  <div className="flex items-center gap-2">
                    <div className="text-xs text-muted flex-shrink-0">密码</div>
                    {role.password ? (
                      <div className="flex items-center gap-1 flex-1 min-w-0 bg-base rounded px-2 py-1">
                        <span className="text-xs text-main font-mono truncate flex-1">
                          {getMaskedPassword(role.password)}
                        </span>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            copyToClipboard(role.password!, `password-${role.id}`);
                          }}
                          className={`flex-shrink-0 p-1 rounded transition-colors ${copiedField === `password-${role.id}`
                            ? 'text-emerald-600'
                            : 'text-muted hover:text-main hover:bg-surface'
                            }`}
                          title="复制密码"
                        >
                          {copiedField === `password-${role.id}` ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                        </button>
                      </div>
                    ) : (
                      <div className="flex-1 min-w-0 bg-base rounded px-2 py-1">
                        <span className="text-xs text-muted truncate flex-1">该账户未配置密码</span>
                      </div>
                    )}
                  </div>
                </div>

                <div className="mt-3 grid grid-cols-2 gap-2">
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleAddRecordClick(role);
                    }}
                    disabled={!role.canAddMore}
                    className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200 ${role.canAddMore
                      ? role.canRun
                        ? 'bg-emerald-500 text-white hover:bg-emerald-600 hover:shadow-md transform hover:-translate-y-0.5'
                        : 'bg-amber-500 text-white hover:bg-amber-600 hover:shadow-md transform hover:-translate-y-0.5'
                      : 'bg-base text-muted cursor-not-allowed border border-base'
                      }`}
                  >
                    添加记录
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleViewRecordsClick(role);
                    }}
                    className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200 ${role.canRun
                      ? 'bg-white text-emerald-700 border border-emerald-200 hover:bg-emerald-50 hover:border-emerald-300'
                      : 'bg-white text-amber-700 border border-amber-200 hover:bg-amber-50 hover:border-amber-300'
                      }`}
                  >
                    查看详情
                  </button>
                </div>
              </div>
            );
          })}
        </div>

        {sortedRoles.length === 0 && (
          <div className="text-center py-12 text-muted">
            <Shield className="w-16 h-16 mx-auto mb-4 text-muted/50" />
            <p className="text-lg">没有找到符合条件的角色</p>
            <p className="text-sm mt-2">请尝试切换排序方式</p>
          </div>
        )}
      </div>

      {showAddRecordModal && selectedRoleForModal && (
        <AddRecordModal
          isOpen={showAddRecordModal}
          onClose={() => setShowAddRecordModal(false)}
          onSubmit={handleAddRecord}
          raid={raid}
          role={selectedRoleForModal}
        />
      )}

      {successToast.visible && (
        <div className="fixed bottom-8 left-1/2 -translate-x-1/2 bg-emerald-600 text-white px-6 py-3 rounded-xl shadow-lg flex items-center gap-2 text-sm font-medium animate-in z-[9999]">
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
          </svg>
          <span>{successToast.message}</span>
        </div>
      )}

      {showRoleRecordsModal && selectedRoleForModal && (
        <RoleRecordsModal
          isOpen={showRoleRecordsModal}
          onClose={() => setShowRoleRecordsModal(false)}
          role={selectedRoleForModal}
          records={records}
          raid={raid}
          setRecords={setRecords}
          isAdmin={true}
          onEditRecord={onEditRecord}
        />
      )}
    </div>
  );
};
