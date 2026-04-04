import React, { useState, useRef, useEffect, useCallback } from 'react';
import { SECTS } from '../constants';
import { getSectConfig, getSectIconPath } from '../utils/sectConfig';
import { ChevronDown } from 'lucide-react';

interface SectSelectProps {
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    error?: string | boolean;
}

export const SectSelect: React.FC<SectSelectProps> = ({
    value,
    onChange,
    placeholder = '请选择心法',
    error = false
}) => {
    const [isOpen, setIsOpen] = useState(false);
    const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties | null>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const triggerRef = useRef<HTMLDivElement>(null);

    // 点击外部关闭下拉菜单
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
                setIsOpen(false);
                setDropdownStyle(null);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    const toggleDropdown = useCallback(() => {
        if (isOpen) {
            setIsOpen(false);
            setDropdownStyle(null);
        } else {
            // 计算下拉菜单位置
            if (triggerRef.current) {
                const rect = triggerRef.current.getBoundingClientRect();
                const dropdownHeight = 320; // 下拉菜单估算高度
                const gap = 8; // 间距
                const spaceBelow = window.innerHeight - rect.bottom - gap;
                const openUp = spaceBelow < dropdownHeight;

                setDropdownStyle({
                    position: 'fixed' as const,
                    top: openUp ? rect.top - gap - dropdownHeight : rect.bottom + gap,
                    left: rect.left,
                    width: rect.width,
                    zIndex: 9999,
                    transformOrigin: openUp ? 'bottom' : 'top'
                });
                setIsOpen(true);
            }
        }
    }, [isOpen]);

    const handleSelect = (sect: string) => {
        onChange(sect);
        setIsOpen(false);
        setDropdownStyle(null);
    };

    const selectedSect = value;
    const selectedIconPath = selectedSect ? getSectIconPath(selectedSect) : null;
    const selectedConfig = selectedSect ? getSectConfig(selectedSect) : null;

    return (
        <div ref={containerRef} className="relative">
            <div
                ref={triggerRef}
                className={`
                    relative group cursor-pointer
                    ${error ? 'ring-1 ring-red-500' : ''}
                `}
                onClick={toggleDropdown}
            >
                {/* 选中值显示 */}
                <div className="w-full pl-9 pr-10 py-2.5 bg-base border border-base rounded-lg transition-all text-main">
                    {selectedSect ? (
                        <div className="flex items-center gap-2">
                            {selectedIconPath && (
                                <img
                                    src={selectedIconPath}
                                    alt=""
                                    className="w-5 h-5 object-contain"
                                />
                            )}
                            <span>{selectedConfig?.shortName || selectedSect}</span>
                        </div>
                    ) : (
                        <div className="flex items-center gap-2">
                            <span className="w-5 h-5 flex items-center justify-center text-muted text-xs">-</span>
                            <span className="text-muted">{placeholder}</span>
                        </div>
                    )}
                </div>
                {/* 下拉箭头 */}
                <ChevronDown
                    className={`
                        absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted
                        transition-transform duration-200 pointer-events-none
                        ${isOpen ? 'rotate-180' : ''}
                    `}
                />
            </div>

            {/* 下拉菜单 - 使用 fixed 定位 */}
            {isOpen && dropdownStyle && (
                <div
                    className="fixed bg-surface border border-base rounded-lg shadow-xl max-h-72 overflow-y-auto"
                    style={dropdownStyle}
                >
                    <div className="py-1">
                        {/* 空选项 */}
                        <div
                            className="px-3 py-2 hover:bg-base cursor-pointer text-muted transition-colors flex items-center gap-2"
                            onClick={() => handleSelect('')}
                        >
                            <span>{placeholder}</span>
                        </div>
                        {/* 心法选项 */}
                        {SECTS.map((sect) => {
                            const iconPath = getSectIconPath(sect);
                            const sectConfig = getSectConfig(sect);
                            const isSelected = sect === selectedSect;
                            return (
                                <div
                                    key={sect}
                                    className={`
                                        px-3 py-2 cursor-pointer transition-colors flex items-center gap-2
                                        ${isSelected
                                            ? 'bg-primary/10 text-primary'
                                            : 'hover:bg-base text-main'
                                        }
                                    `}
                                    onClick={() => handleSelect(sect)}
                                >
                                    {iconPath && (
                                        <img
                                            src={iconPath}
                                            alt=""
                                            className="w-5 h-5 object-contain"
                                        />
                                    )}
                                    <span className="font-medium">{sectConfig?.shortName || sect}</span>
                                    <span className="text-muted text-sm">- {sect}</span>
                                </div>
                            );
                        })}
                    </div>
                </div>
            )}
        </div>
    );
};
