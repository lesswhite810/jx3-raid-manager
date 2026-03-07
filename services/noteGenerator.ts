/**
 * 备注生成服务
 */

import { AnalysisResult, SpecialItem } from '../types';

/**
 * 生成分析结果的备注内容
 * @param result 分析结果
 * @returns 备注字符串
 */
export function generateNotes(result: AnalysisResult): string {
  const parts: string[] = [];

  // 躺拍标记
  if (result.isLyingFlat) {
    parts.push('躺拍');
  }

  // 特殊掉落详情
  if (result.specialItems.length > 0) {
    const itemDetails = result.specialItems.map(item => {
      const priceStr = item.price.toLocaleString() + '金';
      if (item.isWorkerBought) {
        return `${item.name}(自购 ${priceStr})`;
      }
      return `${item.name}(${priceStr})`;
    });
    parts.push(`特殊掉落: ${itemDetails.join(', ')}`);
  }

  // 消费明细（如果有非特殊物品消费）
  if (result.scatteredConsumption > 0 || result.ironConsumption > 0 || result.otherConsumption > 0) {
    const consumptionParts: string[] = [];
    if (result.scatteredConsumption > 0) {
      consumptionParts.push(`敷件 ${result.scatteredConsumption.toLocaleString()}金`);
    }
    if (result.ironConsumption > 0) {
      consumptionParts.push(`小铁 ${result.ironConsumption.toLocaleString()}金`);
    }
    if (result.otherConsumption > 0) {
      consumptionParts.push(`其他 ${result.otherConsumption.toLocaleString()}金`);
    }
    if (consumptionParts.length > 0) {
      parts.push(`消费: ${consumptionParts.join(', ')}`);
    }
  }

  // 罚款
  if (result.fine > 0) {
    parts.push(`罚款: ${result.fine.toLocaleString()}金`);
  }

  // 原始备注（躲拍、难度等）
  if (result.notes && result.notes.trim()) {
    parts.push(result.notes.trim());
  }

  return parts.join(' | ');
}

/**
 * 生成特殊物品的简短描述
 */
export function generateSpecialItemsBrief(items: SpecialItem[], maxLength: number = 50): string {
  if (items.length === 0) return '';

  const names = items.map(item => item.name);
  let brief = names.join(', ');

  if (brief.length > maxLength) {
    brief = brief.substring(0, maxLength - 3) + '...';
  }

  return brief;
}

/**
 * 生成填充记录时的备注
 * 包含更详细的信息
 */
export function generateFillNotes(result: AnalysisResult): string {
  const parts: string[] = [];

  // 特殊掉落
  if (result.specialItems.length > 0) {
    const itemDetails = result.specialItems.map(item => {
      const priceStr = item.price.toLocaleString() + '金';
      const buyer = item.isWorkerBought ? '自购' : item.buyer;
      return `${item.name}(${buyer}, ${priceStr})`;
    });
    parts.push(`特殊掉落: ${itemDetails.join(', ')}`);
  }

  // 原始备注
  if (result.notes && result.notes.trim()) {
    parts.push(result.notes.trim());
  }

  return parts.join(' | ');
}
