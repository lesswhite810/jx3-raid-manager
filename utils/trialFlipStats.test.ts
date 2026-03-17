import { describe, expect, it } from 'vitest';
import { TrialPlaceRecord } from '../types';
import { calculateTrialFlipStats } from './trialFlipStats';

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
});
