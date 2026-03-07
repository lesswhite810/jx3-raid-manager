/**
 * dropMapping 服务单元测试
 */

import { describe, it, expect } from 'vitest';
import {
  identifySpecialDrop,
  generateDropFlags,
  getDropTypeTags,
  DEFAULT_DROP_FLAGS,
  SPECIAL_DROP_MAPPINGS,
} from '../services/dropMapping';
import { SpecialItem } from '../types';

describe('dropMapping', () => {
  // ============================================================================
  // identifySpecialDrop
  // ============================================================================

  describe('identifySpecialDrop', () => {
    describe('玄晶', () => {
      it('应识别"玄晶"', () => {
        expect(identifySpecialDrop('玄晶')).toBe('xuanjing');
      });

      it('应识别"化玉玄晶"', () => {
        expect(identifySpecialDrop('化玉玄晶')).toBe('xuanjing');
      });

      it('应识别包含"玄晶"的物品名', () => {
        expect(identifySpecialDrop('[英雄]玄晶')).toBe('xuanjing');
      });
    });

    describe('马具', () => {
      it('应识别"马鞍"', () => {
        expect(identifySpecialDrop('马鞍')).toBe('maju');
      });

      it('应识别"马鞭"', () => {
        expect(identifySpecialDrop('马鞭')).toBe('maju');
      });

      it('应识别"缰绳"', () => {
        expect(identifySpecialDrop('缰绳')).toBe('maju');
      });
    });

    describe('宠物', () => {
      it('应识别"宠物"', () => {
        expect(identifySpecialDrop('宠物·小灰灰')).toBe('pet');
      });

      it('应识别"跟宠"', () => {
        expect(identifySpecialDrop('跟宠·大白')).toBe('pet');
      });
    });

    describe('挂件', () => {
      it('应识别"挂件"', () => {
        expect(identifySpecialDrop('背部挂件')).toBe('pendant');
      });

      it('应识别"腰挂"', () => {
        expect(identifySpecialDrop('腰挂·玉佩')).toBe('pendant');
      });

      it('应识别"背挂"', () => {
        expect(identifySpecialDrop('背挂·剑匣')).toBe('pendant');
      });
    });

    describe('坐骑', () => {
      it('应识别"坐骑"', () => {
        expect(identifySpecialDrop('坐骑·赤兔')).toBe('mount');
      });

      it('应识别"马匹"', () => {
        expect(identifySpecialDrop('马匹·汗血')).toBe('mount');
      });
    });

    describe('外观', () => {
      it('应识别"外观"', () => {
        expect(identifySpecialDrop('外观·霓裳')).toBe('appearance');
      });

      it('应识别"成衣"', () => {
        expect(identifySpecialDrop('成衣·云锦')).toBe('appearance');
      });

      it('应识别"盒子"', () => {
        expect(identifySpecialDrop('盒子·新春')).toBe('appearance');
      });
    });

    describe('称号', () => {
      it('应识别"称号"', () => {
        expect(identifySpecialDrop('称号·天下第一')).toBe('title');
      });
    });

    describe('秘籍', () => {
      it('应识别"秘籍"', () => {
        expect(identifySpecialDrop('秘籍·太极拳')).toBe('secretBook');
      });

      it('应识别"残页"', () => {
        expect(identifySpecialDrop('残页·太极剑')).toBe('secretBook');
      });

      it('应识别"断篇"', () => {
        expect(identifySpecialDrop('断篇·纯阳诀')).toBe('secretBook');
      });
    });

    describe('未识别物品', () => {
      it('普通物品应返回 null', () => {
        expect(identifySpecialDrop('铁犁')).toBeNull();
        expect(identifySpecialDrop('丝绸')).toBeNull();
        expect(identifySpecialDrop('普通的装备')).toBeNull();
      });

      it('空字符串应返回 null', () => {
        expect(identifySpecialDrop('')).toBeNull();
      });
    });
  });

  // ============================================================================
  // generateDropFlags
  // ============================================================================

  describe('generateDropFlags', () => {
    it('空数组应返回默认标志', () => {
      const result = generateDropFlags([]);
      expect(result).toEqual(DEFAULT_DROP_FLAGS);
    });

    it('应正确设置单个标志', () => {
      const items: SpecialItem[] = [
        { name: '玄晶', buyer: '张三', price: 50000, isWorkerBought: false },
      ];
      const result = generateDropFlags(items);
      expect(result.hasXuanjing).toBe(true);
      expect(result.hasMaJu).toBe(false);
    });

    it('应正确设置多个标志', () => {
      const items: SpecialItem[] = [
        { name: '玄晶', buyer: '张三', price: 50000, isWorkerBought: false },
        { name: '马鞍', buyer: '李四', price: 10000, isWorkerBought: false },
        { name: '坐骑·赤兔', buyer: '王五', price: 20000, isWorkerBought: false },
      ];
      const result = generateDropFlags(items);
      expect(result.hasXuanjing).toBe(true);
      expect(result.hasMaJu).toBe(true);
      expect(result.hasMount).toBe(true);
      expect(result.hasPet).toBe(false);
    });

    it('同类型多个物品应只设置一个标志', () => {
      const items: SpecialItem[] = [
        { name: '玄晶', buyer: '张三', price: 50000, isWorkerBought: false },
        { name: '化玉玄晶', buyer: '李四', price: 30000, isWorkerBought: false },
      ];
      const result = generateDropFlags(items);
      expect(result.hasXuanjing).toBe(true);
    });
  });

  // ============================================================================
  // getDropTypeTags
  // ============================================================================

  describe('getDropTypeTags', () => {
    it('空数组应返回空数组', () => {
      expect(getDropTypeTags([])).toEqual([]);
    });

    it('应返回正确的标签列表', () => {
      const items: SpecialItem[] = [
        { name: '玄晶', buyer: '张三', price: 50000, isWorkerBought: false },
        { name: '马鞍', buyer: '李四', price: 10000, isWorkerBought: false },
      ];
      const tags = getDropTypeTags(items);

      expect(tags).toHaveLength(2);
      expect(tags.find(t => t.type === 'xuanjing')).toBeDefined();
      expect(tags.find(t => t.type === 'maju')).toBeDefined();
    });

    it('同类型应去重', () => {
      const items: SpecialItem[] = [
        { name: '玄晶', buyer: '张三', price: 50000, isWorkerBought: false },
        { name: '化玉玄晶', buyer: '李四', price: 30000, isWorkerBought: false },
      ];
      const tags = getDropTypeTags(items);

      expect(tags).toHaveLength(1);
      expect(tags[0].type).toBe('xuanjing');
      expect(tags[0].label).toBe('玄晶');
    });

    it('应返回正确的中文标签', () => {
      const items: SpecialItem[] = [
        { name: '玄晶', buyer: '', price: 0, isWorkerBought: false },
      ];
      const tags = getDropTypeTags(items);

      expect(tags[0].label).toBe(SPECIAL_DROP_MAPPINGS.xuanjing.label);
    });
  });

  // ============================================================================
  // DEFAULT_DROP_FLAGS
  // ============================================================================

  describe('DEFAULT_DROP_FLAGS', () => {
    it('所有标志应为 false', () => {
      expect(DEFAULT_DROP_FLAGS.hasXuanjing).toBe(false);
      expect(DEFAULT_DROP_FLAGS.hasMaJu).toBe(false);
      expect(DEFAULT_DROP_FLAGS.hasPet).toBe(false);
      expect(DEFAULT_DROP_FLAGS.hasPendant).toBe(false);
      expect(DEFAULT_DROP_FLAGS.hasMount).toBe(false);
      expect(DEFAULT_DROP_FLAGS.hasAppearance).toBe(false);
      expect(DEFAULT_DROP_FLAGS.hasTitle).toBe(false);
      expect(DEFAULT_DROP_FLAGS.hasSecretBook).toBe(false);
    });
  });
});
