import React, { useState, useEffect } from 'react';
import { SECTS } from '../constants';
import { X, User, MapPin, Server, Sparkles, Shield, AlertCircle } from 'lucide-react';

interface AddRoleModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSubmit: (data: {
        name: string;
        server: string;
        region: string;
        sect: string;
        equipmentScore?: number;
        isClient: boolean;
    }) => void;
    accountTypeIsClient: boolean; // Used to set default isClient state
}

export const AddRoleModal: React.FC<AddRoleModalProps> = ({
    isOpen,
    onClose,
    onSubmit,
    accountTypeIsClient
}) => {
    const [name, setName] = useState('');
    const [server, setServer] = useState('梦江南');
    const [region, setRegion] = useState('电信区');
    const [sect, setSect] = useState('');
    const [equipmentScore, setEquipmentScore] = useState<number | undefined>(undefined);
    const [isClient, setIsClient] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (isOpen) {
            setName('');
            setServer('梦江南');
            setRegion('电信区');
            setSect('');
            setEquipmentScore(undefined);
            setIsClient(accountTypeIsClient);
            setError(null);
        }
    }, [isOpen, accountTypeIsClient]);

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();
        if (!name.trim()) {
            setError('请输入角色名称');
            return;
        }
        if (!server.trim()) {
            setError('请输入服务器');
            return;
        }
        if (!region.trim()) {
            setError('请输入大区');
            return;
        }

        onSubmit({
            name: name.trim(),
            server: server.trim(),
            region: region.trim(),
            sect: sect || '',
            equipmentScore,
            isClient
        });
        onClose();
    };

    if (!isOpen) return null;

    return (
        <>
            <div
                className="fixed inset-0 bg-black/40 z-40 transition-opacity duration-200"
                onClick={onClose}
            />
            <div className="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
                <div className="bg-surface border border-base rounded-xl shadow-xl w-full max-w-md overflow-hidden pointer-events-auto animate-in fade-in zoom-in-95 duration-200">
                    <div className="px-6 py-4 flex items-center justify-between border-b border-base">
                        <div className="flex items-center gap-2">
                            <User className="w-5 h-5 text-primary" />
                            <h2 className="text-lg font-semibold text-main">新增角色</h2>
                        </div>
                        <button
                            onClick={onClose}
                            className="text-muted hover:text-main hover:bg-base p-1.5 rounded-lg transition-colors"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>

                    <form onSubmit={handleSubmit} className="p-6 space-y-5">
                        {error && (
                            <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-lg flex items-center gap-2 text-red-600 dark:text-red-400 text-sm">
                                <AlertCircle className="w-4 h-4 shrink-0" />
                                <span>{error}</span>
                            </div>
                        )}

                        <div className="space-y-1.5">
                            <label className="block text-sm font-medium text-main ml-1">
                                角色名称 <span className="text-red-500">*</span>
                            </label>
                            <div className="relative group">
                                <User className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                <input
                                    type="text"
                                    value={name}
                                    onChange={e => {
                                        setName(e.target.value);
                                        if (error) setError(null);
                                    }}
                                    className="w-full pl-9 pr-3 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all placeholder:text-muted text-main"
                                    placeholder="输入角色名称"
                                    autoFocus
                                />
                            </div>
                        </div>

                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-1.5">
                                <label className="block text-sm font-medium text-main ml-1">
                                    大区 <span className="text-red-500">*</span>
                                </label>
                                <div className="relative group">
                                    <MapPin className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                    <input
                                        type="text"
                                        value={region}
                                        onChange={e => setRegion(e.target.value)}
                                        className="w-full pl-9 pr-3 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all text-main"
                                    />
                                </div>
                            </div>
                            <div className="space-y-1.5">
                                <label className="block text-sm font-medium text-main ml-1">
                                    服务器 <span className="text-red-500">*</span>
                                </label>
                                <div className="relative group">
                                    <Server className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                    <input
                                        type="text"
                                        value={server}
                                        onChange={e => setServer(e.target.value)}
                                        className="w-full pl-9 pr-3 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all text-main"
                                    />
                                </div>
                            </div>
                        </div>

                        <div className="space-y-1.5">
                            <label className="block text-sm font-medium text-main ml-1">
                                门派
                            </label>
                            <div className="relative group">
                                <Sparkles className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                <select
                                    value={sect}
                                    onChange={e => setSect(e.target.value)}
                                    className="w-full pl-9 pr-3 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all appearance-none text-main"
                                >
                                    <option value="">请选择门派</option>
                                    {SECTS.map(s => (
                                        <option key={s} value={s}>{s}</option>
                                    ))}
                                </select>
                            </div>
                        </div>

                        <div className="space-y-1.5">
                            <label className="block text-sm font-medium text-main ml-1">
                                装分
                            </label>
                            <div className="relative group">
                                <Shield className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                <input
                                    type="number"
                                    min="0"
                                    value={equipmentScore === undefined ? '' : equipmentScore}
                                    onChange={e => {
                                        const val = e.target.value;
                                        setEquipmentScore(val === '' ? undefined : parseInt(val));
                                    }}
                                    className="w-full pl-9 pr-3 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all text-main"
                                    placeholder="可选"
                                />
                            </div>
                        </div>

                        <div className="flex gap-3 pt-4 border-t border-border">
                            <button
                                type="button"
                                onClick={onClose}
                                className="flex-1 px-4 py-2.5 border border-base text-main rounded-lg font-medium hover:bg-base transition-colors active:scale-[0.98]"
                            >
                                取消
                            </button>
                            <button
                                type="submit"
                                className="flex-1 px-4 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium shadow-sm transition-all active:scale-[0.98]"
                            >
                                保存
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </>
    );
};
