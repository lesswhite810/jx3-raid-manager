import { describe, expect, it } from 'vitest';
import { getRecordBossNames } from './recordBossDisplay';

describe('getRecordBossNames', () => {
  it('returns multi-select BOSS names in record order', () => {
    expect(getRecordBossNames({
      bossIds: ['luoyangzhizhan_1', 'luoyangzhizhan_5'],
      bossNames: ['突利和顺', '史朝义'],
    })).toEqual(['突利和顺', '史朝义']);
  });

  it('falls back to the legacy single-BOSS fields', () => {
    expect(getRecordBossNames({
      bossId: 'luoyangzhizhan_5',
      bossName: '史朝义',
    })).toEqual(['史朝义']);
  });

  it('returns an empty list when the raid has no BOSS tracking', () => {
    expect(getRecordBossNames({})).toEqual([]);
    expect(getRecordBossNames({ bossIds: [], bossNames: [] })).toEqual([]);
  });

  it('drops blank names so name-less historical records render nothing', () => {
    expect(getRecordBossNames({
      bossIds: ['custom_1', 'luoyangzhizhan_5'],
      bossNames: ['', ' 史朝义 '],
    })).toEqual(['史朝义']);
  });
});
