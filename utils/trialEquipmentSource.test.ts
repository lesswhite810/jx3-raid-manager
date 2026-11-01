import { describe, expect, it } from 'vitest';
import {
  getTrialEquipmentSource,
  isTrialEquipmentFromSource,
} from './trialEquipmentSource';

describe('试炼装备来源', () => {
  it('苍生铸世使用无观项链和暗器白名单', () => {
    const source = getTrialEquipmentSource({ name: '苍生铸世' });

    expect(source.keyword).toBe('无观');
    expect(isTrialEquipmentFromSource(
      { Name: '无观项链·内·荒', SubType: 4 },
      'trinket',
      source
    )).toBe(true);
    expect(isTrialEquipmentFromSource(
      { Name: '无观囊·外·荒', SubType: 1, DetailType: 6 },
      'weapon',
      source
    )).toBe(true);
    expect(isTrialEquipmentFromSource(
      { Name: '无观冠·内·荒', SubType: 3 },
      'armor',
      source
    )).toBe(false);
    expect(isTrialEquipmentFromSource(
      { Name: '无观剑·内·荒', SubType: 1, DetailType: 0 },
      'weapon',
      source
    )).toBe(false);
  });

  it('未知或原型链上的赛季名保持无修全类别来源', () => {
    const source = getTrialEquipmentSource({ name: '后续赛季' });

    expect(source.keyword).toBe('无修');
    expect(isTrialEquipmentFromSource(
      { Name: '无修冠', SubType: 3 },
      'armor',
      source
    )).toBe(true);
    expect(getTrialEquipmentSource({ name: 'constructor' }).keyword).toBe('无修');
    expect(getTrialEquipmentSource({ name: '__proto__' }).keyword).toBe('无修');
  });
});
