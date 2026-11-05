/**
 * 试炼之地奖励分类只用于展示：原始收据、顺序与数量均由调用方保留。
 *
 * 奖励列表为空代表尚未观察到收据，不能据此推断本次没有翻牌奖励；
 * 只有常规奖励时同样只说明当前收据中没有额外物品，不能作为无翻牌奖励的证明。
 */
export interface TrialRewardLike {
  name: string;
}

export interface TrialRewardClassification<T extends TrialRewardLike> {
  /** 银叶子·试炼之地、九花玉露散。 */
  regularRewards: T[];
  /** 除常规奖励外的所有物品，保持原有顺序。 */
  flipRewards: T[];
  /** 是否存在可供分类的实时收据物品。 */
  hasObservedRewards: boolean;
}

export const TRIAL_REGULAR_REWARD_NAMES = [
  '银叶子·试炼之地',
  '九花玉露散',
] as const;

const TRIAL_REGULAR_REWARD_NAME_SET = new Set<string>(TRIAL_REGULAR_REWARD_NAMES);

export function isTrialRegularReward(reward: TrialRewardLike): boolean {
  return TRIAL_REGULAR_REWARD_NAME_SET.has(reward.name);
}

/**
 * 将试炼奖励按展示语义分组，不排序、不合并，也不修改传入的收据记录。
 */
export function classifyTrialRewards<T extends TrialRewardLike>(
  rewards: readonly T[] | null | undefined,
): TrialRewardClassification<T> {
  const observedRewards = rewards ?? [];
  const regularRewards: T[] = [];
  const flipRewards: T[] = [];

  for (const reward of observedRewards) {
    if (isTrialRegularReward(reward)) {
      regularRewards.push(reward);
    } else {
      flipRewards.push(reward);
    }
  }

  return {
    regularRewards,
    flipRewards,
    hasObservedRewards: observedRewards.length > 0,
  };
}
