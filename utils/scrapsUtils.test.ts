import { describe, expect, it } from 'vitest';
import {
  computeScrapsValue,
  getRecordScrapsValue,
  normalizeScrapsItem,
  normalizeScrapsItems,
  RawScrapsItem,
} from './scrapsUtils';
import { ScrapsItem } from '../types';

describe('normalizeScrapsItem', () => {
  it('新版 camelCase 数据原样保留（不做单位换算）', () => {
    const raw: ScrapsItem = {
      name: '尽幽冠',
      count: 1,
      unitPrice: 112,
      category: 'equipment',
      priceSource: 'npc',
    };
    expect(normalizeScrapsItem(raw)).toEqual(raw);
  });

  it('旧版 snake_case 材料数据映射为 camelCase，价格保持金单位', () => {
    const raw: RawScrapsItem = {
      name: '上品茶饼·兑',
      count: 1,
      unit_price: 555,
      category: 'material',
      price_source: 'jx3box',
    };
    expect(normalizeScrapsItem(raw)).toEqual({
      name: '上品茶饼·兑',
      count: 1,
      unitPrice: 555,
      category: 'material',
      priceSource: 'jx3box',
    });
  });

  it('旧版 snake_case 装备 npc 单价按 铜→金 换算', () => {
    // 尽幽冠：drop_items.Price = 1,118,787 铜 ≈ 112 金（旧版曾直接当金显示）
    const raw: RawScrapsItem = {
      name: '尽幽冠',
      count: 1,
      unit_price: 1118787,
      category: 'equipment',
      price_source: 'npc',
    };
    const normalized = normalizeScrapsItem(raw);
    expect(normalized.unitPrice).toBe(112);
    expect(normalized.priceSource).toBe('npc');
    expect(normalized.category).toBe('equipment');
  });

  it('混合键数据优先使用 camelCase 手填值，不二次换算', () => {
    // 用户在旧版界面手填过单价：camelCase 与 snake_case 并存
    const raw = {
      name: '丘墟囊',
      count: 1,
      unit_price: 839090,
      category: 'equipment',
      price_source: 'npc',
      unitPrice: 50,
    };
    const normalized = normalizeScrapsItem(raw);
    expect(normalized.unitPrice).toBe(50);
  });

  it('非法或缺失字段回退到安全默认值', () => {
    const normalized = normalizeScrapsItem({});
    expect(normalized.name).toBe('未命名物品');
    expect(normalized.count).toBe(1);
    expect(normalized.unitPrice).toBeNull();
    expect(normalized.category).toBe('material');
    expect(normalized.priceSource).toBe('manual');
  });
});

describe('normalizeScrapsItems', () => {
  it('空清单与 undefined 均返回空数组', () => {
    expect(normalizeScrapsItems(undefined)).toEqual([]);
    expect(normalizeScrapsItems([])).toEqual([]);
  });

  it('批量归一化保留条目顺序', () => {
    const items = normalizeScrapsItems([
      { name: '玛瑙', count: 1, unit_price: 3, category: 'material', price_source: 'jx3box' },
      { name: '昭文裤', count: 1, unit_price: 1258635, category: 'equipment', price_source: 'npc' },
    ]);
    expect(items.map(i => i.name)).toEqual(['玛瑙', '昭文裤']);
    expect(items[0].unitPrice).toBe(3);
    expect(items[1].unitPrice).toBe(126); // 1258635 铜 → 126 金
  });
});

describe('computeScrapsValue', () => {
  it('未填单价按 0 计入', () => {
    const items: ScrapsItem[] = [
      { name: 'A', count: 2, unitPrice: 100, category: 'material', priceSource: 'manual' },
      { name: 'B', count: 3, unitPrice: null, category: 'material', priceSource: 'manual' },
    ];
    expect(computeScrapsValue(items)).toBe(200);
  });
});

describe('getRecordScrapsValue', () => {
  it('清单非空时按归一化单价重算，修正旧版虚高估价', () => {
    const legacyItems: RawScrapsItem[] = [
      { name: '尽幽冠', count: 1, unit_price: 1118787, category: 'equipment', price_source: 'npc' },
      { name: '上品茶饼·兑', count: 1, unit_price: 555, category: 'material', price_source: 'jx3box' },
    ];
    const record = {
      // 旧版快照值（铜当金）：16811511
      scrapsValue: 16811511,
      scrapsItems: legacyItems,
    };
    // 1118787 铜 ≈ 112 金 + 材料 555 金
    expect(getRecordScrapsValue(record)).toBe(667);
  });

  it('无清单时回退到扫描快照 scrapsValue', () => {
    expect(getRecordScrapsValue({ scrapsValue: 500 })).toBe(500);
    expect(getRecordScrapsValue({})).toBe(0);
  });
});
