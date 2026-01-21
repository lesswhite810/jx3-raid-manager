import React, { useState, useMemo } from 'react';
import { Raid } from '../types';
import { Plus, Trash2, Shield, Filter, Power } from 'lucide-react';
import { getRaidKey } from '../utils/raidUtils';
import { toast } from '../utils/toastManager';
import { AddRaidModal } from './AddRaidModal';

interface RaidManagerProps {
  raids: Raid[];
  setRaids: React.Dispatch<React.SetStateAction<Raid[]>>;
  onRaidClick?: (raid: Raid) => void;
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
  NORMAL: 'bg-green-100 text-green-700 border-green-200',
  HEROIC: 'bg-blue-100 text-blue-700 border-blue-200',
  CHALLENGE: 'bg-red-100 text-red-700 border-red-200'
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

export const RaidManager: React.FC<RaidManagerProps> = ({ raids, setRaids, onRaidClick }) => {
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
      <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4">
        <h2 className="text-2xl font-bold text-main">副本管理</h2>
        <div className="flex gap-2">
          {versions.length > 0 && (
            <div className="flex items-center gap-2 bg-surface px-3 py-2 rounded-lg border border-base">
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
            className="bg-gradient-to-r from-indigo-600 to-indigo-500 hover:from-indigo-500 hover:to-indigo-400 text-white px-4 py-2 rounded-xl flex items-center gap-2 transition-all shadow-md hover:shadow-lg active:scale-95"
          >
            <Plus className="w-4 h-4" /> 新增副本
          </button>
        </div>
      </div>

      <div className="bg-blue-50 border border-blue-200 rounded-lg p-3 text-center text-sm text-blue-700">
        右键难度框可单独禁用/启用
      </div>



      {Object.keys(mergedGroupedRaids).length > 0 ? (
        versions.map(version => {
          const versionRaids = mergedGroupedRaids[version];
          if (!versionRaids) return null;

          return (
            <div key={version} className="space-y-6">
              <div className="flex items-center gap-3">
                <div className="h-px bg-slate-300 flex-1"></div>
                <h3 className="text-xl font-bold text-slate-800 bg-gradient-to-r from-indigo-50 to-blue-50 px-6 py-2 rounded-full border-2 border-indigo-200 shadow-sm">
                  {version}
                </h3>
                <div className="h-px bg-slate-300 flex-1"></div>
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3 sm:gap-4">
                {versionRaids.map(mergedRaid => {
                  const isRaidActive = mergedRaid.isActive && mergedRaid.raids.some(r => r.isActive);
                  const isStatic = mergedRaid.raids.some(r => isStaticRaid(r));
                  const isDisabled = mergedRaid.disabled;
                  return (
                    <div
                      key={getRaidKey(mergedRaid.raids[0])}
                      className={`p-4 sm:p-5 rounded-2xl border transition-all duration-300 relative group
                        ${isDisabled
                          ? 'border-red-200 bg-red-50/30 dark:bg-red-900/10'
                          : 'bg-white/50 dark:bg-slate-800/50 backdrop-blur-md border-slate-200 dark:border-slate-700 shadow-sm hover:shadow-xl hover:-translate-y-1'
                        }`}
                    >
                      <div className="flex justify-between items-start mb-3">
                        <div className="flex items-center gap-2">
                          <span className={`p-2 rounded-full ${isDisabled ? 'bg-slate-100 text-slate-400' : 'bg-indigo-100 text-indigo-600'}`}>
                            <Shield size={18} />
                          </span>
                          <div>
                            <h3 className={`font-bold ${isDisabled ? 'text-slate-500' : 'text-slate-800'}`}>{mergedRaid.name}</h3>
                          </div>
                        </div>
                        <div className="flex items-center gap-2">
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              toggleRaidStatus(mergedRaid.name);
                            }}
                            className={`transition-all p-1 rounded-md ${isDisabled
                              ? 'text-slate-300 hover:text-slate-500 hover:bg-slate-100'
                              : isRaidActive
                                ? 'text-green-500 hover:text-green-700 hover:bg-green-50'
                                : 'text-slate-300 hover:text-slate-500 hover:bg-slate-100'
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
                              ? 'text-slate-200 cursor-not-allowed'
                              : 'text-slate-300 hover:text-red-500 hover:bg-red-50'
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
                              className={`relative px-3 py-2 text-xs sm:text-sm font-bold rounded-lg border cursor-pointer transition-all hover:scale-105 hover:shadow-sm min-w-[60px] sm:min-w-[70px] text-center ${raid.isActive
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
    </div>
  );
};