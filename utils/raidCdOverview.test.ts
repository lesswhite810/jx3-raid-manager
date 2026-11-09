/**
 * raidCdOverview 单元测试
 *
 * 概览表存在的意义是「不点进详情也能看出 CD 有没有清」，所以判定口径必须
 * 和 RaidDetail 完全一致：待确认优先、部分 BOSS 算部分清、10 人本不展示代清号。
 * 这些用例锁住上述口径，避免以后两处逻辑漂移。
 */

import { describe, expect, it } from 'vitest';
import { Account, AccountType, Raid, RaidRecord } from '../types';
import { injectDefaultBosses } from '../data/raidBosses';
import { buildRaidCdOverview } from './raidCdOverview';

/** 2026-11-09 是周一，21:00 仍在当周窗口内（周界为周一 07:00） */
const NOW = new Date(2026, 10, 9, 21, 0);
const IN_WEEK = new Date(2026, 10, 9, 10, 0).getTime();
const LAST_WEEK = new Date(2026, 10, 2, 10, 0).getTime();

const SEASON_CONTEXT = { currentSeasonId: 1, currentVersionName: '苍生铸世' };

const RAID_NAME = '洛阳之战';
const BOSS_IDS = [
  'luoyangzhizhan_1',
  'luoyangzhizhan_2',
  'luoyangzhizhan_3',
  'luoyangzhizhan_4',
  'luoyangzhizhan_5',
];

function makeRaid(playerCount: 10 | 25, name = RAID_NAME, difficulty: Raid['difficulty'] = '普通'): Raid {
  return injectDefaultBosses({
    name,
    version: '苍生铸世',
    seasonId: 1,
    playerCount,
    difficulty,
    isActive: true,
  });
}

function makeAccount(overrides: Partial<Account> & { roles: Account['roles'] }): Account {
  return {
    id: 'account-1',
    accountName: '大号',
    type: AccountType.OWN,
    ...overrides,
  };
}

function makeRole(id: string, name = '角色') {
  return { id, name, server: '天鹅坪', region: '电信五区' };
}

function makeRecord(overrides: Partial<RaidRecord> & { roleId: string }): RaidRecord {
  return {
    id: `record-${overrides.roleId}-${overrides.date ?? IN_WEEK}`,
    accountId: 'account-1',
    raidName: '25人普通洛阳之战',
    date: IN_WEEK,
    goldIncome: 0,
    hasXuanjing: false,
    source: 'manual',
    status: 'confirmed',
    ...overrides,
  };
}

/** 一条覆盖全部 5 个 BOSS 的记录 */
function makeFullClear(roleId: string, date = IN_WEEK): RaidRecord {
  return makeRecord({
    roleId,
    date,
    bossIds: BOSS_IDS,
    bossNames: ['突利和顺', '田承嗣', '伊曼·寂夜', '阿史那承庆', '史朝义'],
  });
}

describe('buildRaidCdOverview - 四态判定', () => {
  it('25 人本无本周记录时，角色列为未清', () => {
    const raid = makeRaid(25);
    const account = makeAccount({ roles: [makeRole('role-1', '张三')] });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.columns).toHaveLength(1);
    expect(result.columns[0].key).toBe('25人普通洛阳之战');
    expect(result.rows).toHaveLength(1);
    expect(result.rows[0].cells['25人普通洛阳之战'].status).toBe('none');
    expect(result.rows[0].noneClearedCount).toBe(1);
    expect(result.columns[0].clearedCount).toBe(0);
    expect(result.columns[0].totalCount).toBe(1);
  });

  it('打完全部 BOSS 记为已清', () => {
    const raid = makeRaid(25);
    const account = makeAccount({ roles: [makeRole('role-1', '张三')] });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [makeFullClear('role-1')],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    const cell = result.rows[0].cells['25人普通洛阳之战'];
    expect(cell.status).toBe('complete');
    expect(cell.clearedCount).toBe(5);
    expect(cell.totalCount).toBe(5);
    expect(result.rows[0].completeClearedCount).toBe(1);
    expect(result.columns[0].clearedCount).toBe(1);
  });

  it('只打了部分 BOSS 记为部分清，并带 N/M 计数', () => {
    const raid = makeRaid(25);
    const account = makeAccount({ roles: [makeRole('role-1', '张三')] });
    const partial = makeRecord({
      roleId: 'role-1',
      bossIds: ['luoyangzhizhan_1', 'luoyangzhizhan_2'],
      bossNames: ['突利和顺', '田承嗣'],
    });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [partial],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    const cell = result.rows[0].cells['25人普通洛阳之战'];
    expect(cell.status).toBe('partial');
    expect(cell.clearedCount).toBe(2);
    expect(cell.totalCount).toBe(5);
    expect(result.rows[0].partialClearedCount).toBe(1);
    // 部分清不计入列头的已清数
    expect(result.columns[0].clearedCount).toBe(0);
  });

  it('自动扫描的待确认记录优先显示为待确认，而不是已清', () => {
    const raid = makeRaid(25);
    const account = makeAccount({ roles: [makeRole('role-1', '张三')] });
    const pending = makeFullClear('role-1');
    pending.source = 'auto';
    pending.status = 'pending';

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [pending],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.rows[0].cells['25人普通洛阳之战'].status).toBe('pending');
    expect(result.rows[0].pendingClearedCount).toBe(1);
    expect(result.columns[0].clearedCount).toBe(0);
  });

  it('上周的记录不计入本周 CD', () => {
    const raid = makeRaid(25);
    const account = makeAccount({ roles: [makeRole('role-1', '张三')] });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [makeFullClear('role-1', LAST_WEEK)],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.rows[0].cells['25人普通洛阳之战'].status).toBe('none');
  });
});

