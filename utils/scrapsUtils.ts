import { RaidRecord, ScrapsItem } from '../types';

/** 散件材料白名单（与后端 SCRAPS_MATERIAL_WHITELIST 保持一致） */
export const SCRAPS_MATERIAL_WHITELIST = [
  '上品茶饼·兑', '维峰丹', '玛瑙', '猫眼石',
  '叁级五彩石', '五行石（六级）', '肆级五彩石',
];

/** 根据当前散件清单计算估价合计（unitPrice=null 按 0 计入） */
export function computeScrapsValue(items: ScrapsItem[]): number {
  return items.reduce((sum, item) => {
    if (item.unitPrice == null) return sum;
    return sum + item.count * item.unitPrice;
  }, 0);
}

/**
 * v2.1.53 后端写入的旧格式散件项（snake_case 键）
 *
 * 旧版问题：
 * 1. 键名为 unit_price / price_source，前端读取 camelCase 得到 undefined，
 *    表现为编辑弹窗单价全空、估价合计恒为 0
 * 2. 装备 NPC 卖价误把 drop_items.Price（单位：铜）当作金写入
 */
export interface LegacyScrapsItem {
  name?: unknown;
  count?: unknown;
  unit_price?: unknown;
  category?: unknown;
  price_source?: unknown;
}

/** 散件项原始数据（新版 camelCase / 旧版 snake_case 均可） */
export type RawScrapsItem = ScrapsItem | LegacyScrapsItem;

const COPPER_PER_GOLD = 10000;

/** 铜 → 金四舍五入（与后端换算口径一致：(copper + 5000) / 10000） */
function copperToGold(copper: number): number {
  return Math.floor((copper + COPPER_PER_GOLD / 2) / COPPER_PER_GOLD);
}

function readFiniteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function readPriceSource(value: unknown): ScrapsItem['priceSource'] | null {
  return value === 'jx3box' || value === 'npc' || value === 'manual' ? value : null;
}

/**
 * 归一化单个散件项为前端标准格式（camelCase 键 + 金单位）
 *
 * 兼容规则：
 * - camelCase 字段优先（用户在旧版界面手填的值保存在 camelCase 上，不做单位换算）
 * - 仅当数据为旧版 snake_case 且来源为 npc 时，按 铜→金 换算单价
 *   （材料价格来自 JX3Box API，后端已换算为金，无需二次处理）
 * - category 缺失或非法时按 material 处理（散件清单以白名单材料为主）
 */
export function normalizeScrapsItem(raw: RawScrapsItem): ScrapsItem {
  // 统一按运行时类型读取，避免对联合类型做断言
  const source: Record<string, unknown> = { ...raw };

  const name = typeof source.name === 'string' && source.name.trim() !== ''
    ? source.name
    : '未命名物品';
  const rawCount = readFiniteNumber(source.count);
  const count = rawCount != null && rawCount >= 0 ? Math.floor(rawCount) : 1;

  const newPriceSource = readPriceSource(source.priceSource);
  const legacyPriceSource = readPriceSource(source.price_source);
  const priceSource: ScrapsItem['priceSource'] = newPriceSource ?? legacyPriceSource ?? 'manual';

  let unitPrice: number | null;
  if (newPriceSource != null || readFiniteNumber(source.unitPrice) != null) {
    // 新版 camelCase 数据：单价已是金，直接使用
    unitPrice = readFiniteNumber(source.unitPrice);
  } else {
    const legacyPrice = readFiniteNumber(source.unit_price);
    if (legacyPrice == null) {
      unitPrice = null;
    } else if (priceSource === 'npc') {
      // 旧版 bug：装备 NPC 卖价以铜为单位写入，修正为金
      unitPrice = copperToGold(legacyPrice);
    } else {
      // 材料 JX3Box 价格后端已换算为金；未知来源保持原值
      unitPrice = legacyPrice;
    }
  }

  const category: ScrapsItem['category'] =
    source.category === 'equipment' ? 'equipment' : 'material';

  return { name, count, unitPrice, category, priceSource };
}

/** 批量归一化散件清单 */
export function normalizeScrapsItems(items?: readonly RawScrapsItem[]): ScrapsItem[] {
  if (!items || items.length === 0) return [];
  return items.map(normalizeScrapsItem);
}

/**
 * 记录的散件估价展示值
 *
 * 清单非空时按归一化后的单价重算（自动修正旧版数据的键名与单位问题）；
 * 无清单时回退到扫描时快照的 scrapsValue。
 */
export function getRecordScrapsValue(
  record: Pick<RaidRecord, 'scrapsItems' | 'scrapsValue'>,
): number {
  if (record.scrapsItems && record.scrapsItems.length > 0) {
    return computeScrapsValue(normalizeScrapsItems(record.scrapsItems));
  }
  return Number(record.scrapsValue) || 0;
}
