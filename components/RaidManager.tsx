import React, { useState, useMemo } from 'react';
import { Raid } from '../types';
import { Plus, Trash2, Shield, Filter, Power } from 'lucide-react';
import { getRaidKey } from '../utils/raidUtils';
import { toast } from '../utils/toastManager';
import { AddRaidModal } from './AddRaidModal';
import { TrialPlaceManager } from './TrialPlaceManager';
import { TrialPlaceRecord, Account } from '../types';
import { db } from '../services/db';

interface RaidManagerProps {
  raids: Raid[];
  setRaids: React.Dispatch<React.SetStateAction<Raid[]>>;
  onRaidClick?: (raid: Raid) => void;
  trialRecords: TrialPlaceRecord[];
  setTrialRecords: React.Dispatch<React.SetStateAction<TrialPlaceRecord[]>>;
  accounts: Account[];
}

interface MergedRaid {
  name: string;
  version: string;
  raids: Raid[];
  isActive: boolean;
  disabled: boolean;
  difficultyLabels: { [key: string]: string };
}

const DIFFICULTY_LABELS = {
  NORMAL: '普通',
  HEROIC: '英雄',
  CHALLENGE: '挑战'
};

const DIFFICULTY_COLORS = {
  NORMAL: 'bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-900/30 dark:text-emerald-400 dark:border-emerald-800',
  HEROIC: 'bg-blue-50 text-blue-700 border-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:border-blue-800',
  CHALLENGE: 'bg-rose-50 text-rose-700 border-rose-200 dark:bg-rose-900/30 dark:text-rose-400 dark:border-rose-800'
};

const VERSION_ORDER_MAP: Record<string, number> = {
  "丝路风雨": 0,
  "横刀断浪": 1,
  "奉天证道": 2,
  "世外蓬莱": 3,
  "重制版": 4,
  "风骨霸刀": 5,
  "剑胆琴心": 6,
  "安史之乱": 7,
  "巴蜀风云": 8,
  "风起稻香": 9
};