describe('buildRaidCdOverview - 角色与列过滤', () => {
  it('10 人本对代清账号角色显示为不适用', () => {
    const raid = makeRaid(10);
    const own = makeAccount({ roles: [makeRole('role-own', '自己的号')] });
    const client = makeAccount({
      id: 'account-2',
      accountName: '老板号',
      type: AccountType.CLIENT,
      roles: [makeRole('role-client', '老板角色')],
    });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [own, client],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    const key = '10人普通洛阳之战';
    expect(result.rows).toHaveLength(2);
    expect(result.rows[1].cells[key].status).toBe('na');
    // 不适用不计入列统计
    expect(result.columns[0].totalCount).toBe(1);
  });

  it('禁用账号、禁用角色与三个模块都隐藏的角色都不出现在表里', () => {
    const raid = makeRaid(25);
    const disabledAccount = makeAccount({
      id: 'account-disabled',
      accountName: '停用账号',
      disabled: true,
      roles: [makeRole('role-1')],
    });
    const account = makeAccount({
      id: 'account-2',
      roles: [
        makeRole('role-2', '禁用角色'),
        // 只有三个模块都关闭时才彻底隐藏；只关团队副本的角色仍会出现在试炼/百战列
        { ...makeRole('role-3', '全隐藏角色'), visibility: { raid: false, trial: false, baizhan: false } },
        makeRole('role-4', '正常角色'),
      ],
    });
    account.roles[0] = { ...account.roles[0], disabled: true };

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [disabledAccount, account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.rows.map(row => row.roleId)).toEqual(['role-4']);
  });

  it('只隐藏团队副本的角色仍保留行，但副本列显示为不适用', () => {
    const raid = makeRaid(25);
    const account = makeAccount({
      roles: [{ ...makeRole('role-1'), visibility: { raid: false } }],
    });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [],
      trialProgressPeriod: { start: LAST_WEEK },
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.rows.map(row => row.roleId)).toEqual(['role-1']);
    expect(result.rows[0].cells['25人普通洛阳之战'].status).toBe('na');
    // 不参团不影响试炼 / 百战列：周期已就绪时必须给出真实状态而不是「不适用」
    expect(result.rows[0].trial.status).not.toBe('na');
    expect(result.rows[0].baizhan.status).not.toBe('na');
  });

  it('被禁用的副本难度不进入列，并统计跳过数量', () => {
    const activeRaid = makeRaid(25);
    const disabledRaid = { ...makeRaid(25, RAID_NAME, '英雄'), isActive: false };
    const account = makeAccount({ roles: [makeRole('role-1')] });

    const result = buildRaidCdOverview({
      raids: [activeRaid, disabledRaid],
      accounts: [account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.columns.map(column => column.key)).toEqual(['25人普通洛阳之战']);
    expect(result.disabledColumnCount).toBe(1);
  });
});

describe('buildRaidCdOverview - 行顺序', () => {
  it('装分高的角色排在前面', () => {
    const raid = makeRaid(25, RAID_NAME);
    const account = makeAccount({
      roles: [
        { ...makeRole('role-low', '低装分'), equipmentScore: 180000 },
        { ...makeRole('role-high', '高装分'), equipmentScore: 240000 },
      ],
    });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.rows.map(row => row.roleId)).toEqual(['role-high', 'role-low']);
  });

  it('未填装分的角色排在最后', () => {
    const raid = makeRaid(25, RAID_NAME);
    const account = makeAccount({
      roles: [
        makeRole('role-none', '无装分'),
        { ...makeRole('role-low', '低装分'), equipmentScore: 1000 },
      ],
    });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.rows.map(row => row.roleId)).toEqual(['role-low', 'role-none']);
  });

  it('装分为 0 与未填装分一样，都排在最后', () => {
    const raid = makeRaid(25, RAID_NAME);
    const account = makeAccount({
      roles: [
        { ...makeRole('role-zero', '零装分'), equipmentScore: 0 },
        { ...makeRole('role-low', '低装分'), equipmentScore: 1000 },
      ],
    });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.rows.map(row => row.roleId)).toEqual(['role-low', 'role-zero']);
  });

  it('装分相同时欠 CD 越多的角色排得越前，方便先补最缺的号', () => {
    const raidA = makeRaid(25, RAID_NAME);
    const raidB = makeRaid(25, '会战弓月城');
    const account = makeAccount({
      roles: [
        { ...makeRole('role-full', '已清号'), equipmentScore: 200000 },
        { ...makeRole('role-none', '未清号'), equipmentScore: 200000 },
      ],
    });

    const result = buildRaidCdOverview({
      raids: [raidA, raidB],
      accounts: [account],
      records: [
        makeFullClear('role-full'),
        makeRecord({ roleId: 'role-full', raidName: '25人普通会战弓月城', bossIds: BOSS_IDS, bossNames: [] }),
      ],
      weeklyRaidNames: [RAID_NAME, '会战弓月城'],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.rows.map(row => row.roleId)).toEqual(['role-none', 'role-full']);
  });

  it('行数据带上装分与账号密码，供装分列与操作列渲染', () => {
    const raid = makeRaid(25, RAID_NAME);
    const account = makeAccount({
      password: 'secret',
      roles: [{ ...makeRole('role-1'), equipmentScore: 213456 }],
    });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.rows[0].equipmentScore).toBe(213456);
    expect(result.rows[0].password).toBe('secret');
    expect(result.rows[0].accountName).toBe('大号');
  });
});

