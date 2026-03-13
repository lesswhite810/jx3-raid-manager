import { describe, expect, it } from 'vitest';
import { AccountType } from '../types';
import { filterRaidRoles, getClientAccountNote } from './raidRoleUtils';

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
