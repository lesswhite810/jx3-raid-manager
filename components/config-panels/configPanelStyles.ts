/**
 * 系统配置面板的共享样式常量。
 *
 * 面板内部统一用「分组标题 + 卡片 + 行分隔线」表达层次：
 * - 分组标题（SECTION_HEADING_CLASS）负责回答「这一块是干嘛的」；
 * - 卡片（SECTION_CARD_CLASS）用底色与描边把同组行框在一起；
 * - 行（ROW_CLASS）之间由容器的 `divide-y` 画细线，不再依赖各行自己加 border-t。
 * 这样多组并列时，组与组之间的间隔明显大于组内各行之间的间隔。
 */

/** 分组标题 */
export const SECTION_HEADING_CLASS =
    'flex items-center gap-2 text-[11px] font-bold uppercase tracking-wider text-muted mb-2';

/** 分组标题前的竖条 */
export const SECTION_HEADING_BAR_CLASS = 'w-0.5 h-3 rounded-full bg-primary/50';

/** 分组说明文字 */
export const SECTION_HINT_CLASS = 'text-xs text-muted leading-relaxed';

/** 分组卡片容器 */
export const SECTION_CARD_CLASS = 'rounded-lg border border-base bg-base/20 px-4';

/** 卡片内的一行：上下内边距 + 由容器 divide-y 提供分隔线 */
export const ROW_CLASS = 'flex items-center gap-4 py-3.5';

/** 行标签（固定宽度，让右侧内容对齐） */
export const ROW_LABEL_CLASS = 'text-sm font-medium text-muted whitespace-nowrap w-20 flex-shrink-0';

/** 行内说明文字 */
export const ROW_HINT_CLASS = 'text-xs text-muted';

/** 分段按钮组容器 */
export const SEGMENTED_CLASS = 'inline-flex rounded-card border border-ds-border overflow-hidden flex-shrink-0';

/** 分段按钮基础样式 */
export const SEGMENTED_ITEM_BASE_CLASS = 'px-3 py-1.5 text-xs transition-colors disabled:opacity-50';

/** 分段按钮选中态 */
export const SEGMENTED_ITEM_ACTIVE_CLASS = 'bg-ds-success-soft text-ds-success-strong';

/** 分段按钮未选中态 */
export const SEGMENTED_ITEM_INACTIVE_CLASS = 'bg-ds-surface text-muted hover:bg-ds-surface-2 hover:text-main';

/** 开关轨道 */
export const SWITCH_TRACK_CLASS =
    'relative w-11 h-6 rounded-full transition-colors flex-shrink-0 disabled:cursor-not-allowed disabled:opacity-60';

/** 开关轨道选中底色 */
export const SWITCH_TRACK_ON_CLASS = 'bg-ds-success';

/** 开关轨道未选中底色 */
export const SWITCH_TRACK_OFF_CLASS = 'bg-slate-300 dark:bg-slate-600';

/** 开关滑块 */
export const SWITCH_KNOB_CLASS =
    'absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform';

/** 开关滑块选中位移 */
export const SWITCH_KNOB_ON_CLASS = 'translate-x-5';