describe('buildRaidCdOverview - 列来源', () => {
  it('本周团队秘境与收藏副本各占一列，重复的副本只出现一次', () => {
    const weeklyRaid = makeRaid(25, RAID_NAME);
    const favoriteRaid = makeRaid(25, '会战弓月城');
    const account = makeAccount({ roles: [makeRole('role-1')] });

    const result = buildRaidCdOverview({
      raids: [weeklyRaid, favoriteRaid],
      accounts: [account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>([RAID_NAME, '会战弓月城']),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.columns.map(column => column.raid.name)).toEqual([RAID_NAME, '会战弓月城']);
    expect(result.columns[0].source).toBe('weekly');
    expect(result.columns[1].source).toBe('favorite');
  });

  it('收藏副本里被禁用的难度不进入列', () => {
    // 收藏副本本身不在本周团队秘境里，只可能在收藏分支进入列
    const favoriteRaid = makeRaid(25, '会战弓月城');
    const disabledFavorite = { ...makeRaid(25, '会战弓月城', '英雄'), isActive: false };
    const account = makeAccount({ roles: [makeRole('role-1')] });

    const result = buildRaidCdOverview({
      raids: [favoriteRaid, disabledFavorite],
      accounts: [account],
      records: [],
      weeklyRaidNames: [],
      favoriteRaids: new Set<string>(['会战弓月城']),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.columns.map(column => column.key)).toEqual(['25人普通会战弓月城']);
    expect(result.columns[0].source).toBe('favorite');
    expect(result.disabledColumnCount).toBe(1);
  });
});

describe('buildRaidCdOverview - 列顺序', () => {
  /** 造一个指定大版本的副本难度 */
  function makeVersionRaid(name: string, version: string, playerCount: 10 | 25, difficulty: Raid['difficulty'] = '普通'): Raid {
    return { ...makeRaid(playerCount, name, difficulty), version, seasonId: undefined };
  }

  it('周常副本按赛季由新到旧排序，同赛季内按人数与难度排序', () => {
    // 本周名单故意按「旧赛季在前」给出，验证排序不受接口返回顺序影响
    const raids = [
      makeVersionRaid('达摩洞', '奉天证道', 10),
      makeVersionRaid('洛阳之战', '苍生铸世', 25, '英雄'),
      makeVersionRaid('洛阳之战', '苍生铸世', 10),
      makeVersionRaid('洛阳之战', '苍生铸世', 25),
      makeVersionRaid('范阳夜变', '世外蓬莱', 10),
    ];
    const account = makeAccount({ roles: [makeRole('role-1')] });

    const result = buildRaidCdOverview({
      raids,
      accounts: [account],
      records: [],
      weeklyRaidNames: ['达摩洞', '洛阳之战', '范阳夜变'],
      favoriteRaids: new Set<string>(),
      seasonContext: { currentSeasonId: 1, currentVersionName: '苍生铸世' },
      versionOrder: { '苍生铸世': 0, '奉天证道': 1, '世外蓬莱': 2 },
      now: NOW,
    });

    expect(result.columns.map(column => column.key)).toEqual([
      '10人普通洛阳之战',
      '25人普通洛阳之战',
      '25人英雄洛阳之战',
      '10人普通达摩洞',
      '10人普通范阳夜变',
    ]);
  });

  it('版本不在已知顺序里时排在最后', () => {
    const raids = [
      makeVersionRaid('洛阳之战', '苍生铸世', 10),
      makeVersionRaid('达摩洞', '未知版本', 10),
    ];
    const account = makeAccount({ roles: [makeRole('role-1')] });

    const result = buildRaidCdOverview({
      raids,
      accounts: [account],
      records: [],
      weeklyRaidNames: ['达摩洞', '洛阳之战'],
      favoriteRaids: new Set<string>(),
      seasonContext: { currentSeasonId: 1, currentVersionName: '苍生铸世' },
      versionOrder: { '苍生铸世': 0 },
      now: NOW,
    });

    expect(result.columns.map(column => column.key)).toEqual(['10人普通洛阳之战', '10人普通达摩洞']);
  });

  it('收藏副本排在周常副本之后', () => {
    const raids = [
      makeVersionRaid('会战弓月城', '横刀断浪', 25),
      makeVersionRaid('洛阳之战', '苍生铸世', 25),
    ];
    const account = makeAccount({ roles: [makeRole('role-1')] });

    const result = buildRaidCdOverview({
      raids,
      accounts: [account],
      records: [],
      weeklyRaidNames: ['洛阳之战'],
      favoriteRaids: new Set<string>(['洛阳之战', '会战弓月城']),
      seasonContext: { currentSeasonId: 1, currentVersionName: '苍生铸世' },
      versionOrder: { '苍生铸世': 0, '横刀断浪': 1 },
      now: NOW,
    });

    expect(result.columns.map(column => column.source)).toEqual(['weekly', 'favorite']);
    expect(result.columns.map(column => column.key)).toEqual(['25人普通洛阳之战', '25人普通会战弓月城']);
  });
});

describe('buildRaidCdOverview - 玩法参与标记', () => {
  it('没有角色参与试炼 / 百战时，对应标记为 false', () => {
    const raid = makeRaid(25);
    const account = makeAccount({
      roles: [{ ...makeRole('role-1'), visibility: { raid: true, trial: false, baizhan: false } }],
    });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.hasTrialParticipants).toBe(false);
    expect(result.hasBaizhanParticipants).toBe(false);
  });

  it('有角色参与试炼 / 百战时，对应标记为 true', () => {
    const raid = makeRaid(25);
    const account = makeAccount({ roles: [makeRole('role-1')] });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.hasTrialParticipants).toBe(true);
    expect(result.hasBaizhanParticipants).toBe(true);
  });

  it('代清号不参与试炼 / 百战，只有代清号时标记为 false', () => {
    const raid = makeRaid(25);
    const account = makeAccount({
      type: AccountType.CLIENT,
      roles: [{ ...makeRole('role-1'), isClient: true }],
    });

    const result = buildRaidCdOverview({
      raids: [raid],
      accounts: [account],
      records: [],
      weeklyRaidNames: [RAID_NAME],
      favoriteRaids: new Set<string>(),
      seasonContext: SEASON_CONTEXT,
      now: NOW,
    });

    expect(result.hasTrialParticipants).toBe(false);
    expect(result.hasBaizhanParticipants).toBe(false);
  });
});
