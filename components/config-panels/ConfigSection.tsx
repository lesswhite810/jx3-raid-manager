import React from 'react';
import {
    ROW_CLASS,
    SECTION_CARD_CLASS,
    SECTION_HEADING_BAR_CLASS,
    SECTION_HEADING_CLASS,
    SECTION_HINT_CLASS,
    SWITCH_KNOB_CLASS,
    SWITCH_KNOB_ON_CLASS,
    SWITCH_TRACK_CLASS,
    SWITCH_TRACK_OFF_CLASS,
    SWITCH_TRACK_ON_CLASS,
} from './configPanelStyles';

export interface ConfigSectionProps {
    /** 分组标题 */
    title: string;
    /** 标题下方的一句话说明，省略时不渲染 */
    hint?: string;
    children: React.ReactNode;
}

/**
 * 系统配置的分组：标题 + 说明 + 卡片容器。
 *
 * 卡片内直接放若干 `ConfigRow`，行间分隔线由卡片容器的 `divide-y` 提供，
 * 因此组与组之间的间隔会明显大于组内行与行之间的间隔。
 */
export const ConfigSection: React.FC<ConfigSectionProps> = ({ title, hint, children }) => (
    <section>
        <h4 className={SECTION_HEADING_CLASS}>
            <span className={SECTION_HEADING_BAR_CLASS} aria-hidden="true" />
            {title}
        </h4>
        {hint && <p className={`${SECTION_HINT_CLASS} mb-2`}>{hint}</p>}
        <div className={`${SECTION_CARD_CLASS} divide-y divide-base`}>{children}</div>
    </section>
);

export interface ConfigToggleRowProps {
    /** 行标签 */
    label: string;
    /** 行内说明文字 */
    description: string;
    /** 说明元素的 id，供开关的 aria-describedby 使用 */
    descriptionId?: string;
    checked: boolean;
    onChange: (next: boolean) => void;
    disabled?: boolean;
    /** 保存中：禁用控件并标记 aria-busy */
    busy?: boolean;
    /** 开关的无障碍名称，省略时使用 label */
    ariaLabel?: string;
}

/**
 * 「标签 + 说明 + 右侧开关」的标准行。
 *
 * 开关使用 `role="switch"` + `aria-checked`，与仓库既有实现保持一致；
 * 按钮的可访问名称优先取 ariaLabel，避免屏幕阅读器把整段说明当成名称。
 */
export const ConfigToggleRow: React.FC<ConfigToggleRowProps> = ({
    label,
    description,
    descriptionId,
    checked,
    onChange,
    disabled = false,
    busy = false,
    ariaLabel,
}) => (
    <div className={ROW_CLASS}>
        <div className="flex-1 min-w-0 flex items-center gap-4">
            <span className="text-sm font-medium text-muted whitespace-nowrap w-20 flex-shrink-0">{label}</span>
            <p id={descriptionId} className="text-xs text-muted">
                {description}
            </p>
        </div>
        <button
            type="button"
            role="switch"
            onClick={() => onChange(!checked)}
            disabled={disabled || busy}
            aria-checked={checked}
            aria-busy={busy}
            aria-label={ariaLabel ?? label}
            aria-describedby={descriptionId}
            className={`${SWITCH_TRACK_CLASS} ${checked ? SWITCH_TRACK_ON_CLASS : SWITCH_TRACK_OFF_CLASS}`}
        >
            <span className={`${SWITCH_KNOB_CLASS} ${checked ? SWITCH_KNOB_ON_CLASS : ''}`} />
        </button>
    </div>
);
