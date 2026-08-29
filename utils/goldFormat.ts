/**
 * 金币显示格式化：10000 金 = 1 砖。
 *
 * 金额达到 1 砖时显示为「xx砖xx金」，不足 1 砖时保持「x,xxx金」原样。
 * 仅用于展示，数据层仍统一以「金」为单位存储。
 */

/** 1 砖对应的金币数量 */
export const GOLD_PER_ZHUAN = 10000;

interface GoldParts {
  /** 负数前缀 */
  sign: string;
  /** 砖数量 */
  zhuan: number;
  /** 不足 1 砖的剩余金 */
  gold: number;
}

const splitGold = (amount: number): GoldParts => {
  const safe = Number.isFinite(amount) ? Math.round(amount) : 0;
  const abs = Math.abs(safe);

  return {
    sign: safe < 0 ? '-' : '',
    zhuan: Math.floor(abs / GOLD_PER_ZHUAN),
    gold: abs % GOLD_PER_ZHUAN,
  };
};

/**
 * 完整金额文本（含单位）。
 * - 9999 -> "9,999金"
 * - 10000 -> "1砖"
 * - 12345 -> "1砖2345金"
 * - -12345 -> "-1砖2345金"
 */
export const formatGold = (amount: number): string => {
  const { sign, zhuan, gold } = splitGold(amount);

  if (zhuan === 0) {
    return `${sign}${gold.toLocaleString()}金`;
  }
  if (gold === 0) {
    return `${sign}${zhuan.toLocaleString()}砖`;
  }
  return `${sign}${zhuan.toLocaleString()}砖${gold}金`;
};

/**
 * 仅数值部分（不含单位），只用于「金」已由界面单独渲染的场景（如主数字旁的角标单位）。
 * 不折算小数，按「xx砖xx」原样输出；需要带单位的场合一律用 formatGold。
 * - 9999 -> "9,999"
 * - 10000 -> "1砖"
 * - 12345 -> "1砖2345"
 * - 99999 -> "9砖9999"
 */
export const formatGoldNumber = (amount: number): string => {
  const { sign, zhuan, gold } = splitGold(amount);

  if (zhuan === 0) {
    return `${sign}${gold.toLocaleString()}`;
  }
  if (gold === 0) {
    return `${sign}${zhuan.toLocaleString()}砖`;
  }
  return `${sign}${zhuan.toLocaleString()}砖${gold}`;
};
