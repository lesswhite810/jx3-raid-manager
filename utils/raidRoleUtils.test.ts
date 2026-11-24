import { describe, expect, it } from 'vitest';
import { AccountType } from '../types';
import {
  buildRaidRoleVisibilityMap,
  filterRaidRoles,
  getClientAccountNote,
  getRaidClearStats,
  isRaidRoleDisabled,
} from './raidRoleUtils';

describe('buildRaidRoleVisibilityMap', () => {
  it('只把 visible 为 false 的条目计入禁用集合', () => {
    const map = buildRaidRoleVisibilityMap([
      { roleId: 'role-1', raidKey: '25人普通洛阳之战', visible: false },
      { roleId: 'role-2', raidKey: '25人普通洛阳之战', visible: true },
      { roleId: 'role-3', raidKey: '25人英雄洛阳之战', visible: false },
    ]);

    expect(Array.from(map.get('25人普通洛阳之战') ?? [])).toEqual(['role-1']);
    expect(Array.from(map.get('25人英雄洛阳之战') ?? [])).toEqual(['role-3']);
  });

  it('没有任何禁用配置时返回空映射', () => {
    expect(buildRaidRoleVisibilityMap([]).size).toBe(0);
  });
});

describe('isRaidRoleDisabled', () => {
  const map = buildRaidRoleVisibilityMap([
    { roleId: 'role-1', raidKey: '25人普通洛阳之战', visible: false },
  ]);

  it('命中禁用配置时返回 true', () => {
    expect(isRaidRoleDisabled(map, '25人普通洛阳之战', 'role-1')).toBe(true);
  });

  it('未配置的角色和副本都视为可见', () => {
    expect(isRaidRoleDisabled(map, '25人普通洛阳之战', 'role-2')).toBe(false);
    expect(isRaidRoleDisabled(map, '25人英雄洛阳之战', 'role-1')).toBe(false);
    // 可见性数据尚未加载完成时不能把所有角色误判成禁用
    expect(isRaidRoleDisabled(undefined, '25人普通洛阳之战', 'role-1')).toBe(false);
  });
});

describe('getClientAccountNote', () => {
  it('returns trimmed note for client accounts', () => {
    expect(getClientAccountNote(AccountType.CLIENT, '  老板包周常  ')).toBe('老板包周常');
  });

  it('ignores notes for own accounts', () => {
    expect(getClientAccountNote(AccountType.OWN, '本人备注')).toBeUndefined();
  });

  it('ignores blank notes', () => {
    expect(getClientAccountNote(AccountType.CLIENT, '   ')).toBeUndefined();
  });
});

describe('filterRaidRoles', () => {
  const roles = [
    {
      id: '1',
      name: '阿清',
      server: '梦江南',
      region: '电五',
      sect: '奶秀',
      accountName: 'client-account',
      accountNote: '老板包周常'
    },
    {
      id: '2',
      name: '阿花',
      server: '斗转星移',
      region: '双一',
      sect: '花间',
      accountName: 'own-account'
    }
  ];

  it('returns all roles when search is blank', () => {
    expect(filterRaidRoles(roles, '')).toEqual(roles);
  });

  it('matches role name and server', () => {
    expect(filterRaidRoles(roles, '阿花')).toEqual([roles[1]]);
    expect(filterRaidRoles(roles, '梦江南')).toEqual([roles[0]]);
  });

  it('matches account note for client roles', () => {
    expect(filterRaidRoles(roles, '周常')).toEqual([roles[0]]);
  });

  it('matches account name and sect case-insensitively', () => {
    expect(filterRaidRoles(roles, 'OWN-ACCOUNT')).toEqual([roles[1]]);
    expect(filterRaidRoles(roles, '花间')).toEqual([roles[1]]);
  });
});

describe('getRaidClearStats', () => {
  const roles = [
    {
      id: 'enabled-none',
      canRun: true
    },
    {
      id: 'enabled-partial',
      canRun: true,
      bossCooldowns: [
        { hasRecord: true },
        { hasRecord: false }
      ]
    },
    {
      id: 'enabled-complete',
      canRun: false
    },
    {
      id: 'enabled-pending',
      canRun: false,
      hasPendingRecord: true
    },
    {
      id: 'disabled-complete',
      canRun: false,
      bossCooldowns: [
        { hasRecord: true },
        { hasRecord: true }
      ]
    }
  ];

  it('excludes disabled raid roles from clear-state counts', () => {
    expect(getRaidClearStats(roles, {
      'disabled-complete': false
    })).toEqual({
      noneClearedCount: 1,
      partialClearedCount: 1,
      pendingClearedCount: 1,
      completeClearedCount: 1
    });
  });

  it('counts disabled roles again when they are enabled', () => {
    expect(getRaidClearStats(roles, {})).toEqual({
      noneClearedCount: 1,
      partialClearedCount: 1,
      pendingClearedCount: 1,
      completeClearedCount: 2
    });
  });
});
