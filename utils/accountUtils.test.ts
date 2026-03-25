import { describe, expect, it } from 'vitest';
import { Account, AccountType } from '../types';
import {
  canStartAccountDrag,
  getAccountReorderAnimationDuration,
  reorderAccounts,
  sortAccounts,
} from './accountUtils';

describe('sortAccounts', () => {
  it('优先按自定义排序字段排序，禁用状态不覆盖手动顺序', () => {
    const accounts = [
      {
        id: 'disabled-first',
        accountName: '禁用账号',
        type: AccountType.OWN,
        roles: [],
        disabled: true,
        sortOrder: 0,
      },
      {
        id: 'own-second',
        accountName: '本人账号',
        type: AccountType.OWN,
        roles: [],
        sortOrder: 1,
      },
      {
        id: 'client-third',
        accountName: '代清账号',
        type: AccountType.CLIENT,
        roles: [],
        sortOrder: 2,
      },
    ] as Account[];

    expect(sortAccounts(accounts).map(account => account.id)).toEqual([
      'disabled-first',
      'own-second',
      'client-third',
    ]);
  });
});

describe('reorderAccounts', () => {
  it('支持全部账号按拖拽顺序重排', () => {
    const accounts = [
      {
        id: 'own-1',
        accountName: '本人甲',
        type: AccountType.OWN,
        roles: [],
      },
      {
        id: 'client-1',
        accountName: '代清甲',
        type: AccountType.CLIENT,
        roles: [],
      },
      {
        id: 'own-2',
        accountName: '本人乙',
        type: AccountType.OWN,
        roles: [],
      },
      {
        id: 'client-2',
        accountName: '代清乙',
        type: AccountType.CLIENT,
        roles: [],
      },
      {
        id: 'client-3',
        accountName: '代清丙',
        type: AccountType.CLIENT,
        roles: [],
      },
    ] as Account[];

    expect(
      reorderAccounts(accounts, 'client-3', 'own-1').map(account => account.id),
    ).toEqual(['client-3', 'own-1', 'client-1', 'own-2', 'client-2']);
  });
});

describe('canStartAccountDrag', () => {
  it('普通账号头部区域允许开始拖拽', () => {
    expect(
      canStartAccountDrag({
        closest: (selector: string) => (selector === '[data-no-account-drag="true"]' ? null : null),
      }),
    ).toBe(true);
  });

  it('按钮和输入控件区域不允许开始拖拽', () => {
    const interactiveTarget = {
      closest: (selector: string) =>
        selector === '[data-no-account-drag="true"]' ? { tagName: 'BUTTON' } : null,
    };

    expect(canStartAccountDrag(interactiveTarget)).toBe(false);
  });
});

describe('getAccountReorderAnimationDuration', () => {
  it('短距离重排使用较短过渡时长', () => {
    expect(getAccountReorderAnimationDuration(0)).toBe(170);
  });

  it('位移越大时过渡时长越长', () => {
    expect(getAccountReorderAnimationDuration(240)).toBeGreaterThan(
      getAccountReorderAnimationDuration(40),
    );
  });

  it('超大位移会限制在上限时长内', () => {
    expect(getAccountReorderAnimationDuration(2000)).toBe(280);
  });
});
