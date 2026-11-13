import React from 'react';
import { Shield, Star, Power, Trash2 } from 'lucide-react';
import { Raid } from '../types';
import { getRaidKey } from '../utils/raidUtils';
import { db } from '../services/db';
import { DIFFICULTY_COLORS, DIFFICULTY_LABELS, MergedRaid, isStaticRaid } from '../utils/raidDisplay';
import { WeeklyRaidGroup } from '../utils/weeklyRaids';

export interface RaidCollectionViewProps {
  /** 本周团队秘境分组 */
  weeklyRaidGroups: WeeklyRaidGroup[];
  /** 本周名单加载状态，非 success 时在顶部给出提示 */
  weeklyRaidStatus: 'loading' | 'success' | 'error';
  /** 已收藏且未在本周区展示的副本 */
  favoriteMergedRaids: MergedRaid[];
  /** 已收藏副本名集合 */
  favoriteRaids: Set<string>;
  onToggleFavorite: (raidName: string) => void;
  onToggleRaidStatus: (raidName: string) => void;
  onToggleRaidDifficultyStatus: (raidKey: string) => void;
  onSelectRaid: (raid: Raid) => void;
  setRaids: React.Dispatch<React.SetStateAction<Raid[]>>;
}

/**
 * 收藏界面（历史模式的团队副本页默认视图）
 *
 * 本周末团队秘境 + 收藏副本两张卡片区，是 v2.2.8 之前的原始形态。
 * 当前模式用 CD 概览表替代了这块内容，历史模式仍保留它以还原旧版操作路径。
 */
