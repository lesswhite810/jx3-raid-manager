import { describe, expect, it } from 'vitest';
import type { Boss, BossRecord, Raid } from '../types';
import { calculateBossCooldowns } from './bossCooldownManager';
import {
  getPendingRecordBossConfirmFields,
  getPendingRecordBossSelection,
  isPendingRecordBossSelected,
  togglePendingRecordBossSelection,
} from './pendingRecordBossSelection';

const bosses: Boss[] = [
  { id: 'luoyangzhizhan_1', name: '突利和顺', order: 1 },
  { id: 'luoyangzhizhan_5', name: '史朝义', order: 5 },
];

describe('pending record BOSS selection', () => {
  it('saves an edited 史朝义 selection with its ID and name', () => {
    const selection = togglePendingRecordBossSelection(
      { bossIds: [], bossNames: [] },
      bosses[1],
      bosses,
    );

    expect(getPendingRecordBossConfirmFields(selection)).toEqual({
      bossIds: ['luoyangzhizhan_5'],
      bossNames: ['史朝义'],
      bossId: 'luoyangzhizhan_5',
      bossName: '史朝义',
    });
  });

  it('recognizes a name-only scan result and adds its ID after another edit', () => {
    const selection = getPendingRecordBossSelection({ bossNames: ['史朝义'] });
    expect(isPendingRecordBossSelected(selection, bosses[1])).toBe(true);

    const updated = togglePendingRecordBossSelection(selection, bosses[0], bosses);
    expect(getPendingRecordBossConfirmFields(updated)).toMatchObject({
      bossIds: ['luoyangzhizhan_1', 'luoyangzhizhan_5'],
      bossNames: ['突利和顺', '史朝义'],
    });
  });

  it('clears the legacy single-BOSS fallback when all selected bosses are removed', () => {
    const selection = getPendingRecordBossSelection({
      bossIds: [],
      bossNames: [],
      bossId: 'luoyangzhizhan_5',
      bossName: '史朝义',
    });
    const updated = togglePendingRecordBossSelection(selection, bosses[1], bosses);

    expect(getPendingRecordBossConfirmFields(updated)).toEqual({
      bossIds: [],
      bossNames: [],
      bossId: '',
      bossName: '',
    });
  });

  it('keeps unmapped values paired while a configured BOSS is edited', () => {
    const updated = togglePendingRecordBossSelection(
      { bossIds: ['custom_1'], bossNames: ['自定义首领'] },
      bosses[1],
      bosses,
    );

    expect(updated).toEqual({
      bossIds: ['luoyangzhizhan_5', 'custom_1'],
      bossNames: ['史朝义', '自定义首领'],
    });
  });

  it('never creates a blank ID for an unmatched historical name and preserves order on a second edit', () => {
    const firstEdit = togglePendingRecordBossSelection(
      { bossIds: ['custom_1'], bossNames: ['自定义首领', '仅有名称的历史首领'] },
      bosses[1],
      bosses,
    );
    expect(firstEdit).toEqual({
      bossIds: ['luoyangzhizhan_5', 'custom_1'],
      bossNames: ['史朝义', '自定义首领', '仅有名称的历史首领'],
    });
    expect(firstEdit.bossIds.every(Boolean)).toBe(true);

    const secondEdit = togglePendingRecordBossSelection(firstEdit, bosses[0], bosses);
    expect(secondEdit).toEqual({
      bossIds: ['luoyangzhizhan_1', 'luoyangzhizhan_5', 'custom_1'],
      bossNames: ['突利和顺', '史朝义', '自定义首领', '仅有名称的历史首领'],
    });
  });

  it('removes a mismatched legacy ID when its paired configured BOSS is deselected', () => {
    const updated = togglePendingRecordBossSelection(
      { bossIds: ['custom_5'], bossNames: ['史朝义'] },
      bosses[1],
      bosses,
    );
    const confirmed = getPendingRecordBossConfirmFields(updated);
    expect(confirmed).toEqual({
      bossIds: [],
      bossNames: [],
      bossId: '',
      bossName: '',
    });

    const raid: Raid = {
      name: '洛阳之战',
      difficulty: '英雄',
      playerCount: 25,
      isActive: true,
      bosses,
    };
    const record: BossRecord = {
      id: 'record-1',
      raidRecordId: 'record-1',
      bossId: confirmed.bossId ?? '',
      bossName: confirmed.bossName ?? '',
      bossIds: confirmed.bossIds,
      bossNames: confirmed.bossNames,
      date: '2026-11-02T12:00:00+08:00',
      roleId: 'role-1',
      accountId: 'account-1',
    };

    const cooldowns = calculateBossCooldowns(
      raid,
      [record],
      'role-1',
      new Date('2026-11-02T12:00:00+08:00'),
    );
    expect(cooldowns.find(item => item.bossId === 'luoyangzhizhan_5')?.hasRecord).toBe(false);
  });
});
