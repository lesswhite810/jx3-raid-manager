import React from 'react';
import { getSectConfig } from '../utils/sectConfig';

interface SectIconProps {
  sectName: string;
  /** 显示模式: 'full' 显示完整名称, 'short' 显示简称 */
  variant?: 'full' | 'short' | 'badge';
  /** 自定义样式类 */
  className?: string;
  /** 图标尺寸 */
  size?: 'xs' | 'sm' | 'md' | 'lg';
}

/**
 * 职业图标组件
 * 根据职业名称显示带有配色的职业标识
 */
export const SectIcon: React.FC<SectIconProps> = ({
  sectName,
  variant = 'badge',
  className = '',
  size = 'sm'
}) => {
  const config = getSectConfig(sectName);

  // 尺寸配置
  const sizeClasses = {
    xs: 'text-[10px] px-1.5 py-0.5',
    sm: 'text-[11px] px-2 py-0.5',
    md: 'text-sm px-2.5 py-1',
    lg: 'text-base px-3 py-1'
  };

  if (!sectName) {
    return null;
  }

  if (variant === 'short') {
    // 显示简称（小图标）
    return (
      <span
        className={`
          inline-flex items-center justify-center
          font-bold rounded-md min-w-[1.5rem] h-6
          ${config.color} ${config.textColor} ${config.borderColor}
          border
          ${sizeClasses[size]}
          ${className}
        `}
        title={config.name}
      >
        {config.shortName}
      </span>
    );
  }

  if (variant === 'full') {
    // 显示完整名称
    return (
      <span
        className={`
          inline-flex items-center
          font-medium rounded-md
          ${config.color} ${config.textColor} ${config.borderColor}
          border
          ${sizeClasses[size]}
          ${className}
        `}
      >
        {config.name}
      </span>
    );
  }

  // 默认 badge 模式：紧凑显示
  return (
    <span
      className={`
        inline-flex items-center
        font-medium rounded-md
        ${config.color} ${config.textColor} ${config.borderColor}
        border
        ${sizeClasses[size]}
        ${className}
      `}
    >
      {config.shortName}
    </span>
  );
};

export default SectIcon;
