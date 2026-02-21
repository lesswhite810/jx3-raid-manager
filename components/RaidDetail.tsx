import React, { useState, useMemo, useEffect, useRef } from 'react';
import { Account, Raid, RaidRecord, BossCooldownInfo } from '../types';
import { Shield, Calendar, TrendingUp, TrendingDown, RefreshCw, Clock, Copy, Check } from 'lucide-react';
import { AddRecordModal } from './AddRecordModal';
import { RoleRecordsModal } from './RoleRecordsModal';
import { BossCooldownSummary } from './BossCooldownDisplay';
import { deduplicateRecords, formatGoldAmount } from '../utils/recordUtils';
import { calculateCooldown, formatCountdown, getRaidRefreshInfo, CooldownInfo, getLastMonday7AM, getNextMonday7AM } from '../utils/cooldownManager';

import { shouldShowClientRoleInRaid } from '../utils/raidVersionUtils';
import { calculateBossCooldowns } from '../utils/bossCooldownManager';

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
  bossCooldowns?: BossCooldownInfo[];
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
    // 25人本：周一 07:00 ~ 下周一 07:00
    const weekStart = getLastMonday7AM(now);
    const weekEnd = getNextMonday7AM(now);
    return { start: weekStart, end: weekEnd };
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
      const difficultyText = raid.difficulty;
      const matchesDifficulty = r.raidName.includes(difficultyText);
      const matchesPlayerCount = r.raidName.includes(`${raid.playerCount}人`);

      return matchesName && matchesDifficulty && matchesPlayerCount &&
        new Date(r.date).getTime() >= periodStartTime &&
        new Date(r.date).getTime() <= periodEndTime;
    });

    const shouldShowClientRoles = shouldShowClientRoleInRaid(raid.playerCount, raid.version || '');

    thisWeekRecords.forEach(record => {
      if (processedRoleIds.has(record.roleId)) return;

      let roleName = '未知角色';
      let sect = '';
      let region = '';
      let server = record.server || '未知服务器';
      let accountName = record.accountId || '未知账号';
      let password: string | undefined;
      let isClientAccount = false;
      let equipmentScore: number | undefined = undefined;

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
            equipmentScore = role.equipmentScore;
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
        equipmentScore,
        accountId: record.accountId,
        accountName,
        password,
        canRun: !cooldownInfo.hasRecordInCurrentCycle,
        canAddMore: cooldownInfo.canAdd,
        recordCount,
        maxRecords,
        cooldownInfo,
        lastRunDate,
        lastRunGold,
        lastRunIncome,
        lastRunExpense,
        cooldownDays,
        bossCooldowns: calculateBossCooldowns(raid, roleRecords.flatMap(r => {
          // 支持多选BOSS：为每个bossId创建一个记录
          if (r.bossIds && r.bossIds.length > 0) {
            return r.bossIds.map(bossId => ({
              id: r.id,
              raidRecordId: r.id,
              bossId: bossId,
              bossName: r.bossNames?.[r.bossIds?.indexOf(bossId) || 0] || '',
              date: r.date,
              roleId: r.roleId,
              accountId: r.accountId
            }));
          }
          // 向后兼容单选BOSS
          return [{
            id: r.id,
            raidRecordId: r.id,
            bossId: r.bossId || '',
            bossName: r.bossName || '',
            date: r.date,
            roleId: r.roleId,
            accountId: r.accountId
          }];
        }), record.roleId)
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
          canRun: !cooldownInfo.hasRecordInCurrentCycle,
          canAddMore: cooldownInfo.canAdd,
          recordCount: 0,
          maxRecords,
          cooldownInfo,
          lastRunDate: undefined,
          lastRunGold: undefined,
          lastRunIncome: undefined,
          lastRunExpense: undefined,
          cooldownDays,
          bossCooldowns: calculateBossCooldowns(raid, [], role.id)
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
    <div className="space-y-8 animate-in fade-in duration-300">
      {/* Header + Card Grid */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <button
              onClick={onBack}
              className="text-sm text-muted hover:text-main transition-colors"
            >
              ← 返回
            </button>
            <span className="text-muted/40">|</span>
            <h2 className="text-lg font-bold text-main flex items-center gap-2">
              <Shield className="w-5 h-5 text-primary" />
              {raid.playerCount}人{raid.difficulty}{raid.name}
            </h2>
            <RaidRefreshCountdown raid={raid} />
          </div>

          <div className="flex items-center gap-4">
            <div className="flex items-center gap-1.5 text-sm">
              <span className="font-bold text-emerald-600">{availableCount}</span>
              <span className="text-muted text-xs">可打</span>
            </div>
            <div className="flex items-center gap-1.5 text-sm">
              <span className="font-bold text-amber-600">{unavailableCount}</span>
              <span className="text-muted text-xs">已打</span>
            </div>
          </div>
        </div>


        {/* Role Cards */}
        {sortedRoles.length === 0 ? (
          <div className="text-center py-8 text-muted bg-surface rounded-xl border border-base border-dashed">
            没有找到符合条件的角色，请先在账号管理中添加并启用角色
          </div>
        ) : (
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


                    <div className="flex items-center justify-between min-h-[24px]">
                      <div className="flex items-center gap-2 flex-wrap flex-1 min-w-0">
                        {role.lastRunDate ? (
                          <>
                            <div className="flex items-center gap-1.5 text-muted overflow-hidden">
                              <Clock className={`w-3.5 h-3.5 flex-shrink-0 ${isAtLimit ? 'text-muted' : role.canRun ? 'text-emerald-500' : 'text-amber-500'}`} />
                              <span className="text-[11px] whitespace-nowrap">{formatDate(role.lastRunDate)}</span>
                            </div>
                            {role.lastRunIncome !== undefined && role.lastRunIncome > 0 && (
                              <div className="flex items-center gap-1 text-emerald-600 ml-1">
                                <TrendingUp className="w-3 h-3 flex-shrink-0" />
                                <span className="text-[11px] font-medium whitespace-nowrap">{formatGoldAmount(role.lastRunIncome)}金</span>
                              </div>
                            )}
                            {role.lastRunExpense !== undefined && role.lastRunExpense > 0 && (
                              <div className="flex items-center gap-1 text-amber-600 ml-1">
                                <TrendingDown className="w-3 h-3 flex-shrink-0" />
                                <span className="text-[11px] font-medium whitespace-nowrap">{formatGoldAmount(role.lastRunExpense)}金</span>
                              </div>
                            )}
                          </>
                        ) : (
                          <div className="flex items-center gap-1.5 text-muted">
                            <Calendar className={`w-3.5 h-3.5 flex-shrink-0 ${isAtLimit ? 'text-muted' : role.canRun ? 'text-emerald-400' : 'text-amber-400'}`} />
                            <span className="text-[11px] whitespace-nowrap">暂无记录</span>
                          </div>
                        )}
                      </div>

                      {role.bossCooldowns && role.bossCooldowns.length > 0 && (
                        <div className="flex-shrink-0 ml-2">
                          <BossCooldownSummary bossCooldowns={role.bossCooldowns} />
                        </div>
                      )}
                    </div>
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

