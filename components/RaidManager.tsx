import React, { useState, useMemo, useEffect } from 'react';
import { Raid } from '../types';
import { Trash2, Shield, Filter, Power } from 'lucide-react';
import { getRaidKey } from '../utils/raidUtils';
import { toast } from '../utils/toastManager';
import { TrialPlaceManager } from './TrialPlaceManager';
import { BaizhanManager } from './BaizhanManager';
import { TrialPlaceRecord, Account, BaizhanRecord, RaidRecord } from '../types';
import { db } from '../services/db';
import { RaidDetail } from './RaidDetail';

interface RaidManagerProps {
  raids: Raid[];
  setRaids: React.Dispatch<React.SetStateAction<Raid[]>>;
  records: RaidRecord[];
  setRecords: React.Dispatch<React.SetStateAction<RaidRecord[]>>;
  onEditRecord?: (record: RaidRecord) => void;
  onEditBaizhanRecord?: (record: BaizhanRecord) => void;
  trialRecords: TrialPlaceRecord[];
  setTrialRecords: React.Dispatch<React.SetStateAction<TrialPlaceRecord[]>>;
  baizhanRecords: BaizhanRecord[];
  setBaizhanRecords: React.Dispatch<React.SetStateAction<BaizhanRecord[]>>;
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

const DIFFICULTY_LABELS: Record<string, string> = {
  '普通': '普通',
  '英雄': '英雄',
  '挑战': '挑战'
};

const DIFFICULTY_COLORS: Record<string, string> = {
  '普通': 'bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-900/30 dark:text-emerald-400 dark:border-emerald-800',
  '英雄': 'bg-blue-50 text-blue-700 border-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:border-blue-800',
  '挑战': 'bg-rose-50 text-rose-700 border-rose-200 dark:bg-rose-900/30 dark:text-rose-400 dark:border-rose-800'
};



export const RaidManager: React.FC<RaidManagerProps> = ({
  raids,
  setRaids,
  records,
  setRecords,
  onEditRecord,
  onEditBaizhanRecord,
  trialRecords,
  setTrialRecords,
  baizhanRecords,
  setBaizhanRecords,
  accounts
}) => {
  const [activeTab, setActiveTab] = useState<'raid' | 'trial' | 'baizhan'>('raid');
  const [selectedRaid, setSelectedRaid] = useState<Raid | null>(null);

  const [versionOrderMap, setVersionOrderMap] = useState<Record<string, number>>({});
  const [isOrderLoaded, setIsOrderLoaded] = useState(false);

  useEffect(() => {
    const fetchVersions = async () => {
      try {
        const cacheKey = 'raid_versions_cache_v2';
        const cache = await db.getCache(cacheKey);

        let shouldFetch = true;

        if (cache && cache.updatedAt) {
          const now = new Date();
          const lastUpdate = new Date(cache.updatedAt);
          // 每天早上7点刷新
          const refreshTime = new Date(now);
          refreshTime.setHours(7, 0, 0, 0);

          if (now.getTime() < refreshTime.getTime()) {
            refreshTime.setDate(refreshTime.getDate() - 1);
          }

          if (lastUpdate.getTime() > refreshTime.getTime()) {
            shouldFetch = false;
            const orderMap: Record<string, number> = {};
            cache.value.forEach((v: string, i: number) => {
              orderMap[v] = i;
            });
            setVersionOrderMap(orderMap);
          }
        }

        if (shouldFetch) {
          const versions = await db.getRaidVersions();
          const orderMap: Record<string, number> = {};
          versions.forEach((v: string, i: number) => {
            orderMap[v] = i;
          });
          setVersionOrderMap(orderMap);
          await db.saveCache(cacheKey, versions);
        }
        setIsOrderLoaded(true);
      } catch (err) {
        console.error('Failed to fetch raid versions order', err);
        setIsOrderLoaded(true);
      }
    };

    fetchVersions();
  }, []);

  const versions = useMemo(() => {
    const versionSet = new Set<string>();
    raids.forEach(raid => {
      if (raid.version) {
        versionSet.add(raid.version);
      }
    });

    return Array.from(versionSet).sort((a, b) => {
      const orderA = versionOrderMap[a] !== undefined ? versionOrderMap[a] : 999;
      const orderB = versionOrderMap[b] !== undefined ? versionOrderMap[b] : 999;
      return orderA - orderB;
    });
  }, [raids, versionOrderMap]);

  // 默认选择最新版本（由于需要等待后端字典加载完毕，故初始化为空/降级值，随后通过效应重置）
  const [selectedVersion, setSelectedVersion] = useState<string>('all');

  useEffect(() => {
    if (isOrderLoaded && versions.length > 0) {
      setSelectedVersion((prev) => {
        // 如果当前是无意义的 all 或已不在现存列表里，则推诿到数组首位（即最新版本）
        if (prev === 'all' || !versions.includes(prev)) {
          return versions[0];
        }
        return prev;
      });
    }
  }, [versions, isOrderLoaded]);

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
          const difficultyOrder: Record<string, number> = { '普通': 0, '英雄': 1, '挑战': 2 };

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

      // 这里不再需要对版本下的副本数组按版本再次排序，因为它们本身就是同一个版本的副本。
      // 如果是为了某种内部顺序，我们可以保留原状或按名称排序
      mergedGroups[version] = mergedRaids;
    });

    return mergedGroups;
  }, [groupedRaids]);



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
          onClick={() => { setActiveTab('raid'); setSelectedRaid(null); }}
          className={`px-4 py-2 text-sm font-medium rounded-t-lg transition-colors relative top-px ${activeTab === 'raid'
            ? 'bg-base text-primary border border-base border-b-transparent'
            : 'text-muted hover:text-main hover:bg-base/50'
            }`}
        >
          团队副本
        </button>
        <button
          onClick={() => { setActiveTab('trial'); setSelectedRaid(null); }}
          className={`px-4 py-2 text-sm font-medium rounded-t-lg transition-colors relative top-px ${activeTab === 'trial'
            ? 'bg-base text-primary border border-base border-b-transparent'
            : 'text-muted hover:text-main hover:bg-base/50'
            }`}
        >
          试炼之地
        </button>
        <button
          onClick={() => { setActiveTab('baizhan'); setSelectedRaid(null); }}
          className={`px-4 py-2 text-sm font-medium rounded-t-lg transition-colors relative top-px ${activeTab === 'baizhan'
            ? 'bg-base text-primary border border-base border-b-transparent'
            : 'text-muted hover:text-main hover:bg-base/50'
            }`}
        >
          百战
        </button>
      </div>

      {activeTab === 'baizhan' ? (
        <BaizhanManager
          records={baizhanRecords}
          accounts={accounts}
          onAddRecord={(record) => setBaizhanRecords(prev => [...prev, record])}
          onDeleteRecord={async (recordId) => {
            try {
              await db.deleteBaizhanRecord(recordId);
              setBaizhanRecords(prev => prev.filter(r => r.id !== recordId));
              toast.success('已删除百战记录');
            } catch (error) {
              console.error('删除失败:', error);
              toast.error('删除百战记录失败');
            }
          }}
          onEditRecord={onEditBaizhanRecord}
        />
      ) : activeTab === 'trial' ? (
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
      ) : selectedRaid ? (
        <RaidDetail
          key={`raidDetail-${getRaidKey(selectedRaid)}`}
          raid={selectedRaid}
          accounts={accounts}
          records={records}
          onBack={() => setSelectedRaid(null)}
          setRecords={setRecords}
          onEditRecord={onEditRecord}
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
                                  ? 'text-muted cursor-not-allowed opacity-50'
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
                                  onClick={() => setSelectedRaid(raid)}
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
              <p>暂无副本数据</p>
            </div>
          )}
        </>
      )}
    </div>
  );
};