export const RaidCollectionView: React.FC<RaidCollectionViewProps> = ({
  weeklyRaidGroups,
  weeklyRaidStatus,
  favoriteMergedRaids,
  favoriteRaids,
  onToggleFavorite,
  onToggleRaidStatus,
  onToggleRaidDifficultyStatus,
  onSelectRaid,
  setRaids,
}) => {
  const handleDeleteRaid = (mergedRaid: MergedRaid) => {
    if (mergedRaid.raids.some(isStaticRaid)) {
      alert('预制副本不能删除，只能禁用');
      return;
    }
    if (!confirm('确认删除此副本？')) return;
    // 先落库再改 UI：自动保存默认不删行，必须显式调用删除命令
    mergedRaid.raids.forEach(raid => {
      const key = getRaidKey(raid);
      db.deleteRaidStructured(key)
        .then(() => {
          setRaids(prev => prev.filter(item => getRaidKey(item) !== key));
        })
        .catch(error => {
          console.error('删除副本失败:', error);
          alert('删除副本失败，请重试');
        });
    });
  };

  return (
    <div className="space-y-4">
      {weeklyRaidStatus !== 'success' && (
        <div className="rounded-card border border-ds-warning-soft bg-ds-warning-soft px-3 py-2 text-sm text-ds-warning-strong">
          {weeklyRaidStatus === 'loading' ? '正在获取本周团队秘境…' : '本周团队秘境获取失败，以下仅显示已收藏副本。'}
        </div>
      )}

      {weeklyRaidGroups.length > 0 && (
        <>
          <div className="flex items-center gap-3">
            <div className="h-px bg-ds-success-soft flex-1"></div>
            <h3 className="text-sm font-bold text-ds-success-strong uppercase tracking-wider bg-ds-success-soft px-3 py-1 rounded-tag border border-ds-success-soft flex items-center gap-1.5">
              <Shield size={14} />
              本周团队秘境
            </h3>
            <div className="h-px bg-ds-success-soft flex-1"></div>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3 sm:gap-4">
            {weeklyRaidGroups.map(group => {
              const allDisabled = !group.missingConfig && group.raids.every(raid => !raid.isActive);
              const hasMultiplePlayerCounts = new Set(group.raids.map(raid => raid.playerCount)).size > 1;
              // 本赛季副本显示全部难度；历史赛季副本只显示 10 人难度
              const scopeLabel = group.missingTenPlayerConfig
                ? '未找到10人配置，已显示现有难度'
                : group.scope === 'history' ? '10人难度' : '全部难度';
              const subtitle = group.missingConfig
                ? '未找到副本配置'
                : [
                  scopeLabel + (allDisabled ? '（已禁用）' : ''),
                  group.isFavorite ? '已收藏' : null,
                ].filter(Boolean).join(' · ');

              return (
                <div
                  key={`weekly-${group.name}`}
                  className={`p-4 rounded-card border ${group.missingConfig || group.missingTenPlayerConfig
                    ? 'border-ds-warning-soft bg-ds-warning-soft/30'
                    : allDisabled
                      ? 'border-base bg-base opacity-70'
                      : 'border-ds-success-soft bg-ds-success-soft/30'
                    }`}
                >
                  <div className="flex items-start justify-between gap-2 mb-3">
                    <div className="flex items-center gap-2">
                      <span className={`p-1.5 rounded-tag ${group.isFavorite ? 'bg-ds-warning-soft text-ds-warning-strong' : 'bg-ds-success-soft text-ds-success-strong'}`}>
                        {group.isFavorite ? <Star size={16} fill="currentColor" /> : <Shield size={16} />}
                      </span>
                      <div>
                        <h3 className="font-semibold text-sm text-main">{group.name}</h3>
                        <p className="text-xs text-muted">{subtitle}</p>
                      </div>
                    </div>
                    {!group.missingConfig && (
                      <button
                        onClick={(event) => {
                          event.stopPropagation();
                          onToggleFavorite(group.name);
                        }}
                        className={`transition-all p-1 rounded-md ${group.isFavorite
                          ? 'text-ds-warning-strong hover:bg-ds-warning-soft'
                          : 'text-muted hover:text-ds-warning-strong hover:bg-ds-warning-soft/50'
                          }`}
                        title={group.isFavorite ? '取消收藏' : '收藏此副本'}
                      >
                        <Star size={16} fill={group.isFavorite ? 'currentColor' : 'none'} />
                      </button>
                    )}
                  </div>
                  <div className="flex items-center gap-2 flex-wrap">
                    {group.raids.map(raid => {
                      const label = hasMultiplePlayerCounts
                        ? `${raid.playerCount}人${DIFFICULTY_LABELS[raid.difficulty]}`
                        : DIFFICULTY_LABELS[raid.difficulty];
                      return (
                        <div
                          key={getRaidKey(raid)}
                          onClick={() => onSelectRaid(raid)}
                          onContextMenu={(event) => {
                            event.preventDefault();
                            event.stopPropagation();
                            onToggleRaidDifficultyStatus(getRaidKey(raid));
                          }}
                          className={`px-2 py-1 text-xs font-medium rounded-tag border cursor-pointer ${raid.isActive
                            ? 'bg-ds-success-soft text-ds-success-strong border-ds-success-soft'
                            : 'bg-base text-muted border-base'
                            }`}
                          title={`${label} - ${raid.isActive ? '点击进入详情，右键禁用' : '已禁用，点击进入详情，右键启用'}`}
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
        </>
      )}

      {favoriteMergedRaids.length > 0 && (
        <div className="flex items-center gap-3">
          <div className="h-px bg-ds-warning-soft flex-1"></div>
          <h3 className="text-sm font-bold text-ds-warning-strong uppercase tracking-wider bg-ds-warning-soft px-3 py-1 rounded-full border border-ds-warning-soft flex items-center gap-1.5">
            <Star size={14} fill="currentColor" />
            收藏
          </h3>
          <div className="h-px bg-ds-warning-soft flex-1"></div>
        </div>
      )}

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3 sm:gap-4">
        {favoriteMergedRaids.map(mergedRaid => {
          const staticRaid = mergedRaid.raids.some(isStaticRaid);
          return (
            <div
              key={`favorite-${mergedRaid.name}`}
              className="p-4 rounded-xl border transition-all duration-200 relative group bg-ds-warning-soft/30 border-ds-warning-soft hover:border-ds-warning hover:shadow-sm"
            >
              <div className="flex justify-between items-start mb-3">
                <div className="flex items-center gap-2">
                  <span className="p-1.5 rounded-lg bg-ds-warning-soft text-ds-warning-strong">
                    <Shield size={16} />
                  </span>
                  <div>
                    <h3 className="font-semibold text-sm text-main">{mergedRaid.name}</h3>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={(event) => {
                      event.stopPropagation();
                      onToggleFavorite(mergedRaid.name);
                    }}
                    className="transition-all p-1 rounded-md text-ds-warning-strong hover:text-ds-warning-strong hover:bg-ds-warning-soft"
                    title="取消收藏"
                  >
                    <Star size={16} fill="currentColor" />
                  </button>
                  <button
                    onClick={(event) => {
                      event.stopPropagation();
                      onToggleRaidStatus(mergedRaid.name);
                    }}
                    className="transition-all p-1 rounded-md text-ds-success-strong hover:text-ds-success-strong hover:bg-ds-success-soft"
                    title="禁用此副本（所有难度）"
                  >
                    <Power size={16} />
                  </button>
                  <button
                    onClick={(event) => {
                      event.stopPropagation();
                      handleDeleteRaid(mergedRaid);
                    }}
                    className={`transition-colors p-1 rounded-md ${staticRaid
                      ? 'text-muted cursor-not-allowed opacity-50'
                      : 'text-muted hover:text-red-500 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/40'
                      }`}
                    title={staticRaid ? '预制副本不能删除' : '删除'}
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>

              <div className="flex items-center gap-2 mb-3 flex-wrap justify-center sm:justify-start">
                {mergedRaid.raids.map(raid => {
                  const label = mergedRaid.difficultyLabels[getRaidKey(raid)] || DIFFICULTY_LABELS[raid.difficulty];
                  return (
                    <div
                      key={getRaidKey(raid)}
                      onClick={() => onSelectRaid(raid)}
                      onContextMenu={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        onToggleRaidDifficultyStatus(getRaidKey(raid));
                      }}
                      className={`relative px-2 py-1 text-xs font-medium rounded border cursor-pointer transition-all hover:scale-105 min-w-[60px] text-center ${DIFFICULTY_COLORS[raid.difficulty]}`}
                      title={`${label} - 点击进入详情，右键切换状态`}
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

      {favoriteRaids.size === 0 && weeklyRaidGroups.length === 0 && (
        <div className="text-center py-12 text-muted bg-surface rounded-xl border border-base">
          <Shield className="w-12 h-12 mx-auto mb-2 opacity-20" />
          <p>暂无副本数据</p>
        </div>
      )}
    </div>
  );
};
