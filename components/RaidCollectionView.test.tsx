import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { RaidCollectionView } from './RaidCollectionView';
import { AccountType, Raid } from '../types';
import { MergedRaid } from '../utils/raidDisplay';
import { WeeklyRaidGroup } from '../utils/weeklyRaids';

const RAID: Raid = {
  name: '洛阳之战',
  difficulty: '普通',
  playerCount: 25,
  version: '苍生铸世',
  isActive: true,
};

const FAVORITE_RAID: MergedRaid = {
  name: '阆风悬城',
  version: '丝路风雨',
  raids: [
    { name: '阆风悬城', difficulty: '普通', playerCount: 25, version: '丝路风雨', isActive: true },
  ],
  isActive: true,
  disabled: false,
  difficultyLabels: {},
};

const WEEKLY_GROUP: WeeklyRaidGroup = {
  name: '洛阳之战',
  version: '苍生铸世',
  raids: [RAID],
  isFavorite: false,
  scope: 'current',
  missingTenPlayerConfig: false,
  missingConfig: false,
};

interface RenderOptions {
  weeklyRaidGroups?: WeeklyRaidGroup[];
  weeklyRaidStatus?: 'loading' | 'success' | 'error';
  favoriteMergedRaids?: MergedRaid[];
  favoriteRaids?: Set<string>;
}

const noop = () => undefined;

const renderView = (options: RenderOptions = {}): string => renderToStaticMarkup(
  <RaidCollectionView
    weeklyRaidGroups={options.weeklyRaidGroups ?? [WEEKLY_GROUP]}
    weeklyRaidStatus={options.weeklyRaidStatus ?? 'success'}
    favoriteMergedRaids={options.favoriteMergedRaids ?? [FAVORITE_RAID]}
    favoriteRaids={options.favoriteRaids ?? new Set<string>(['阆风悬城'])}
    onToggleFavorite={noop}
    onToggleRaidStatus={noop}
    onToggleRaidDifficultyStatus={noop}
    onSelectRaid={noop}
    setRaids={noop}
  />,
);

describe('RaidCollectionView（历史模式收藏界面）', () => {
  it('同时渲染本周团队秘境与收藏两栏', () => {
    const markup = renderView();

    expect(markup).toContain('本周团队秘境');
    expect(markup).toContain('洛阳之战');
    expect(markup).toContain('收藏');
    expect(markup).toContain('阆风悬城');
  });

  it('本周团队秘境加载中时给出提示，不展示失败文案', () => {
    const markup = renderView({ weeklyRaidStatus: 'loading' });

    expect(markup).toContain('正在获取本周团队秘境');
    expect(markup).not.toContain('以下仅显示已收藏副本');
  });

  it('本周团队秘境获取失败时提示仅显示已收藏副本', () => {
    const markup = renderView({ weeklyRaidStatus: 'error' });

    expect(markup).toContain('本周团队秘境获取失败，以下仅显示已收藏副本。');
  });

  it('没有任何周常与收藏时显示空态', () => {
    const markup = renderView({
      weeklyRaidGroups: [],
      favoriteMergedRaids: [],
      favoriteRaids: new Set<string>(),
    });

    expect(markup).toContain('暂无副本数据');
    expect(markup).not.toContain('本周团队秘境');
  });

  it('多个难度存在时难度标签带上人数前缀', () => {
    const multiDifficultyGroup: WeeklyRaidGroup = {
      ...WEEKLY_GROUP,
      raids: [
        RAID,
        { ...RAID, difficulty: '英雄' },
        { ...RAID, playerCount: 10 },
      ],
    };
    const markup = renderView({ weeklyRaidGroups: [multiDifficultyGroup] });

    expect(markup).toContain('10人普通');
    expect(markup).toContain('25人英雄');
  });

  it('账号类型等无关字段不参与渲染', () => {
    const markup = renderView();

    expect(markup).not.toContain(AccountType.CLIENT);
  });
});
