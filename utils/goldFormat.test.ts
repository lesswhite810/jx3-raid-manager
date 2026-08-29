import { describe, expect, it } from 'vitest';
import { formatGold, formatGoldNumber } from './goldFormat';

describe('formatGold', () => {
  it('不足 1 砖时保持「x,xxx金」', () => {
    expect(formatGold(0)).toBe('0金');
    expect(formatGold(999)).toBe('999金');
    expect(formatGold(9999)).toBe('9,999金');
  });

  it('整砖时不显示 0 金', () => {
    expect(formatGold(10000)).toBe('1砖');
    expect(formatGold(30000)).toBe('3砖');
  });

  it('超过 1 砖时显示为「xx砖xx金」', () => {
    expect(formatGold(10001)).toBe('1砖1金');
    expect(formatGold(12345)).toBe('1砖2345金');
    expect(formatGold(123456)).toBe('12砖3456金');
  });

  it('负数保留符号', () => {
    expect(formatGold(-9999)).toBe('-9,999金');
    expect(formatGold(-12345)).toBe('-1砖2345金');
  });

  it('非法数值按 0 处理', () => {
    expect(formatGold(Number.NaN)).toBe('0金');
  });

  it('小数四舍五入后换算', () => {
    expect(formatGold(12345.6)).toBe('1砖2346金');
  });
});

describe('formatGoldNumber', () => {
  it('只输出数值部分，不含单位', () => {
    expect(formatGoldNumber(9999)).toBe('9,999');
    expect(formatGoldNumber(10000)).toBe('1砖');
    expect(formatGoldNumber(12345)).toBe('1砖2345');
    expect(formatGoldNumber(-12345)).toBe('-1砖2345');
  });

  it('不折算小数，余数按原样显示', () => {
    expect(formatGoldNumber(10001)).toBe('1砖1');
    expect(formatGoldNumber(25000)).toBe('2砖5000');
    expect(formatGoldNumber(99999)).toBe('9砖9999');
    expect(formatGoldNumber(19999)).toBe('1砖9999');
  });
});
