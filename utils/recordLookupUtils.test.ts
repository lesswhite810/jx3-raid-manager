import { describe, expect, it } from 'vitest';
import { Account, AccountType, Role, TrialPlaceRecord } from '../types';
import {
  buildClientAccountIdSet,
  buildEquipmentLookup,
  buildRoleInfoLookup,
  getEquipmentById,
  getVisibleRecordRange
} from './recordLookupUtils';

const createRole = (overrides: Partial<Role> = {}): Role => ({
  id: 'role-1',
  name: '角色一',
  region: '电信一区',
  server: '梦江南',
  ...overrides
});

const createAccount = (overrides: Partial<Account> = {}): Account => ({
  id: 'account-1',
  accountName: '账号一',
  type: AccountType.OWN,
  roles: [createRole()],
  ...overrides
});

describe('record lookup helpers', () => {
  it('builds role info by account and role id', () => {
    const lookup = buildRoleInfoLookup([
      createAccount({
        id: 'account-1',
        roles: [createRole({ id: 'role-1', name: '角色一' })]
      })
    ]);

    expect(lookup.get('account-1::role-1')).toEqual({
      roleName: '角色一',
      server: '电信一区 梦江南'
    });
  });

  it('keeps only enabled client accounts in a Set', () => {
    const clientIds = buildClientAccountIdSet([
      createAccount({ id: 'own', type: AccountType.OWN }),
      createAccount({ id: 'client-enabled', type: AccountType.CLIENT }),
      createAccount({ id: 'client-disabled', type: AccountType.CLIENT, disabled: true })
    ]);

    expect(clientIds.has('client-enabled')).toBe(true);
    expect(clientIds.has('client-disabled')).toBe(false);
    expect(clientIds.has('own')).toBe(false);
  });

  it('indexes equipment ids and underscore ids for constant-time lookup', () => {
    const lookup = buildEquipmentLookup([
      { ID: 1001, BindType: 1 },
      { ID: 'prefix_2002', BindType: 2 }
    ]);

    expect(getEquipmentById(lookup, '1001')).toEqual({ ID: 1001, BindType: 1 });
    expect(getEquipmentById(lookup, 'item_1001')).toEqual({ ID: 1001, BindType: 1 });
    expect(getEquipmentById(lookup, 'prefix_2002')).toEqual({ ID: 'prefix_2002', BindType: 2 });
    expect(getEquipmentById(lookup, '')).toBeNull();
  });

  it('calculates stable virtualized ranges with overscan', () => {
    expect(getVisibleRecordRange({
      totalCount: 100,
      scrollTop: 160,
      viewportHeight: 240,
      rowHeight: 80,
      overscan: 1
    })).toEqual({ startIndex: 1, endIndex: 6, topPadding: 80, bottomPadding: 7520 });
  });

  it('returns an empty virtualized range when there are no records', () => {
    expect(getVisibleRecordRange({
      totalCount: 0,
      scrollTop: 160,
      viewportHeight: 240,
      rowHeight: 80,
      overscan: 1
    })).toEqual({ startIndex: 0, endIndex: 0, topPadding: 0, bottomPadding: 0 });
  });
});
