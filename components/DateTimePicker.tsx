import React from 'react';
import DatePicker, { registerLocale } from 'react-datepicker';
import { zhCN } from 'date-fns/locale/zh-CN';
import 'react-datepicker/dist/react-datepicker.css';

// 注册中文本地化
registerLocale('zh-CN', zhCN);

interface DateTimePickerProps {
    value: string | Date;
    onChange: (date: string) => void;
    className?: string;
}

export const DateTimePicker: React.FC<DateTimePickerProps> = ({ value, onChange, className = '' }) => {
    // 确保默认值为有效的 Date 对象
    const selectedDate = value ? new Date(value) : new Date();

    const handleChange = (date: Date | null) => {
        if (date) {
            // 提供不随失去焦点改变而变化的稳定输出，将 Date 转换为 string
            // ISO 格式可能会受到时区影响，但原生 datetime-local 是使用本地时间，因此可以直接返回本地时间的 ISO 格式字符串（包含时区信息）以便后端和存储正确处理
            // 这里我们保持和之前原生 string 处理习惯大致一致的格式输出给外部（假设为 ISO string）
            const offset = date.getTimezoneOffset()
            const localDate = new Date(date.getTime() - (offset * 60 * 1000))
            const isoString = localDate.toISOString().slice(0, 16); // 取前面的 YYYY-MM-DDTHH:mm
            onChange(isoString);
        }
    };

    return (
        <DatePicker
            selected={selectedDate}
            onChange={handleChange}
            showTimeSelect
            timeFormat="HH:mm"
            timeIntervals={15}
            timeCaption="时间"
            dateFormat="yyyy-MM-dd HH:mm"
            locale="zh-CN"
            isClearable={false}
            className={`w-full px-3 py-2.5 bg-surface border border-base rounded-lg text-main focus:outline-none focus:ring-2 focus:ring-primary transition-all text-sm custom-datepicker-input ${className}`}
            wrapperClassName="w-full"
            popperClassName="custom-datepicker-popper"
        />
    );
};
