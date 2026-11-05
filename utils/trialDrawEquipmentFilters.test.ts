import { describe, expect, it } from 'vitest';
import { JX3Equip } from '../services/jx3BoxApi';
import {
  filterTrialDrawEquipments,
  getTrialDrawEquipmentAttributes,
  getTrialDrawEquipmentSlot,
} from './trialDrawEquipmentFilters';

const equipment = (overrides: Partial<JX3Equip>): JX3Equip => ({
  ID: 1,
  Name: '无观项链·内·荒',
  UiID: '1',
  IconID: null,
  Level: 900,
  Quality: '4',
  SubType: 4,
  BindType: 3,
  ...overrides,
});

const allFilters = (overrides = {}) => ({
  query: '', minLevel: 900, maxLevel: 1280, slot: '全部', bindType: null,
  attributes: new Set<string>(), ...overrides,
});

describe('逐次翻牌装备筛选', () => {
  const innerCritical = equipment({
    ID: 51443,
    Level: 1280,
    _Magic1Type: { attr: ['atMagicAttackPowerBase'] },
    _Magic2Type: { attr: ['atAllTypeCriticalStrike'] },
    _Magic3Type: { attr: ['atStrainBase'] },
  });
  const outerOvercome = equipment({
    ID: 51486,
    Name: '无观项链·外·荒',
    Level: 1050,
    BindType: 2,
    _Magic1Type: { attr: ['atPhysicsAttackPowerBase'] },
    _Magic2Type: { attr: ['atMagicOvercomeBase'] },
  });
  const hiddenWeapon = equipment({
    ID: 47942,
    Name: '无观囊·内·荒',
    Level: 1110,
    SubType: 1,
    DetailType: 6,
    TypeLabel: undefined,
  });

  it('从本地目录缺失 TypeLabel 的条目推导可筛选部位', () => {
    expect(getTrialDrawEquipmentSlot(innerCritical)).toBe('项链');
    expect(getTrialDrawEquipmentSlot(hiddenWeapon)).toBe('暗器');
  });

  it('从魔法字段提取属性，支持属性组合筛选', () => {
    expect(getTrialDrawEquipmentAttributes(innerCritical)).toEqual(expect.arrayContaining(['内功', '会心', '无双']));
    expect(filterTrialDrawEquipments([innerCritical, outerOvercome, hiddenWeapon], allFilters({
      attributes: new Set(['内功', '会心']),
    }))).toEqual([innerCritical]);
  });

  it('合并 940 品加速装备的魔法字段与目录绿字，不丢失外功或加速', () => {
    const hasteEquipment = (id: number) => equipment({
      ID: id,
      Name: '无观项链·外·荒',
      Level: 940,
      _Magic1Type: { attr: ['atPhysicsAttackPowerBase', '353', '353', '0', '0'] },
      _Magic2Type: { attr: ['atHasteBase', '320', '320', '0', '0'] },
      _Attrs: ['Haste'],
      attributes: [
        { type: 'atHasteBase', label: '加速', color: 'green' },
        { type: 'atVitalityBase', label: '体质', color: 'white' },
        { type: 'atUnmappedAttribute', label: '未知属性', color: 'green' },
      ],
    });
    const realCatalogEntries = [hasteEquipment(51494), hasteEquipment(51508)];

    realCatalogEntries.forEach(item => {
      expect(getTrialDrawEquipmentAttributes(item)).toEqual(expect.arrayContaining(['外功', '加速']));
      expect(getTrialDrawEquipmentAttributes(item)).not.toContain('未知属性');
    });
    expect(filterTrialDrawEquipments(realCatalogEntries, allFilters({
      minLevel: 940,
      attributes: new Set(['外功', '加速']),
    }))).toEqual(realCatalogEntries);
  });

  it('在已有魔法属性时仍合并 _Attrs 中有证据的绿字', () => {
    const catalogAttributeOnly = equipment({
      ID: 51509,
      _Magic1Type: { attr: ['atPhysicsAttackPowerBase'] },
      _Attrs: ['Haste', 'UnknownAttribute'],
    });
    expect(getTrialDrawEquipmentAttributes(catalogAttributeOnly)).toEqual(expect.arrayContaining(['外功', '加速']));
    expect(getTrialDrawEquipmentAttributes(catalogAttributeOnly)).not.toContain('UnknownAttribute');
  });

  it('联合按品级、部位、绑定与搜索文本过滤自动记录可选装备', () => {
    expect(filterTrialDrawEquipments([innerCritical, outerOvercome, hiddenWeapon], allFilters({
      query: '暗器 1110', slot: '暗器', minLevel: 1000, maxLevel: 1200, bindType: 3,
    }))).toEqual([hiddenWeapon]);

    expect(filterTrialDrawEquipments([innerCritical, outerOvercome, hiddenWeapon], allFilters({
      query: '装备后绑定 破防', slot: '项链', minLevel: 1000, maxLevel: 1100, bindType: 2,
    }))).toEqual([outerOvercome]);
  });
});
