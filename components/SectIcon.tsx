import React from 'react';
import { getSectConfig, getSectIconPath } from '../utils/sectConfig';

interface SectIconProps {
  sectName: string;
  /** 显示模式: 'full' 显示完整名称, 'short' 显示简称, 'icon' 仅图标 */
  variant?: 'full' | 'short' | 'badge' | 'icon';
  /** 自定义样式类 */
  className?: string;
  /** 图标尺寸 */
  size?: 'xs' | 'sm' | 'md' | 'lg';
  /** 是否显示图标 */
  showIcon?: boolean;
}

/**
 * 职业图标组件
 * 根据职业名称显示带有配色的职业标识
 */
export const SectIcon: React.FC<SectIconProps> = ({
  sectName,
  variant = 'badge',
  className = '',
  size = 'sm',
  showIcon = false
}) => {
  const config = getSectConfig(sectName);
  const iconPath = getSectIconPath(sectName);

  // 尺寸配置
  const sizeClasses = {
    xs: 'text-[10px] px-1.5 py-0.5',
    sm: 'text-[11px] px-2 py-0.5',
    md: 'text-sm px-2.5 py-1',
    lg: 'text-base px-3 py-1'
  };

  // 图标尺寸
  const iconSizes = {
    xs: 'w-4 h-4',
    sm: 'w-5 h-5',
    md: 'w-6 h-6',
    lg: 'w-7 h-7'
  };

  if (!sectName) {
    return null;
  }

  // 仅显示图标模式
  if (variant === 'icon') {
    return (
      <img
        src={iconPath}
        alt={config.name}
        className={`${iconSizes[size]} object-contain ${className}`}
        title={config.name}
      />
    );
  }

  // 带图标的徽章模式
  if (showIcon && iconPath) {
    return (
      <span
        className={`
          inline-flex items-center gap-1
          font-medium rounded-md
          ${config.color} ${config.textColor} ${config.borderColor}
          border
          ${sizeClasses[size]}
          ${className}
        `}
        title={config.name}
      >
        <img
          src={iconPath}
          alt=""
          className={`${iconSizes[size]} object-contain`}
        />
        <span>{config.shortName}</span>
      </span>
    );
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
