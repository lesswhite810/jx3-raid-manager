import React, { useEffect, useRef, useState } from 'react';
import { Check, ChevronDown } from 'lucide-react';

export interface FilterSelectOption {
  value: string;
  label: string;
  /** 选项文字前的自定义节点（如职业图标） */
  leading?: React.ReactNode;
}

interface FilterSelectProps {
  value: string;
  options: FilterSelectOption[];
  onChange: (value: string) => void;
  /** 无障碍标签（触发按钮与列表的 aria-label） */
  ariaLabel: string;
  /** 触发按钮宽度，默认 w-32 */
  className?: string;
}

/**
 * 通用自绘筛选下拉框。
 *
 * 与 VersionSelect 同一套自绘实现（原生 <select> 的展开列表由 WebView2 系统弹层
 * 绘制，不吃主题变量），并做了三点参数化：支持空值选项（如「全部心法」）、
 * 宽度可调、选项可携带前置节点（职业图标）。
 */
export const FilterSelect: React.FC<FilterSelectProps> = ({
  value,
  options,
  onChange,
  ariaLabel,
  className = 'w-32',
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [menuStyle, setMenuStyle] = useState<React.CSSProperties | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const optionRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const selectedLabel = options.find(option => option.value === value)?.label ?? options[0]?.label ?? '';

  useEffect(() => {
    if (!isOpen) return;
    const close = () => {
      setIsOpen(false);
      setMenuStyle(null);
    };
    const handlePointerDown = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        close();
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        close();
        triggerRef.current?.focus();
      }
    };
    document.addEventListener('mousedown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen]);

  const openMenu = (focusIndex: number | null = null) => {
    if (!triggerRef.current) return;
    const rect = triggerRef.current.getBoundingClientRect();
    const menuHeight = Math.min(options.length, 8) * 36 + 8;
    const gap = 4;
    const openUp = window.innerHeight - rect.bottom - gap < menuHeight && rect.top > menuHeight;
    setMenuStyle({
      position: 'fixed',
      top: openUp ? rect.top - gap - menuHeight : rect.bottom + gap,
      left: rect.left,
      width: rect.width,
      zIndex: 9999,
    });
    setIsOpen(true);
    if (focusIndex !== null) {
      window.setTimeout(() => optionRefs.current[focusIndex]?.focus(), 0);
    }
  };

  const toggleMenu = () => {
    if (isOpen) {
      setIsOpen(false);
      setMenuStyle(null);
    } else {
      const selectedIndex = options.findIndex(option => option.value === value);
      openMenu(selectedIndex >= 0 ? selectedIndex : 0);
    }
  };

  const handleTriggerKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (isOpen || (event.key !== 'ArrowDown' && event.key !== 'ArrowUp')) return;
    event.preventDefault();
    openMenu(event.key === 'ArrowDown' ? 0 : options.length - 1);
  };

  const handleMenuKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const keys = ['ArrowDown', 'ArrowUp', 'Home', 'End'];
    if (!keys.includes(event.key)) return;
    event.preventDefault();
    const items = optionRefs.current.filter((item): item is HTMLButtonElement => item !== null);
    if (items.length === 0) return;
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    let next: number;
    if (event.key === 'ArrowDown') next = current < 0 ? 0 : (current + 1) % items.length;
    else if (event.key === 'ArrowUp') next = current < 0 ? items.length - 1 : (current - 1 + items.length) % items.length;
    else if (event.key === 'Home') next = 0;
    else next = items.length - 1;
    items[next].focus();
  };

  const handleSelect = (next: string) => {
    onChange(next);
    setIsOpen(false);
    setMenuStyle(null);
    triggerRef.current?.focus();
  };

  return (
    <div ref={containerRef} className={`relative ${className} shrink-0`}>
      <button
        ref={triggerRef}
        type="button"
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        onClick={toggleMenu}
        onKeyDown={handleTriggerKeyDown}
        className={`w-full flex items-center justify-between gap-2 px-3 py-1.5 bg-surface border rounded-lg text-sm text-left text-main transition-colors focus:outline-none focus:ring-2 focus:ring-primary/20 ${
          isOpen
            ? 'border-primary focus:border-primary'
            : 'border-base dark:border-ds-border-strong hover:border-primary/50 focus:border-primary'
        }`}
      >
        <span className="flex-1 min-w-0 truncate">{selectedLabel}</span>
        <ChevronDown
          className={`w-3.5 h-3.5 text-muted pointer-events-none shrink-0 transition-transform duration-150 ${isOpen ? 'rotate-180' : ''}`}
        />
      </button>

      {isOpen && menuStyle && (
        <div
          role="listbox"
          aria-label={ariaLabel}
          onKeyDown={handleMenuKeyDown}
          className="py-1 bg-surface border border-base dark:border-ds-border-strong rounded-lg shadow-ds-modal max-h-72 overflow-y-auto"
          style={menuStyle}
        >
          {options.map((option, index) => {
            const isSelected = option.value === value;
            return (
              <button
                key={option.value}
                ref={element => { optionRefs.current[index] = element; }}
                type="button"
                role="option"
                aria-selected={isSelected}
                onClick={() => handleSelect(option.value)}
                className={`w-full flex items-center gap-2 px-3 py-2 text-sm text-left transition-colors focus:outline-none focus:bg-base ${
                  isSelected
                    ? 'bg-primary/10 text-ds-accent-strong'
                    : 'text-main hover:bg-base'
                }`}
              >
                {option.leading && <span className="shrink-0 flex items-center">{option.leading}</span>}
                <span className="flex-1 min-w-0 truncate">{option.label}</span>
                {isSelected && <Check className="w-3.5 h-3.5 shrink-0" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
};

export default FilterSelect;
