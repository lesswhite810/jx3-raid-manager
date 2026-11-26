/**
 * RaidDetail 角色卡片凭证区渲染测试
 *
 * 玩家进副本详情页最常做的事就是复制账号密码去上号，所以这里断言
 * 角色卡片上确实带着账号、备注与密码（密码只出掩码）的展示与复制入口。
 */

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Account, AccountType, Raid } from '../types';
import { injectDefaultBosses } from '../data/raidBosses';
import { RaidDetail } from './RaidDetail';

const RAID: Raid = injectDefaultBosses({
  name: '洛阳之战',
  version: '苍生铸世',
  seasonId: 1,
  playerCount: 25,
  difficulty: '普通',
  isActive: true,
});

const OWN_ACCOUNT: Account = {
  id: 'account-1',
  accountName: 'werhua520',
  password: 'secret123',
  type: AccountType.OWN,
  roles: [{ id: 'role-1', name: '张三', server: '天鹅坪', region: '电信五区' }],
};

const CLIENT_ACCOUNT: Account = {
  id: 'account-2',
  accountName: 'daqinghao',
  password: 'clientpw',
  type: AccountType.CLIENT,
  notes: '周三晚才能上',
  roles: [{ id: 'role-2', name: '李四', server: '天鹅坪', region: '电信五区' }],
};

const renderDetail = (accounts: Account[]) =>
  renderToStaticMarkup(
    <RaidDetail
      raid={RAID}
      accounts={accounts}
      records={[]}
      onBack={() => {}}
    />,
  );

describe('RaidDetail - 角色卡片账号与密码', () => {
  it('角色卡片展示账号与密码复制入口，密码只出掩码', () => {
    const markup = renderDetail([OWN_ACCOUNT]);

    expect(markup).toContain('账号');
    expect(markup).toContain('werhua520');
    expect(markup).toContain('复制账号');
    expect(markup).toContain('密码');
    expect(markup).toContain('复制密码');
    // 密码只出掩码，不明文渲染
    expect(markup).not.toContain('secret123');
  });

  it('代清账号的备注一并展示在角色卡片上', () => {
    const markup = renderDetail([CLIENT_ACCOUNT]);

    expect(markup).toContain('daqinghao');
    expect(markup).toContain('备注');
    expect(markup).toContain('周三晚才能上');
  });

  it('账号没有密码时提示未配置密码', () => {
    const markup = renderDetail([{ ...OWN_ACCOUNT, password: undefined }]);

    expect(markup).toContain('未配置密码');
    expect(markup).not.toContain('复制密码');
  });
});
