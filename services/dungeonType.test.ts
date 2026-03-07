/**
 * dungeonType 服务单元测试
 */

import { describe, it, expect } from 'vitest';
import { identifyDungeonType, extractBaizhanLevel, parseRaidInfo, isValidDungeonName } from '../services/dungeonType';

describe('dungeonType', () => {
  // ============================================================================
  // identifyDungeonType
  // ============================================================================

  describe('identifyDungeonType', () => {
    describe('团队副本', () => {
      it('应识别标准团队副本名称', () => {
        expect(identifyDungeonType('英雄冷龙峰')).toBe('raid');
        expect(identifyDungeonType('普通冷龙峰')).toBe('raid');
        expect(identifyDungeonType('挑战冷龙峰')).toBe('raid');
      });

      it('应识别25人副本', () => {
        expect(identifyDungeonType('25人英雄冷龙峰')).toBe('raid');
        expect(identifyDungeonType('25人普通冷龙峰')).toBe('raid');
      });

      it('应识别10人副本', () => {
        expect(identifyDungeonType('10人英雄冷龙峰')).toBe('raid');
        expect(identifyDungeonType('10人普通冷龙峰')).toBe('raid');
      });

      it('应识别已知副本名称', () => {
        expect(identifyDungeonType('白帝水宫')).toBe('raid');
        expect(identifyDungeonType('烛龙殿')).toBe('raid');
        expect(identifyDungeonType('风雪稻香村')).toBe('raid');
      });
    });

    describe('百战', () => {
      it('应识别"百战"关键字', () => {
        expect(identifyDungeonType('百战')).toBe('baizhan');
        expect(identifyDungeonType('百战·一阶')).toBe('baizhan');
        expect(identifyDungeonType('百战二阶')).toBe('baizhan');
      });

      it('应识别百战带数字等级', () => {
        expect(identifyDungeonType('百战1')).toBe('baizhan');
        expect(identifyDungeonType('百战2')).toBe('baizhan');
        expect(identifyDungeonType('百战3')).toBe('baizhan');
        expect(identifyDungeonType('百战12')).toBe('baizhan');
      });

      it('应识别百战带层数', () => {
        expect(identifyDungeonType('百战·30层')).toBe('baizhan');
        expect(identifyDungeonType('百战30层')).toBe('baizhan');
      });
    });

    describe('默认行为', () => {
      it('未知副本默认返回 raid', () => {
        expect(identifyDungeonType('未知副本')).toBe('raid');
        expect(identifyDungeonType('新副本')).toBe('raid');
      });

      it('空字符串应返回 raid', () => {
        expect(identifyDungeonType('')).toBe('raid');
      });
    });
  });

  // ============================================================================
  // extractBaizhanLevel
  // ============================================================================

  describe('extractBaizhanLevel', () => {
    it('应提取百战层数', () => {
      expect(extractBaizhanLevel('百战·30层')).toBe(30);
      expect(extractBaizhanLevel('百战30层')).toBe(30);
      expect(extractBaizhanLevel('百战·12层')).toBe(12);
    });

    it('无层数应返回默认值 1', () => {
      expect(extractBaizhanLevel('百战')).toBe(1);
      expect(extractBaizhanLevel('百战·')).toBe(1);
    });

    it('非百战副本应返回默认值 1', () => {
      // 实际实现总是返回数字，默认为 1
      expect(extractBaizhanLevel('英雄冷龙峰')).toBe(1);
      expect(extractBaizhanLevel('普通副本')).toBe(1);
    });

    it('应正确解析带间隔号的层数', () => {
      expect(extractBaizhanLevel('百战·5层')).toBe(5);
      expect(extractBaizhanLevel('百战•10层')).toBe(10);
    });
  });

  // ============================================================================
  // parseRaidInfo
  // ============================================================================

  describe('parseRaidInfo', () => {
    it('应解析带人数的副本名称', () => {
      const result = parseRaidInfo('25人英雄冷龙峰');
      expect(result.playerCount).toBe(25);
      expect(result.difficulty).toBe('英雄');
      expect(result.name).toBe('冷龙峰');
    });

    it('应解析10人副本', () => {
      const result = parseRaidInfo('10人普通冷龙峰');
      expect(result.playerCount).toBe(10);
      expect(result.difficulty).toBe('普通');
      expect(result.name).toBe('冷龙峰');
    });

    it('应解析挑战模式', () => {
      const result = parseRaidInfo('25人挑战冷龙峰');
      expect(result.playerCount).toBe(25);
      expect(result.difficulty).toBe('挑战');
      expect(result.name).toBe('冷龙峰');
    });

    it('无人數前缀应返回默认值', () => {
      const result = parseRaidInfo('英雄冷龙峰');
      // 默认返回 25 人
      expect(result.playerCount).toBe(25);
      expect(result.difficulty).toBe('普通');
      expect(result.name).toBe('英雄冷龙峰');
    });

    it('空字符串应返回默认值', () => {
      const result = parseRaidInfo('');
      expect(result.playerCount).toBe(25);
      expect(result.difficulty).toBe('普通');
      expect(result.name).toBe('');
    });

    it('只有人数无难度应使用默认难度', () => {
      const result = parseRaidInfo('10人冷龙峰');
      expect(result.playerCount).toBe(10);
      expect(result.difficulty).toBe('普通');
      expect(result.name).toBe('冷龙峰');
    });
  });

  // ============================================================================
  // isValidDungeonName
  // ============================================================================

  describe('isValidDungeonName', () => {
    it('应识别有效的团队副本名称', () => {
      expect(isValidDungeonName('25人英雄冷龙峰')).toBe(true);
      expect(isValidDungeonName('10人普通白帝水宫')).toBe(true);
      expect(isValidDungeonName('25人挑战烛龙殿')).toBe(true);
    });

    it('应识别有效的百战名称', () => {
      expect(isValidDungeonName('百战')).toBe(true);
      expect(isValidDungeonName('百战·30层')).toBe(true);
    });

    it('应拒绝无效名称', () => {
      expect(isValidDungeonName('')).toBe(false);
      expect(isValidDungeonName('   ')).toBe(false);
      expect(isValidDungeonName('普通的副本')).toBe(false);
    });
  });
});
