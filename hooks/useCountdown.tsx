import { useState, useEffect, useCallback, useRef } from 'react';

interface UseCountdownOptions {
  interval?: number;
  onComplete?: () => void;
}

interface UseCountdownReturn {
  days: number;
  hours: number;
  minutes: number;
  seconds: number;
  totalMs: number;
  isExpired: boolean;
  formatted: string;
}

export const useCountdown = (
  targetTime: number | Date,
  options: UseCountdownOptions = {}
): UseCountdownReturn => {
  const { interval = 1000, onComplete } = options;
  const [totalMs, setTotalMs] = useState(() => {
    const target = targetTime instanceof Date ? targetTime.getTime() : targetTime;
    const now = Date.now();
    return Math.max(0, target - now);
  });
  
  const intervalRef = useRef<NodeJS.Timeout | null>(null);
  const onCompleteRef = useRef(onComplete);

  useEffect(() => {
    onCompleteRef.current = onComplete;
  }, [onComplete]);

  useEffect(() => {
    const target = targetTime instanceof Date ? targetTime.getTime() : targetTime;
    const now = Date.now();
    const remaining = Math.max(0, target - now);
    setTotalMs(remaining);

    intervalRef.current = setInterval(() => {
      const current = Date.now();
      const newRemaining = Math.max(0, target - current);
      setTotalMs(newRemaining);

      if (newRemaining <= 0 && intervalRef.current) {
        clearInterval(intervalRef.current);
        onCompleteRef.current?.();
      }
    }, interval);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, [targetTime, interval]);

  const days = Math.floor(totalMs / (1000 * 60 * 60 * 24));
  const hours = Math.floor((totalMs % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
  const minutes = Math.floor((totalMs % (1000 * 60 * 60)) / (1000 * 60));
  const seconds = Math.floor((totalMs % (1000 * 60)) / 1000);

  const isExpired = totalMs <= 0;

  const formatWithDays = () => {
    if (days > 0) {
      return `${days}天 ${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
    }
    return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  };

  return {
    days,
    hours,
    minutes,
    seconds,
    totalMs,
    isExpired,
    formatted: formatWithDays()
  };
};

interface CountdownDisplayProps {
  targetTime: Date | null;
  onComplete?: () => void;
  className?: string;
  showLabels?: boolean;
  expiredText?: string;
}

export const CountdownDisplay: React.FC<CountdownDisplayProps> = ({
  targetTime,
  onComplete,
  className = '',
  showLabels = false,
  expiredText = '已可用'
}) => {
  const countdown = useCountdown(targetTime || Date.now(), { onComplete });

  if (!targetTime || countdown.isExpired) {
    return (
      <span className={`text-emerald-600 font-medium ${className}`}>
        {expiredText}
      </span>
    );
  }

  if (showLabels) {
    return (
      <div className={`flex items-center gap-1 text-amber-600 font-mono ${className}`}>
        {countdown.days > 0 && (
          <span className="text-sm font-medium">{countdown.days}天</span>
        )}
        <span className="text-sm font-medium">
          {String(countdown.hours).padStart(2, '0')}:
          {String(countdown.minutes).padStart(2, '0')}:
          {String(countdown.seconds).padStart(2, '0')}
        </span>
      </div>
    );
  }

  return (
    <span className={`text-amber-600 font-mono ${className}`}>
      {countdown.formatted}
    </span>
  );
};
