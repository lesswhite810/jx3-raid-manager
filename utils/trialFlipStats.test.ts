import { describe, expect, it } from 'vitest';
import { TrialPlaceRecord } from '../types';
import { calculateTrialFlipStats, findTrialBossEquipmentStats } from './trialFlipStats';

const createTrialRecord = (overrides: Partial<TrialPlaceRecord> = {}): TrialPlaceRecord => ({
  id: 'record-1',
  accountId: 'account-1',
  roleId: 'role-1',
  roleName: '阿试',
  server: '梦江南',
  date: Date.now(),
  layer: 10,
  bosses: ['一号', '二号', '三号'],
  card1: '',
  card2: '',
  card3: '',
  card4: '',
  card5: '',
  flippedIndex: 1,
  type: 'trial',
  ...overrides
});

describe('calculateTrialFlipStats', () => {
  it('counts flipped equipment when flipped position contains equipment', () => {
    const stats = calculateTrialFlipStats([
      createTrialRecord({
        card1: '1001',
        flippedIndex: 1
      })
    ]);

    expect(stats.totalRecords).toBe(1);
    expect(stats.positions[0]).toMatchObject({
      position: 1,
      flipCount: 1,
      flippedEquipmentCount: 1,
      appearanceCount: 1,
      flipEquipmentRate: 1,
      appearanceRate: 1
    });
  });

  it('separates flipped count from equipment appearance on other positions', () => {
    const stats = calculateTrialFlipStats([
      createTrialRecord({
        card3: '3001',
        flippedIndex: 1
      })
    ]);

    expect(stats.positions[0]).toMatchObject({
      position: 1,
      flipCount: 1,
      flippedEquipmentCount: 0,
      appearanceCount: 0,
      flipEquipmentRate: 0,
      appearanceRate: 0
    });
    expect(stats.positions[2]).toMatchObject({
      position: 3,
      flipCount: 0,
      flippedEquipmentCount: 0,
      appearanceCount: 1,
      flipEquipmentRate: 0,
      appearanceRate: 1
    });
  });

  it('aggregates mixed records across all five positions', () => {
    const stats = calculateTrialFlipStats([
      createTrialRecord({ id: '1', card1: '1001', card3: '3001', flippedIndex: 1 }),
      createTrialRecord({ id: '2', card2: '2001', flippedIndex: 2 }),
      createTrialRecord({ id: '3', card2: '2002', card5: '5001', flippedIndex: 4 })
    ]);

    expect(stats.totalRecords).toBe(3);
    expect(stats.positions[0]).toMatchObject({
      position: 1,
      flipCount: 1,
      flippedEquipmentCount: 1,
      appearanceCount: 1
    });
    expect(stats.positions[1]).toMatchObject({
      position: 2,
      flipCount: 1,
      flippedEquipmentCount: 1,
      appearanceCount: 2
    });
    expect(stats.positions[3]).toMatchObject({
      position: 4,
      flipCount: 1,
      flippedEquipmentCount: 0,
      appearanceCount: 0
    });
    expect(stats.positions[4]).toMatchObject({
      position: 5,
      appearanceCount: 1
    });
  });

  it('returns zeroed positions for empty input', () => {
    const stats = calculateTrialFlipStats([]);

    expect(stats.totalRecords).toBe(0);
    expect(stats.positions).toHaveLength(5);
    expect(stats.positions.every(position => position.flipCount === 0 && position.appearanceCount === 0)).toBe(true);
    expect(stats.bestFlipPosition).toBeNull();
    expect(stats.bestAppearancePosition).toBeNull();
    expect(stats.bossEquipmentStats).toEqual([]);
  });

  it('reads legacy snake_case card fields via shared trial record utils', () => {
    const record = {
      ...createTrialRecord({
        card1: '',
        card2: '',
        card3: '',
        card4: '',
        card5: '',
        flippedIndex: 3
      }),
      card_3: '3003',
      card_4: '4004'
    } as TrialPlaceRecord & {
      card_3?: string;
      card_4?: string;
    };

    const stats = calculateTrialFlipStats([record]);
    expect(stats.positions[2]).toMatchObject({
      position: 3,
      flipCount: 1,
      flippedEquipmentCount: 1,
      appearanceCount: 1
    });
    expect(stats.positions[3]).toMatchObject({
      position: 4,
      appearanceCount: 1
    });
  });

  it('counts equipment positions by ordered boss sequence', () => {
    const records = [
      createTrialRecord({
        id: 'boss-1',
        bosses: ['寅', '叶冬辰', '韦柔丝'],
        card2: 'equip-a',
        card4: 'equip-b',
        flippedIndex: 2
      }),
      createTrialRecord({
        id: 'boss-2',
        bosses: ['寅', '叶冬辰', '韦柔丝'],
        card2: 'equip-c',
        flippedIndex: 1
      }),
      createTrialRecord({
        id: 'boss-3',
        bosses: ['雷神', '叶冬辰', '韦柔丝'],
        card5: 'equip-d',
        flippedIndex: 5
      })
    ];

    const stats = calculateTrialFlipStats(records);
    const bossStats = findTrialBossEquipmentStats(records, ['寅', '叶冬辰', '韦柔丝']);

    expect(stats.bossEquipmentStats).toHaveLength(2);
    expect(bossStats?.totalRecords).toBe(2);
    expect(bossStats?.equipmentCount).toBe(3);
    expect(bossStats?.bestEquipmentPosition?.position).toBe(2);
    expect(bossStats?.positions.find(position => position.position === 2)?.appearanceCount).toBe(2);
  });

  it('keeps boss order as part of the grouping key', () => {
    const records = [
      createTrialRecord({
        id: 'order-1',
        bosses: ['寅', '叶冬辰', '韦柔丝'],
        card2: 'equip-a',
        flippedIndex: 2
      }),
      createTrialRecord({
        id: 'order-2',
        bosses: ['寅', '韦柔丝', '叶冬辰'],
        card3: 'equip-b',
        flippedIndex: 3
      })
    ];

    const firstOrder = findTrialBossEquipmentStats(records, ['寅', '叶冬辰', '韦柔丝']);
    const secondOrder = findTrialBossEquipmentStats(records, ['寅', '韦柔丝', '叶冬辰']);

    expect(firstOrder?.bestEquipmentPosition?.position).toBe(2);
    expect(secondOrder?.bestEquipmentPosition?.position).toBe(3);
  });

  it('keeps confirmed parent completions separate from unknown-position flip metrics', () => {
    const stats = calculateTrialFlipStats([
      createTrialRecord({ id: 'confirmed', card1: '1001', flippedIndex: 1, status: 'confirmed' }),
      createTrialRecord({ id: 'pending', card2: '2001', flippedIndex: 2, status: 'pending' }),
      createTrialRecord({ id: 'unknown-flip', card3: '3001', flippedIndex: 0, status: 'confirmed' }),
    ]);

    expect(stats.totalRecords).toBe(2);
    expect(stats.drawCount).toBe(1);
    expect(stats.knownPositionDrawCount).toBe(1);
    expect(stats.positions[0]).toMatchObject({ flipCount: 1, flippedEquipmentCount: 1, appearanceCount: 1 });
    expect(stats.positions[1]).toMatchObject({ flipCount: 0, appearanceCount: 0 });
    expect(stats.positions[2]).toMatchObject({ flipCount: 0, appearanceCount: 0 });
    expect(stats.bossEquipmentStats[0]).toMatchObject({ totalRecords: 1, equipmentCount: 1 });
  });

  it('keeps legacy manual records without equipment as confirmed no-equipment samples', () => {
    const stats = calculateTrialFlipStats([
      createTrialRecord({ id: 'auto', flippedIndex: 2 }),
      createTrialRecord({ id: 'manual-no-equipment', flippedIndex: 1 }),
    ]);

    expect(stats.totalRecords).toBe(2);
    expect(stats.equipmentReviewedRecords).toBe(2);
    expect(stats.positions[0]).toMatchObject({
      flipCount: 1,
      equipmentReviewCount: 1,
      flippedEquipmentCount: 0,
      flipEquipmentRate: 0,
      appearanceRate: 0,
    });
    expect(stats.positions[1]).toMatchObject({
      flipCount: 1,
      equipmentReviewCount: 1,
      flipEquipmentRate: 0,
    });
    expect(stats.bestFlipPosition?.position).toBe(1);
  });

  it('counts one completion with multiple reliable draws without double-counting parent cards', () => {
    const record = {
      ...createTrialRecord({
        source: 'auto',
        flippedIndex: 5,
        card5: 'parent-card-must-not-be-counted',
      }),
      draws: [
        {
          card1: '', card2: '', card3: '', card4: '', card5: '',
          flippedIndex: 0,
        },
        {
          card1: '', card2: 'second-draw-selected', card3: '', card4: '', card5: '',
          flippedIndex: 2,
        },
        {
          card1: '', card2: 'third-draw-observed-only', card3: '', card4: '', card5: '',
          flippedIndex: 2,
        },
        {
          card1: '', card2: '', card3: '', card4: '', card5: 'fourth-draw-other-position',
          flippedIndex: 4,
        },
      ],
    } as TrialPlaceRecord & { draws: Array<{
      card1: string; card2: string; card3: string; card4: string; card5: string;
      flippedIndex: number;
    }> };

    const stats = calculateTrialFlipStats([record]);

    expect(stats.totalRecords).toBe(1);
    expect(stats.drawCount).toBe(4);
    expect(stats.knownPositionDrawCount).toBe(3);
    expect(stats.equipmentReviewedRecords).toBe(3);
    expect(stats.positions[1]).toMatchObject({
      flipCount: 2,
      equipmentReviewCount: 2,
      flippedEquipmentCount: 2,
      appearanceCount: 2,
    });
    expect(stats.positions[3]).toMatchObject({
      flipCount: 1,
      equipmentReviewCount: 1,
      flippedEquipmentCount: 0,
      appearanceCount: 0,
    });
    expect(stats.positions[4]).toMatchObject({ appearanceCount: 1 });
    expect(stats.positions[4].appearanceCount).not.toBe(2);
    expect(stats.bossEquipmentStats[0]).toMatchObject({ totalRecords: 3, equipmentCount: 3 });
  });

  it('does not double-count unassigned manual draws when automatic receipts exist in the same round', () => {
    const record = {
      ...createTrialRecord({ source: 'auto', flippedIndex: 0 }),
      draws: [
        {
          card1: 'manual-card', card2: '', card3: '', card4: '', card5: '',
          flippedIndex: 1, source: 'manual' as const,
        },
        {
          card1: '', card2: 'auto-card', card3: '', card4: '', card5: '',
          flippedIndex: 2, source: 'auto' as const,
        },
      ],
    } as TrialPlaceRecord & { draws: Array<{
      card1: string; card2: string; card3: string; card4: string; card5: string;
      flippedIndex: number; source: 'manual' | 'auto';
    }> };

    const stats = calculateTrialFlipStats([record]);
    expect(stats.totalRecords).toBe(1);
    expect(stats.drawCount).toBe(1);
    expect(stats.knownPositionDrawCount).toBe(1);
    expect(stats.equipmentReviewedRecords).toBe(1);
    expect(stats.positions[0]).toMatchObject({ flipCount: 0, appearanceCount: 0 });
    expect(stats.positions[1]).toMatchObject({ flipCount: 1, flippedEquipmentCount: 1, appearanceCount: 1 });
  });

  it('keeps old manual single-card statistics when the migrated draw array is empty', () => {
    const record = {
      ...createTrialRecord({ source: 'manual', flippedIndex: 1, card1: 'legacy-manual-card' }),
      draws: [],
    } as TrialPlaceRecord & { draws: unknown[] };

    const stats = calculateTrialFlipStats([record]);
    expect(stats.totalRecords).toBe(1);
    expect(stats.drawCount).toBe(1);
    expect(stats.knownPositionDrawCount).toBe(1);
    expect(stats.positions[0]).toMatchObject({ flipCount: 1, appearanceCount: 1 });
  });

  it('keeps a historical automatic single-card record when its draw array is empty', () => {
    const record = {
      ...createTrialRecord({ source: 'auto', flippedIndex: 2, card2: 'stale-parent-card' }),
      draws: [],
    } as TrialPlaceRecord & { draws: unknown[] };

    const stats = calculateTrialFlipStats([record]);
    expect(stats.totalRecords).toBe(1);
    expect(stats.drawCount).toBe(1);
    expect(stats.knownPositionDrawCount).toBe(1);
    expect(stats.equipmentReviewedRecords).toBe(1);
    expect(stats.positions[1]).toMatchObject({ flipCount: 1, appearanceCount: 1 });
  });
});
