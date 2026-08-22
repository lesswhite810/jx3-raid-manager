import { ScrapsItem } from '../types';

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
