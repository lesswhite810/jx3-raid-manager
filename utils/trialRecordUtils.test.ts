import { describe, expect, it } from 'vitest';
import { TrialPlaceRecord } from '../types';
import { getTrialRecordEquipmentEntries, getTrialRecordEquipmentIds } from './trialRecordUtils';

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

describe('getTrialRecordEquipmentIds', () => {
  it('returns recorded equipments even when flipped card is not selected', () => {
    const record = createTrialRecord({
      card2: '2001',
      card4: '2002',
      flippedIndex: 1
    });

    expect(getTrialRecordEquipmentIds(record)).toEqual(['2001', '2002']);
  });

  it('keeps the flipped equipment first when it exists', () => {
    const record = createTrialRecord({
      card1: '1001',
      card3: '1003',
      flippedIndex: 3
    });

    expect(getTrialRecordEquipmentIds(record)).toEqual(['1003', '1001']);
  });

  it('reads legacy snake_case card fields', () => {
    const record = {
      ...createTrialRecord(),
      card1: '',
      card2: '',
      card3: '',
      card4: '',
      card5: '',
      flippedIndex: 1,
      card_1: '3001',
      card_3: '3003'
    } as TrialPlaceRecord & {
      card_1?: string;
      card_3?: string;
    };

    expect(getTrialRecordEquipmentIds(record)).toEqual(['3001', '3003']);
  });

  it('preserves duplicate equipment ids from different card slots', () => {
    const record = createTrialRecord({
      card1: '4001',
      card3: '4001',
      flippedIndex: 1
    });

    expect(getTrialRecordEquipmentIds(record)).toEqual(['4001', '4001']);
  });

  it('returns card slot metadata and flipped state for duplicate equipments', () => {
    const record = createTrialRecord({
      card1: '5001',
      card3: '5001',
      flippedIndex: 1
    });

    expect(getTrialRecordEquipmentEntries(record)).toEqual([
      { cardIndex: 1, equipId: '5001', isFlipped: true },
      { cardIndex: 3, equipId: '5001', isFlipped: false }
    ]);
  });
});
