import { describe, expect, it } from 'vitest';
import {
  computeScrapsValue,
  getRecordScrapsExpense,
  getRecordScrapsValue,
  normalizeScrapsItem,
  normalizeScrapsItems,
  RawScrapsItem,
  summarizeScrapsByName,
} from './scrapsUtils';
import { RaidRecord, ScrapsItem } from '../types';

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

describe('getRecordScrapsExpense', () => {
  it('非散件记录恒为 0', () => {
    const record = { isScrapsBoss: false, goldExpense: 8000, scrapsItems: [] };
    expect(getRecordScrapsExpense(record)).toBe(0);
  });

  it('新版记录：按白名单物品实际购买价合计（而非副本总支出）', () => {
    const record = {
      isScrapsBoss: true,
      goldExpense: 8000, // 副本总支出含装备购买，不应计入
      scrapsItems: [
        { name: '上品茶饼·兑', count: 6, unitPrice: 296, totalPrice: 1776, category: 'material', priceSource: 'jx3box' },
        { name: '叁级五彩石', count: 6, unitPrice: 510, totalPrice: 3060, category: 'material', priceSource: 'jx3box' },
        { name: '尽幽冠', count: 1, unitPrice: 112, totalPrice: 0, category: 'equipment', priceSource: 'npc' },
      ] as ScrapsItem[],
    };
    // 1776 + 3060 + 0 = 4836
    expect(getRecordScrapsExpense(record)).toBe(4836);
  });

  it('旧版记录（无 totalPrice）：回退为副本总支出', () => {
    const record = {
      isScrapsBoss: true,
      goldExpense: 8000,
      scrapsItems: [
        { name: '上品茶饼·兑', count: 6, unitPrice: 296, category: 'material', priceSource: 'jx3box' },
      ] as ScrapsItem[],
    };
    expect(getRecordScrapsExpense(record)).toBe(8000);
  });

  it('无清单的散件记录：回退为副本总支出', () => {
    expect(getRecordScrapsExpense({ isScrapsBoss: true, goldExpense: 3000 })).toBe(3000);
    expect(getRecordScrapsExpense({ isScrapsBoss: true })).toBe(0);
  });
});

describe('summarizeScrapsByName', () => {
  function makeRecord(
    id: string,
    isScrapsBoss: boolean,
    scrapsItems: ScrapsItem[],
  ): RaidRecord {
    return {
      id,
      accountId: 'a1',
      roleId: 'r1',
      raidName: '测试副本',
      date: Date.now(),
      goldIncome: 0,
      hasXuanjing: false,
      isScrapsBoss,
      scrapsItems,
    };
  }

  it('空记录返回空数组', () => {
    expect(summarizeScrapsByName([])).toEqual([]);
  });

  it('isScrapsBoss=false 的记录被排除', () => {
    const records = [
      makeRecord('rec1', false, [
        { name: '维峰丹', count: 5, unitPrice: 100, category: 'material', priceSource: 'jx3box' },
      ]),
    ];
    expect(summarizeScrapsByName(records)).toEqual([]);
  });

  it('同物品多记录聚合 count + raidCount', () => {
    const records = [
      makeRecord('rec1', true, [
        { name: '维峰丹', count: 2, unitPrice: 100, category: 'material', priceSource: 'jx3box' },
      ]),
      makeRecord('rec2', true, [
        { name: '维峰丹', count: 3, unitPrice: 100, category: 'material', priceSource: 'jx3box' },
      ]),
      makeRecord('rec3', true, [
        { name: '维峰丹', count: 5, unitPrice: 100, category: 'material', priceSource: 'jx3box' },
      ]),
    ];
    const rows = summarizeScrapsByName(records);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      name: '维峰丹',
      category: 'material',
      totalCount: 10,
      raidCount: 3,
      avgPerRaid: 10 / 3,
    });
    // 不再包含 type 字段
    expect((rows[0] as Record<string, unknown>).type).toBeUndefined();
  });

  it('同一记录内多个相同物品按一条统计（raidCount 不重复计）', () => {
    const records = [
      makeRecord('rec1', true, [
        { name: '玛瑙', count: 2, unitPrice: 5, category: 'material', priceSource: 'jx3box' },
        { name: '玛瑙', count: 3, unitPrice: 5, category: 'material', priceSource: 'jx3box' },
      ]),
    ];
    const rows = summarizeScrapsByName(records);
    expect(rows).toHaveLength(1);
    expect(rows[0].totalCount).toBe(5);
    expect(rows[0].raidCount).toBe(1);
  });

  it('按 totalCount 降序排序，相同数量按 name 升序', () => {
    const records = [
      makeRecord('rec1', true, [
        { name: '维峰丹', count: 1, unitPrice: 100, category: 'material', priceSource: 'jx3box' },
        { name: '玛瑙', count: 10, unitPrice: 5, category: 'material', priceSource: 'jx3box' },
        { name: '上品茶饼·兑', count: 5, unitPrice: 300, category: 'material', priceSource: 'jx3box' },
        { name: '尽幽冠', count: 1, unitPrice: 112, category: 'equipment', priceSource: 'npc' },
      ]),
    ];
    const rows = summarizeScrapsByName(records);
    expect(rows.map(r => r.name)).toEqual(['玛瑙', '上品茶饼·兑', '尽幽冠', '维峰丹']);
    expect(rows.map(r => r.totalCount)).toEqual([10, 5, 1, 1]);
  });

  it('兼容旧版 snake_case 数据', () => {
    const records = [
      makeRecord('rec1', true, [
        { name: '维峰丹', count: 3, unit_price: 100, category: 'material', price_source: 'jx3box' } as RawScrapsItem as ScrapsItem,
      ]),
    ];
    const rows = summarizeScrapsByName(records);
    expect(rows[0].totalCount).toBe(3);
    expect(rows[0].category).toBe('material');
  });
});
