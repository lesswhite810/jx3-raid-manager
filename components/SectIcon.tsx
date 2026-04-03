import React from 'react';
import { getSectConfig, getSectIconPath } from '../utils/sectConfig';

interface SectIconProps {
  sectName: string;
  /** 显示模式: 'image' 仅图标, 'full' 显示完整名称+图标, 'short' 显示简称+图标, 'badge' 仅简称 */
  variant?: 'image' | 'full' | 'short' | 'badge';
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
  variant = 'image',
  className = '',
  size = 'sm'
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
    xs: 'w-5 h-5',
    sm: 'w-6 h-6',
    md: 'w-8 h-8',
    lg: 'w-10 h-10'
  };

  if (!sectName) {
    return null;
  }

  // 仅显示图标模式（默认）
  if (variant === 'image') {
    if (!iconPath) {
      // 无图标时显示简称作为备选
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
    return (
      <img
        src={iconPath}
        alt={config.name}
        className={`${iconSizes[size]} object-contain ${className}`}
        title={config.name}
      />
    );
  }

  // 显示完整名称+图标
  if (variant === 'full') {
    return (
      <span
        className={`
          inline-flex items-center gap-1.5
          font-medium rounded-md
          ${config.color} ${config.textColor} ${config.borderColor}
          border
          ${sizeClasses[size]}
          ${className}
        `}
        title={config.name}
      >
        {iconPath && (
          <img
            src={iconPath}
            alt=""
            className={`${iconSizes[size]} object-contain`}
          />
        )}
        <span>{config.name}</span>
      </span>
    );
  }

  // 显示简称+图标
  if (variant === 'short') {
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
        {iconPath && (
          <img
            src={iconPath}
            alt=""
            className={`${iconSizes[size]} object-contain`}
          />
        )}
        <span>{config.shortName}</span>
      </span>
    );
  }

  // badge 模式：仅显示简称
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
};

export default SectIcon;