export const RaidManager: React.FC<RaidManagerProps> = ({
  raids,
  setRaids,
  onRaidClick,
  trialRecords,
  setTrialRecords,
  accounts
}) => {
  const [activeTab, setActiveTab] = useState<'raid' | 'trial'>('raid');
  const [isAdding, setIsAdding] = useState(false);

  const versions = useMemo(() => {
    const versionSet = new Set<string>();
    raids.forEach(raid => {
      if (raid.version) {
        versionSet.add(raid.version);
      }
    });

    return Array.from(versionSet).sort((a, b) => {
      const orderA = VERSION_ORDER_MAP[a] !== undefined ? VERSION_ORDER_MAP[a] : 999;
      const orderB = VERSION_ORDER_MAP[b] !== undefined ? VERSION_ORDER_MAP[b] : 999;
      return orderA - orderB;
    });
  }, [raids]);

  // 默认选择最新版本（第一个版本）
  const [selectedVersion, setSelectedVersion] = useState<string>(() => {
    // 从 raids 中提取版本并排序，获取最新版本
    const versionSet = new Set<string>();
    raids.forEach(raid => {
      if (raid.version) {
        versionSet.add(raid.version);
      }
    });
    const sortedVersions = Array.from(versionSet).sort((a, b) => {
      const orderA = VERSION_ORDER_MAP[a] !== undefined ? VERSION_ORDER_MAP[a] : 999;
      const orderB = VERSION_ORDER_MAP[b] !== undefined ? VERSION_ORDER_MAP[b] : 999;
      return orderA - orderB;
    });
    return sortedVersions.length > 0 ? sortedVersions[0] : 'all';
  });

  const isStaticRaid = (raid: Raid) => {
    return !!raid.static;
  };

  const filteredRaids = useMemo(() => {
    return raids.filter(raid => raid.version === selectedVersion);
  }, [raids, selectedVersion]);

  const groupedRaids = useMemo(() => {
    const groups: Record<string, Raid[]> = {};
    filteredRaids.forEach(raid => {
      const version = raid.version || '其他';
      if (!groups[version]) {
        groups[version] = [];
      }
      groups[version].push(raid);
    });

    Object.keys(groups).forEach(version => {
      groups[version].reverse();
    });

    return groups;
  }, [filteredRaids]);

  const mergedGroupedRaids = useMemo(() => {
    const mergedGroups: Record<string, MergedRaid[]> = {};

    Object.entries(groupedRaids).forEach(([version, versionRaids]) => {
      const mergedRaids: MergedRaid[] = [];

      // 首先按人数类型分组
      const raidsByPlayerCount: Record<number, Raid[]> = {
        10: [],
        25: []
      };

      versionRaids.forEach(raid => {
        raidsByPlayerCount[raid.playerCount].push(raid);
      });

      // 处理每个副本名称
      const nameGroups = new Map<string, Raid[]>();
      versionRaids.forEach(raid => {
        const key = raid.name;
        if (!nameGroups.has(key)) {
          nameGroups.set(key, []);
        }
        nameGroups.get(key)!.push(raid);
      });

      nameGroups.forEach((raids, name) => {
        const difficultyLabels: { [key: string]: string } = {};

        // 只处理难度标签，不修改 isActive 状态
        const hasMultiplePlayerCounts = new Set(raids.map(r => r.playerCount)).size > 1;

        raids.forEach(raid => {
          const key = getRaidKey(raid);
          const difficultyLabel = DIFFICULTY_LABELS[raid.difficulty];

          // 如果存在多种人数版本，在标签中添加人数标识
          if (hasMultiplePlayerCounts) {
            difficultyLabels[key] = `${raid.playerCount}人${difficultyLabel}`;
          } else {
            difficultyLabels[key] = difficultyLabel;
          }
        });

        const sortedRaids = raids.sort((a, b) => {
          const playerCountOrder = { 10: 0, 25: 1 };
          const difficultyOrder = { 'NORMAL': 0, 'HEROIC': 1, 'CHALLENGE': 2 };

          if (playerCountOrder[a.playerCount] !== playerCountOrder[b.playerCount]) {
            return playerCountOrder[a.playerCount] - playerCountOrder[b.playerCount];
          }

          return difficultyOrder[a.difficulty] - difficultyOrder[b.difficulty];
        });

        const mergedRaid: MergedRaid = {
          name,
          version: raids[0].version || '其他',
          raids: sortedRaids,
          isActive: sortedRaids.some(r => r.isActive),
          disabled: sortedRaids.every(r => !r.isActive),
          difficultyLabels
        };
        mergedRaids.push(mergedRaid);
      });

      // 按版本顺序排序
      mergedGroups[version] = mergedRaids.sort((a, b) => {
        const versionOrder = VERSION_ORDER_MAP[a.version] !== undefined ? VERSION_ORDER_MAP[a.version] : 999;
        const versionOrderB = VERSION_ORDER_MAP[b.version] !== undefined ? VERSION_ORDER_MAP[b.version] : 999;
        return versionOrder - versionOrderB;
      });
    });

    return mergedGroups;
  }, [groupedRaids]);

  const handleAddRaid = (raid: Raid) => {
    setRaids(prev => [...prev, raid]);
    toast.success(`成功添加副本: ${raid.name}`);
  };

  const toggleRaidStatus = (name: string) => {
    const raidsWithName = raids.filter(r => r.name === name);
    if (raidsWithName.length === 0) return;

    const currentActiveCount = raidsWithName.filter(r => r.isActive).length;
    const newStatus = currentActiveCount === 0;

    setRaids(prev => prev.map(r => {
      if (r.name === name) {
        return { ...r, isActive: newStatus };
      }
      return r;
    }));

    const raidName = raidsWithName[0].name;
    toast.success(`${raidName} 已${newStatus ? '启用所有难度' : '禁用所有难度'}`);
  };

  const toggleRaidDifficultyStatus = (key: string) => {
    setRaids(prev => prev.map(r => {
      if (getRaidKey(r) === key) {
        const newStatus = !r.isActive;
        return { ...r, isActive: newStatus };
      }
      return r;
    }));

    const raid = raids.find(r => getRaidKey(r) === key);
    if (raid) {
      const isNowActive = !raid.isActive;
      toast.success(`${raid.name} - ${DIFFICULTY_LABELS[raid.difficulty]} 已${isNowActive ? '启用' : '禁用'}`);
    }
  };

  return (
    <div className="space-y-6">
      {/* Tabs */}
      <div className="flex gap-2 border-b border-base pb-1">
        <button
          onClick={() => setActiveTab('raid')}
          className={`px-4 py-2 text-sm font-medium rounded-t-lg transition-colors relative top-px ${activeTab === 'raid'
            ? 'bg-base text-primary border border-base border-b-transparent'
            : 'text-muted hover:text-main hover:bg-base/50'
            }`}
        >
          团队副本
        </button>
        <button
          onClick={() => setActiveTab('trial')}
          className={`px-4 py-2 text-sm font-medium rounded-t-lg transition-colors relative top-px ${activeTab === 'trial'
            ? 'bg-base text-primary border border-base border-b-transparent'
            : 'text-muted hover:text-main hover:bg-base/50'
            }`}
        >
          试炼之地
        </button>
      </div>

      {activeTab === 'trial' ? (
        <TrialPlaceManager
          records={trialRecords}
          accounts={accounts}
          onAddRecord={(record) => setTrialRecords(prev => [...prev, record])}
          onDeleteRecord={async (recordId) => {
            try {
              await db.deleteTrialRecord(recordId);
              setTrialRecords(prev => prev.filter(r => r.id !== recordId));
              toast.success('已删除试炼记录');
            } catch (error) {
              console.error('删除失败:', error);
              toast.error('删除试炼记录失败');
            }
          }}
        />
      ) : (
        <>
          <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4">
            <h2 className="text-xl font-bold text-main">副本管理</h2>
            <div className="flex gap-2">
              {versions.length > 0 && (
                <div className="flex items-center gap-2 bg-surface px-3 py-1.5 rounded-lg border border-base shadow-sm">
                  <Filter className="w-4 h-4 text-muted" />
                  <select
                    className="bg-transparent text-sm outline-none cursor-pointer text-main"
                    value={selectedVersion}
                    onChange={e => setSelectedVersion(e.target.value)}
                  >
                    {versions.map(version => (
                      <option key={version} value={version}>{version}</option>
                    ))}
                  </select>
                </div>
              )}
              <button
                onClick={() => setIsAdding(true)}
                className="bg-primary hover:bg-primary-hover text-white px-3 py-1.5 rounded-lg flex items-center gap-2 transition-all shadow-sm active:scale-[0.98] text-sm font-medium"
              >
                <Plus className="w-4 h-4" /> 新增副本
              </button>
            </div>
          </div>

          <div className="bg-blue-50/50 border border-blue-100 rounded-lg p-3 text-center text-sm text-blue-600">
            右键难度框可单独禁用/启用
          </div>

          {Object.keys(mergedGroupedRaids).length > 0 ? (
            versions.map(version => {
              const versionRaids = mergedGroupedRaids[version];
              if (!versionRaids) return null;

              return (
                <div key={version} className="space-y-4">
                  <div className="flex items-center gap-3">
                    <div className="h-px bg-border flex-1"></div>
                    <h3 className="text-sm font-bold text-muted uppercase tracking-wider bg-base px-3 py-1 rounded-full border border-border">
                      {version}
                    </h3>
                    <div className="h-px bg-border flex-1"></div>
                  </div>
                  <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3 sm:gap-4">
                    {versionRaids.map(mergedRaid => {
                      const isRaidActive = mergedRaid.isActive && mergedRaid.raids.some(r => r.isActive);
                      const isStatic = mergedRaid.raids.some(r => isStaticRaid(r));
                      const isDisabled = mergedRaid.disabled;
                      return (
                        <div
                          key={getRaidKey(mergedRaid.raids[0])}
                          className={`p-4 rounded-xl border transition-all duration-200 relative group
                        ${isDisabled
                              ? 'border-base bg-base opacity-70'
                              : 'bg-surface border-base hover:border-primary/50 hover:shadow-sm'
                            }`}
                        >
                          <div className="flex justify-between items-start mb-3">
                            <div className="flex items-center gap-2">
                              <span className={`p-1.5 rounded-lg ${isDisabled ? 'bg-base text-muted' : 'bg-primary/10 text-primary'}`}>
                                <Shield size={16} />
                              </span>
                              <div>
                                <h3 className={`font-semibold text-sm ${isDisabled ? 'text-muted' : 'text-main'}`}>{mergedRaid.name}</h3>
                              </div>
                            </div>
                            <div className="flex items-center gap-2">
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  toggleRaidStatus(mergedRaid.name);
                                }}
                                className={`transition-all p-1 rounded-md ${isDisabled
                                  ? 'text-muted hover:text-main hover:bg-base'
                                  : isRaidActive
                                    ? 'text-emerald-500 hover:text-emerald-600 hover:bg-emerald-50'
                                    : 'text-muted hover:text-main hover:bg-base'
                                  }`}
                                title={isDisabled ? '启用此副本' : '禁用此副本（所有难度）'}
                              >
                                <Power size={16} />
                              </button>
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  if (isStatic) {
                                    alert('预制副本不能删除，只能禁用');
                                    return;
                                  }
                                  if (confirm('确认删除此副本？')) {
                                    mergedRaid.raids.forEach(raid => {
                                      const key = getRaidKey(raid);
                                      setRaids(prev => prev.filter(r => getRaidKey(r) !== key));
                                    });
                                  }
                                }}
                                className={`transition-colors p-1 rounded-md ${isStatic
                                  ? 'text-base cursor-not-allowed opacity-50'
                                  : 'text-muted hover:text-red-500 hover:bg-red-50'
                                  }`}
                                title={isStatic ? '预制副本不能删除' : '删除'}
                              >
                                <Trash2 size={16} />
                              </button>
                            </div>
                          </div>

                          <div className="flex items-center gap-2 mb-3 flex-wrap justify-center sm:justify-start">
                            {mergedRaid.raids.map(raid => {
                              const label = mergedRaid.difficultyLabels[getRaidKey(raid)] || DIFFICULTY_LABELS[raid.difficulty];
                              const isSpecialRaid = raid.name === '弓月城' || raid.name === '缚罪之渊';
                              return (
                                <div
                                  key={getRaidKey(raid)}
                                  onClick={() => onRaidClick?.(raid)}
                                  onContextMenu={(e) => {
                                    e.preventDefault();
                                    e.stopPropagation();
                                    toggleRaidDifficultyStatus(getRaidKey(raid));
                                  }}
                                  className={`relative px-2 py-1 text-xs font-medium rounded border cursor-pointer transition-all hover:scale-105 min-w-[60px] text-center ${raid.isActive
                                    ? DIFFICULTY_COLORS[raid.difficulty]
                                    : 'bg-base text-muted border-base hover:bg-base/80 opacity-60'
                                    }`}
                                  title={raid.isActive ? `${label} - 点击进入详情，右键切换状态` : `已禁用 - ${label} - 右键启用${!raid.isActive && !isSpecialRaid ? '（默认禁用）' : ''}`}
                                >
                                  {label}
                                </div>
                              );
                            })}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              );
            })
          ) : (
            <div className="text-center py-12 text-muted bg-surface rounded-xl border border-base">
              <Shield className="w-12 h-12 mx-auto mb-2 opacity-20" />
              <p>暂无副本，点击上方按钮添加</p>
            </div>
          )}
          <AddRaidModal
            isOpen={isAdding}
            onClose={() => setIsAdding(false)}
            onSubmit={handleAddRaid}
            existingRaids={raids}
          />
        </>
      )}
    </div>
  );
};