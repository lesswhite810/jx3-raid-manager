import React, { useEffect, useRef, useState } from 'react';
import { Check, ChevronDown, Filter } from 'lucide-react';

export interface VersionOption {
  value: string;
  label: string;
}

interface VersionSelectProps {
  value: string;
  options: VersionOption[];
  onChange: (value: string) => void;
}

/**
 * 副本管理 · 版本过滤下拉框。
 *
 * 原生 <select> 的展开列表由 WebView2 系统弹层绘制，不吃主题变量：深色下边框
 * 几乎看不见、选中项被系统蓝色高亮、宽度还会随最长的版本名漂移。这里改成自绘
 * 列表，边框 / 悬停 / 聚焦 / 选中态全部走语义 token，三套主题表现一致；宽度按最长
 * 的四字版本名收紧（w-32），图标叠在左内边距上，与项目里其它带图标输入框
 * （pl-9 + 绝对定位图标）对齐，四字之后不再留大片空白。
 */
export const VersionSelect: React.FC<VersionSelectProps> = ({ value, options, onChange }) => {
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
    <div ref={containerRef} className="relative w-32 shrink-0">
      <button
        ref={triggerRef}
        type="button"
        aria-label="选择副本版本"
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        onClick={toggleMenu}
        onKeyDown={handleTriggerKeyDown}
        className={`relative w-full flex items-center pl-9 pr-8 py-1.5 bg-surface border rounded-lg text-sm text-left text-main transition-colors focus:outline-none focus:ring-2 focus:ring-primary/20 ${
          isOpen
            ? 'border-primary focus:border-primary'
            : 'border-base dark:border-ds-border-strong hover:border-primary/50 focus:border-primary'
        }`}
      >
        <Filter className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted pointer-events-none" />
        <span className="flex-1 min-w-0 truncate">{selectedLabel}</span>
        <ChevronDown
          className={`absolute right-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted pointer-events-none transition-transform duration-150 ${isOpen ? 'rotate-180' : ''}`}
        />
      </button>

      {isOpen && menuStyle && (
        <div
          role="listbox"
          aria-label="选择副本版本"
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
                className={`w-full flex items-center justify-between gap-2 px-3 py-2 text-sm text-left transition-colors focus:outline-none focus:bg-base ${
                  isSelected
                    ? 'bg-primary/10 text-ds-accent-strong'
                    : 'text-main hover:bg-base'
                }`}
              >
                <span className="truncate">{option.label}</span>
                {isSelected && <Check className="w-3.5 h-3.5 shrink-0" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
};
