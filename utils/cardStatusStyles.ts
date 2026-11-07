/**
 * 角色卡片状态样式（团队副本 / 试炼之地 / 百战 共用）
 *
 * 背景：三个副本模块此前各自手写一套 getCardStyle / getButtonAddStyle /
 * getButtonViewStyle，token 很快就分叉了——同样是「待确认」，团队副本的
 * 查看按钮用 soft 描边，试炼之地用 DEFAULT 描边；同样是卡片，团队副本有
 * hover 浮起，另外两个没有。手写三份的实现无法靠「记得同步」维持一致。
 *
 * 因此把状态 → 样式的映射收敛到这里，三个模块只负责判断状态，不再各自
 * 拼 class 字符串。规范依据见 specs/design-tokens.md「副本进度卡片四态配色」。
 *
 * 四条不可违反的约定：
 * 1. 卡片描边一律用与底色同色的 soft 档，DEFAULT 只在 hover 时出现。
 *    饱和描边压在浅底上 ΔE 达 43~55，配 border-2 会形成突兀硬框。
 * 2. 实心按钮底色用 *-strong 档，配 ds-fg-inverse。DEFAULT 是给文字/描边
 *    设计的中等明度，做底色时白字对比度不足（jianghu 主题 4.07 < 4.5）。
 * 3. hover 不换实心按钮底色（换 DEFAULT 会把对比度拉回 4.07），反馈交给
 *    shadow-ds-stack + 上浮位移。
 * 4. 四态语义固定：未清 success / 部分清 info / 待确认 warning / 完全清 neutral。
 *    不要在任何模块里把 warning 当作「部分清」——历史上团队副本这么用过，
 *    导致同一个棕色在两个页面含义相反。
 */

/** 卡片状态。百战只有 none / complete 两态，另外两态不会被传入。 */
export type CardStatus = 'none' | 'partial' | 'pending' | 'complete';

/** 状态 → 卡片底色 + 描边（描边取与底色同色的 soft 档，hover 才提升到 DEFAULT） */
const CARD_CLASS: Record<CardStatus, string> = {
    none: 'bg-ds-success-soft dark:bg-ds-success-soft/10 border-ds-success-soft dark:border-ds-success-soft hover:border-ds-success hover:shadow-ds-stack',
    partial: 'bg-ds-info-soft dark:bg-ds-info-soft/20 border-ds-info-soft dark:border-ds-info-soft hover:border-ds-info hover:shadow-ds-stack',
    pending: 'bg-ds-warning-soft dark:bg-ds-warning-soft/20 border-ds-warning-soft dark:border-ds-warning-soft hover:border-ds-warning hover:shadow-ds-stack',
    complete: 'bg-base dark:bg-ds-neutral-soft/10 border-ds-neutral-soft dark:border-ds-neutral-soft hover:border-ds-neutral hover:shadow-ds-stack',
};

/** 「添加记录」按钮：未清 / 完全清为实心主操作，部分清 / 待确认为描边 */
const ADD_BUTTON_CLASS: Record<CardStatus, string> = {
    none: 'bg-ds-success-strong text-ds-fg-inverse hover:shadow-ds-stack transform hover:-translate-y-0.5',
    partial: 'bg-surface text-ds-info-strong border border-ds-info hover:bg-ds-info-soft hover:border-ds-info hover:shadow-ds-stack transform hover:-translate-y-0.5',
    pending: 'bg-surface text-ds-warning-strong border border-ds-warning hover:bg-ds-warning-soft hover:border-ds-warning hover:shadow-ds-stack transform hover:-translate-y-0.5',
    complete: 'bg-ds-neutral text-ds-fg-inverse hover:shadow-ds-stack transform hover:-translate-y-0.5',
};

/** 「查看详情」按钮：四态统一描边，底色跟随主题（不用写死的 bg-white） */
const VIEW_BUTTON_CLASS: Record<CardStatus, string> = {
    none: 'bg-surface text-ds-success-strong border border-ds-success-soft hover:bg-ds-success-soft hover:border-ds-success',
    partial: 'bg-surface text-ds-info-strong border border-ds-info-soft hover:bg-ds-info-soft hover:border-ds-info',
    pending: 'bg-surface text-ds-warning-strong border border-ds-warning-soft hover:bg-ds-warning-soft hover:border-ds-warning',
    complete: 'bg-surface text-ds-neutral border border-ds-neutral-soft hover:bg-ds-neutral-soft hover:border-ds-neutral',
};

/** 按钮不可用时的统一外观（禁用态不参与状态配色） */
const DISABLED_BUTTON_CLASS = 'bg-base text-muted cursor-not-allowed border border-base';

/** 次级操作按钮（如试炼的「翻牌统计」）：中性描边，不抢主操作 */
export const SECONDARY_BUTTON_CLASS =
    'bg-surface text-main border border-base hover:bg-base hover:border-ds-neutral';

/** 卡片：底色 + 描边。调用方自己拼 p-4 / rounded-card / border-2 / transition */
export function getCardClass(status: CardStatus): string {
    return CARD_CLASS[status];
}

/** 「添加记录」按钮配色。disabled 为 true 时优先返回禁用外观。 */
export function getAddButtonClass(status: CardStatus, disabled = false): string {
    if (disabled) return DISABLED_BUTTON_CLASS;
    return ADD_BUTTON_CLASS[status];
}

/** 「查看详情」按钮配色 */
export function getViewButtonClass(status: CardStatus): string {
    return VIEW_BUTTON_CLASS[status];
}
