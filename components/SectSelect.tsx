import React, { useState, useRef, useEffect } from 'react';
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
    const containerRef = useRef<HTMLDivElement>(null);

    // 点击外部关闭下拉菜单
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    // 检测下拉菜单是否超出视口
    const [dropdownUp, setDropdownUp] = useState(false);
    useEffect(() => {
        if (isOpen && containerRef.current) {
            const rect = containerRef.current.getBoundingClientRect();
            const dropdownHeight = 280; // 估算下拉菜单高度
            const spaceBelow = window.innerHeight - rect.bottom;
            setDropdownUp(spaceBelow < dropdownHeight);
        }
    }, [isOpen]);

    const selectedSect = value;
    const selectedIconPath = selectedSect ? getSectIconPath(selectedSect) : null;
    const selectedConfig = selectedSect ? getSectConfig(selectedSect) : null;

    return (
        <div ref={containerRef} className="relative">
            <div
                className={`
                    relative group cursor-pointer
                    ${error ? 'ring-1 ring-red-500' : ''}
                `}
                onClick={() => setIsOpen(!isOpen)}
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

            {/* 下拉菜单 */}
            {isOpen && (
                <div
                    className={`
                        absolute z-50 w-full bg-surface border border-base rounded-lg shadow-lg
                        max-h-64 overflow-y-auto animate-in fade-in slide-in-from-top-1 duration-150
                        ${dropdownUp ? 'bottom-full mb-1 origin-bottom' : 'top-full mt-1 origin-top'}
                    `}
                >
                    <div className="py-1">
                        {/* 空选项 */}
                        <div
                            className="px-3 py-2 hover:bg-base cursor-pointer text-muted transition-colors flex items-center gap-2"
                            onClick={() => {
                                onChange('');
                                setIsOpen(false);
                            }}
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
                                    onClick={() => {
                                        onChange(sect);
                                        setIsOpen(false);
                                    }}
                                >
                                    {iconPath && (
                                        <img
                                            src={iconPath}
                                            alt=""
                                            className="w-5 h-5 object-contain"
                                        />
                                    )}
                                    <span className="font-medium">{sectConfig.shortName}</span>
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
