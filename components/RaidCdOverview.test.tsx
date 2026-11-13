/**
 * RaidCdOverview 渲染测试
 *
 * 概览表的价值在于「不点进去就能看出哪个号没清」，所以这里直接断言渲染出来的
 * 文案与配色，确保未清、部分清、已清三种状态在界面上真的能区分开。
 */

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Account, AccountType, BaizhanRecord, Raid, RaidRecord, TrialPlaceRecord } from '../types';
import { injectDefaultBosses } from '../data/raidBosses';
import { RaidCdOverview } from './RaidCdOverview';
import { TrialProgressPeriod } from '../utils/trialProgress';

const BOSS_IDS = [
  'luoyangzhizhan_1',
  'luoyangzhizhan_2',
  'luoyangzhizhan_3',
  'luoyangzhizhan_4',
  'luoyangzhizhan_5',
];

const RAID: Raid = injectDefaultBosses({
  name: '洛阳之战',
  version: '苍生铸世',
  seasonId: 1,
  playerCount: 25,
  difficulty: '普通',
  isActive: true,
});

const ACCOUNT: Account = {
  id: 'account-1',
  accountName: '大号',
  type: AccountType.OWN,
  roles: [{ id: 'role-1', name: '张三', server: '天鹅坪', region: '电信五区' }],
};

const SEASON_CONTEXT = { currentSeasonId: 1, currentVersionName: '苍生铸世' };
const TRIAL_PERIOD: TrialProgressPeriod = { start: new Date(2026, 10, 2, 7, 0).getTime() };

function makeRecord(overrides: Partial<RaidRecord> = {}): RaidRecord {
  return {
    id: 'record-1',
    accountId: 'account-1',
    roleId: 'role-1',
    raidName: '25人普通洛阳之战',
    date: new Date().getTime(),
    goldIncome: 0,
    hasXuanjing: false,
    source: 'manual',
    status: 'confirmed',
    ...overrides,
  };
}

function makeTrialRecord(overrides: Partial<TrialPlaceRecord> = {}): TrialPlaceRecord {
  return {
    id: 'trial-1',
    accountId: 'account-1',
    roleId: 'role-1',
    roleName: '张三',
    server: '天鹅坪',
    date: new Date().getTime(),
    layer: 1,
    bosses: [],
    // 已知牌位，否则该轮会计入「待确认」而掩盖本次要断言的三态
    flippedIndex: 1,
    type: 'trial',
    ...overrides,
  } as TrialPlaceRecord;
}

function makeBaizhanRecord(overrides: Partial<BaizhanRecord> = {}): BaizhanRecord {
  return {
    id: 'baizhan-1',
    accountId: 'account-1',
    roleId: 'role-1',
    roleName: '张三',
    server: '天鹅坪',
    date: new Date().getTime(),
    goldIncome: 0,
    type: 'baizhan',
    ...overrides,
  };
}

const noop = () => undefined;
const noopConfirmRaid = (_scope: { roleId: string; raid: Raid }) => undefined;

interface RenderOptions {
  records?: RaidRecord[];
  trialRecords?: TrialPlaceRecord[];
  baizhanRecords?: BaizhanRecord[];
  account?: Account | null;
  /** 'loading' 用来模拟「周期尚未就绪」，对应组件收到 undefined 的场景 */
  trialProgressPeriod?: TrialProgressPeriod | null | 'loading';
}

const renderOverview = ({
  records = [],
  trialRecords = [],
  baizhanRecords = [],
  account = ACCOUNT,
  trialProgressPeriod = TRIAL_PERIOD,
}: RenderOptions = {}): string => {
  // 注意：解构默认值会把显式传入的 undefined 也替换掉，所以「加载中」用哨兵值区分
  const resolvedPeriod = trialProgressPeriod === 'loading' ? undefined : trialProgressPeriod;
  return renderToStaticMarkup(
    <RaidCdOverview
      raids={[RAID]}
      accounts={account ? [account] : []}
      records={records}
      trialRecords={trialRecords}
      baizhanRecords={baizhanRecords}
      trialProgressPeriod={resolvedPeriod}
      weeklyRaidNames={['洛阳之战']}
      favoriteRaids={new Set<string>()}
      seasonContext={SEASON_CONTEXT}
      onOpenRaid={noop}
      onOpenTrial={noop}
      onOpenBaizhan={noop}
      onRequestConfirmRaid={noopConfirmRaid}
      onOpenRaidCell={noop}
      onOpenTrialCell={noop}
      onOpenBaizhanCell={noop}
    />,
  );
};

