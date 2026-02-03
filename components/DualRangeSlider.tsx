import React, { useRef, useState, useEffect, useCallback } from 'react';

interface DualRangeSliderProps {
    min: number;
    max: number;
    value: { min: number; max: number };
    onChange: (value: { min: number; max: number }) => void;
    className?: string;
}

export const DualRangeSlider: React.FC<DualRangeSliderProps> = ({ min, max, value, onChange, className = '' }) => {
    const [dragging, setDragging] = useState<'min' | 'max' | null>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const range = max - min;

    // Helper to get percentage from value
    const getPercent = useCallback((val: number) => {
        return Math.round(((val - min) / range) * 100);
    }, [min, range]);

    // Handle interaction
    const handleMove = useCallback((clientX: number) => {
        if (!containerRef.current || !dragging) return;

        const rect = containerRef.current.getBoundingClientRect();
        const percent = Math.min(Math.max(0, (clientX - rect.left) / rect.width), 1);
        const newValue = Math.round(min + percent * range);

        if (dragging === 'min') {
            const nextMin = Math.min(newValue, value.max - 1); // Prevent crossing, allow 1 unit gap
            onChange({ ...value, min: Math.max(min, nextMin) });
        } else {
            const nextMax = Math.max(newValue, value.min + 1);
            onChange({ ...value, max: Math.min(max, nextMax) });
        }
    }, [dragging, min, max, range, value, onChange]);

    // Global event listeners for dragging
    useEffect(() => {
        const onPointerMove = (e: PointerEvent) => handleMove(e.clientX);
        const onPointerUp = () => setDragging(null);

        if (dragging) {
            window.addEventListener('pointermove', onPointerMove);
            window.addEventListener('pointerup', onPointerUp);
        }

        return () => {
            window.removeEventListener('pointermove', onPointerMove);
            window.removeEventListener('pointerup', onPointerUp);
        };
    }, [dragging, handleMove]);

    // Track click to jump (optional, but good UX)
    const handleTrackClick = (e: React.PointerEvent) => {
        if (!containerRef.current) return;
        const rect = containerRef.current.getBoundingClientRect();
        const percent = Math.min(Math.max(0, (e.clientX - rect.left) / rect.width), 1);
        const clickedValue = Math.round(min + percent * range);

        // Decide whether to move min or max based on proximity
        if (Math.abs(clickedValue - value.min) < Math.abs(clickedValue - value.max)) {
            onChange({ ...value, min: Math.min(clickedValue, value.max - 1) });
            setDragging('min');
        } else {
            onChange({ ...value, max: Math.max(clickedValue, value.min + 1) });
            setDragging('max');
        }
    };

    const minPercent = getPercent(value.min);
    const maxPercent = getPercent(value.max);

    return (
        <div className={`relative h-6 flex items-center select-none ${className}`}>
            {/* Track Container */}
            <div
                ref={containerRef}
                className="absolute left-0 right-0 h-1.5 bg-surface border border-base rounded-full cursor-pointer"
                onPointerDown={handleTrackClick}
            >
                {/* Active Range Highlight */}
                <div
                    className="absolute top-0 bottom-0 bg-primary/40 rounded-full"
                    style={{
                        left: `${minPercent}%`,
                        width: `${maxPercent - minPercent}%`
                    }}
                />
            </div>

            {/* Min Thumb */}
            <div
                className={`absolute w-4 h-4 bg-white border border-primary rounded-full shadow-sm cursor-grab z-20 transition-transform active:scale-110 active:cursor-grabbing ${dragging === 'min' ? 'scale-110 z-30 ring-2 ring-primary/20' : ''}`}
                style={{
                    left: `${minPercent}%`,
                    transform: 'translate(-50%, 0)'
                }}
                onPointerDown={(e) => {
                    e.stopPropagation();
                    setDragging('min');
                    e.currentTarget.setPointerCapture(e.pointerId); // Better touch/drag handling
                }}
                onPointerUp={(e) => e.currentTarget.releasePointerCapture(e.pointerId)}
            />

            {/* Max Thumb */}
            <div
                className={`absolute w-4 h-4 bg-white border border-primary rounded-full shadow-sm cursor-grab z-20 transition-transform active:scale-110 active:cursor-grabbing ${dragging === 'max' ? 'scale-110 z-30 ring-2 ring-primary/20' : ''}`}
                style={{
                    left: `${maxPercent}%`,
                    transform: 'translate(-50%, 0)'
                }}
                onPointerDown={(e) => {
                    e.stopPropagation();
                    setDragging('max');
                    e.currentTarget.setPointerCapture(e.pointerId);
                }}
                onPointerUp={(e) => e.currentTarget.releasePointerCapture(e.pointerId)}
            />
        </div>
    );
};
