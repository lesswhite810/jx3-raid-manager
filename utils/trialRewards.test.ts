import { describe, expect, it } from 'vitest';
import type { TrialDrawReward } from '../types';
import {
  classifyTrialRewards,
  isTrialRegularReward,
  TRIAL_REGULAR_REWARD_NAMES,
} from './trialRewards';

describe('trialRewards', () => {
  it('按少年白了发 11 月 3 日的四次翻牌分类，并保留收据顺序与数量', () => {
    const draws: TrialDrawReward[][] = [
      [
        { name: '银叶子·试炼之地', quantity: 31 },
        { name: '九花玉露散', quantity: 10 },
        { name: '五行石（一级）', quantity: 200 },
      ],
      [
        { name: '银叶子·试炼之地', quantity: 29 },
        { name: '福禄宝箱·雾海寻龙', quantity: 1 },
        { name: '九花玉露散', quantity: 8 },
      ],
      [
        { name: '银叶子·试炼之地', quantity: 32 },
        { name: '彩·耐受·血魂·破势(肆)', quantity: 1 },
        { name: '九花玉露散', quantity: 10 },
      ],
      [
        { name: '银叶子·试炼之地', quantity: 33 },
        { name: '匡义令·月', quantity: 36 },
        { name: '九花玉露散', quantity: 8 },
      ],
    ];

    const classifiedDraws = draws.map(classifyTrialRewards);

    expect(classifiedDraws.map(({ regularRewards }) => regularRewards)).toEqual([
      [
        { name: '银叶子·试炼之地', quantity: 31 },
        { name: '九花玉露散', quantity: 10 },
      ],
      [
        { name: '银叶子·试炼之地', quantity: 29 },
        { name: '九花玉露散', quantity: 8 },
      ],
      [
        { name: '银叶子·试炼之地', quantity: 32 },
        { name: '九花玉露散', quantity: 10 },
      ],
      [
        { name: '银叶子·试炼之地', quantity: 33 },
        { name: '九花玉露散', quantity: 8 },
      ],
    ]);
    expect(classifiedDraws.map(({ flipRewards }) => flipRewards)).toEqual([
      [{ name: '五行石（一级）', quantity: 200 }],
      [{ name: '福禄宝箱·雾海寻龙', quantity: 1 }],
      [{ name: '彩·耐受·血魂·破势(肆)', quantity: 1 }],
      [{ name: '匡义令·月', quantity: 36 }],
    ]);
    expect(classifiedDraws.every(({ hasObservedRewards }) => hasObservedRewards)).toBe(true);
  });

  it('只按精确名称识别常规奖励，未知物品默认保留为翻牌奖励', () => {
    const rewards: TrialDrawReward[] = [
      { name: '银叶子·试炼之地', quantity: 1 },
      { name: '五行石（一级）', quantity: 200 },
      { name: '九花玉露散', quantity: 1 },
      { name: '匡义令·月', quantity: 36 },
    ];

    const result = classifyTrialRewards(rewards);

    expect(TRIAL_REGULAR_REWARD_NAMES).toEqual(['银叶子·试炼之地', '九花玉露散']);
    expect(isTrialRegularReward(rewards[0])).toBe(true);
    expect(isTrialRegularReward(rewards[1])).toBe(false);
    expect(result.regularRewards).toEqual([rewards[0], rewards[2]]);
    expect(result.flipRewards).toEqual([rewards[1], rewards[3]]);
    expect(result.flipRewards[0]).toBe(rewards[1]);
  });

  it('空收据与仅常规奖励均不推断为无翻牌奖励', () => {
    const emptyResult = classifyTrialRewards<TrialDrawReward>([]);
    const basicOnlyResult = classifyTrialRewards([
      { name: '银叶子·试炼之地', quantity: 30 },
      { name: '九花玉露散', quantity: 10 },
    ]);

    expect(emptyResult).toEqual({
      regularRewards: [],
      flipRewards: [],
      hasObservedRewards: false,
    });
    expect(basicOnlyResult).toEqual({
      regularRewards: [
        { name: '银叶子·试炼之地', quantity: 30 },
        { name: '九花玉露散', quantity: 10 },
      ],
      flipRewards: [],
      hasObservedRewards: true,
    });
  });
});