describe('RaidCdOverview', () => {
  it('展示概览标题、副本列与未清状态', () => {
    const markup = renderOverview();

    expect(markup).toContain('CD 概览');
    expect(markup).toContain('洛阳之战');
    expect(markup).toContain('25人普通');
    expect(markup).toContain('张三');
    expect(markup).toContain('未清');
    expect(markup).toContain('bg-ds-success-soft');
  });

  it('打完全部 BOSS 后该格子显示为已清', () => {
    const markup = renderOverview({
      records: [
        makeRecord({ bossIds: BOSS_IDS, bossNames: ['突利和顺', '田承嗣', '伊曼·寂夜', '阿史那承庆', '史朝义'] }),
      ],
      // 试炼与百战也打满，避免它们各自的「未清」绿色底干扰本用例的配色断言
      trialRecords: [
        makeTrialRecord(),
        makeTrialRecord({ id: 'trial-2' }),
        makeTrialRecord({ id: 'trial-3' }),
      ],
      baizhanRecords: [makeBaizhanRecord()],
    });

    expect(markup).toContain('已清');
    // 已清的格子用中性灰，不再出现「未清」的绿色底
    expect(markup).not.toContain('bg-ds-success-soft');
  });

  it('只打部分 BOSS 时用 已清/总数 的形式提示进度', () => {
    const markup = renderOverview({
      records: [makeRecord({ bossIds: BOSS_IDS.slice(0, 2), bossNames: ['突利和顺', '田承嗣'] })],
    });

    expect(markup).toContain('2/5');
    expect(markup).toContain('bg-ds-info-soft');
  });

  it('有待确认记录时格子显示待确认，并用警示配色', () => {
    const markup = renderOverview({
      records: [makeRecord({ source: 'auto', status: 'pending' })],
    });

    expect(markup).toContain('待确认');
    expect(markup).toContain('bg-ds-warning-soft');
  });

  it('装分列展示装分，未填写时显示占位符', () => {
    const scored = renderOverview({
      account: { ...ACCOUNT, roles: [{ ...ACCOUNT.roles[0], equipmentScore: 213456 }] },
    });
    expect(scored).toContain('装分');
    expect(scored).toContain('213,456');

    const unscored = renderOverview();
    expect(unscored).toContain('—');
  });

  it('账号未配置密码时不渲染复制密码按钮', () => {
    const markup = renderOverview();

    expect(markup).toContain('复制账号');
    expect(markup).not.toContain('复制密码');
  });

  it('账号配置了密码时渲染复制密码按钮', () => {
    const markup = renderOverview({ account: { ...ACCOUNT, password: 'secret' } });

    expect(markup).toContain('复制账号');
    expect(markup).toContain('复制密码');
  });

  it('试炼之地与百战列：按本周次数展示进度', () => {
    const markup = renderOverview({
      trialRecords: [makeTrialRecord(), makeTrialRecord({ id: 'trial-2' })],
      baizhanRecords: [makeBaizhanRecord()],
    });

    expect(markup).toContain('试炼之地');
    expect(markup).toContain('百战');
    // 试炼本周目标 3 次，已完成 2 次 → 部分清
    expect(markup).toContain('2/3');
    // 百战每周 1 次，已完成 1 次 → 已清
    expect(markup).toContain('1/1');
  });

  it('试炼进度周期未加载完成时该列按不适用处理，不误报未清', () => {
    const markup = renderOverview({ trialProgressPeriod: 'loading' });

    expect(markup).toContain('试炼之地');
    // 周期未知时不能把「还没加载完」显示成「本周 0/3 没打」
    expect(markup).not.toContain('0/3');
  });

  it('角色只关闭团队副本可见性时仍会出现在概览里（试炼/百战仍在参与）', () => {
    const markup = renderOverview({
      account: { ...ACCOUNT, roles: [{ ...ACCOUNT.roles[0], visibility: { raid: false } }] },
    });

    expect(markup).toContain('张三');
  });

  it('三个模块都被关闭的角色不渲染，整块为空', () => {
    const markup = renderOverview({
      account: {
        ...ACCOUNT,
        roles: [{ ...ACCOUNT.roles[0], visibility: { raid: false, trial: false, baizhan: false } }],
      },
    });

    expect(markup).toBe('');
  });

  it('没有账号时整块不渲染，避免占位', () => {
    expect(renderOverview({ account: null })).toBe('');
  });

  it('未清单元格提示点击添加记录', () => {
    const markup = renderOverview();

    expect(markup).toContain('点击添加记录');
    expect(markup).not.toContain('点击查看记录');
  });

  it('已清单元格提示点击查看记录', () => {
    const markup = renderOverview({
      records: [
        makeRecord({ bossIds: BOSS_IDS, bossNames: ['突利和顺', '田承嗣', '伊曼·寂夜', '阿史那承庆', '史朝义'] }),
      ],
      trialRecords: [
        makeTrialRecord(),
        makeTrialRecord({ id: 'trial-2' }),
        makeTrialRecord({ id: 'trial-3' }),
      ],
      baizhanRecords: [makeBaizhanRecord()],
    });

    expect(markup).toContain('点击查看记录');
  });

  it('待确认单元格提示点击处理，不再提示添加或查看', () => {
    const markup = renderOverview({ records: [makeRecord({ source: 'auto', status: 'pending' })] });

    expect(markup).toContain('点击处理');
  });

  it('试炼之地待确认走本模块的查看并确认，不再复用团队副本待确认入口', () => {
    const markup = renderOverview({
      trialRecords: [makeTrialRecord({ status: 'pending' })],
    });

    // 试炼待确认有独立的入口文案
    expect(markup).toContain('试炼之地有记录待确认（点击查看并确认）');
    // 没有团队副本待确认记录时，不应出现团队副本专用的处理文案
    expect(markup).not.toContain('点击处理');
  });

  it('团队副本待确认仍走团队副本的处理入口，不出现试炼的确认文案', () => {
    const markup = renderOverview({
      records: [makeRecord({ source: 'auto', status: 'pending' })],
    });

    expect(markup).toContain('点击处理');
    expect(markup).not.toContain('点击查看并确认');
  });

  it('团队副本待确认单元格的提示点明只处理该角色该副本的记录', () => {
    const markup = renderOverview({
      records: [makeRecord({ source: 'auto', status: 'pending' })],
    });

    // 用户要求：点某角色某副本的「待确认」，弹窗只显示对应角色对应副本的记录
    expect(markup).toContain('（点击处理该角色该副本的待确认记录）');
  });
});
