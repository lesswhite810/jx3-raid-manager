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
                className="fixed inset-0 bg-slate-900/60 z-40"
                onClick={onClose}
            />
            <div className="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
                <div className="bg-white rounded-xl shadow-2xl w-full max-w-md overflow-hidden pointer-events-auto animate-in fade-in zoom-in-95 duration-200">
                    <div className="bg-slate-800 px-6 py-4 flex items-center justify-between">
                        <h2 className="text-lg font-semibold text-white">新增角色</h2>
                        <button
                            onClick={onClose}
                            className="text-slate-400 hover:text-white transition-colors p-1.5 rounded-lg hover:bg-slate-700"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>

                    <form onSubmit={handleSubmit} className="p-6 space-y-4">
                        {error && (
                            <div className="p-3 bg-red-50 border border-red-200 rounded-lg flex items-center gap-2 text-red-700 text-sm">
                                <AlertCircle className="w-4 h-4" />
                                <span>{error}</span>
                            </div>
                        )}

                        <div>
                            <label className="block text-sm font-medium text-slate-700 mb-1.5">
                                角色名称 <span className="text-red-500">*</span>
                            </label>
                            <div className="relative">
                                <User className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
                                <input
                                    type="text"
                                    value={name}
                                    onChange={e => setName(e.target.value)}
                                    className="w-full pl-9 pr-3 py-2.5 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500 outline-none transition-all placeholder:text-slate-400"
                                    placeholder="输入角色名称"
                                    autoFocus
                                />
                            </div>
                        </div>

                        <div className="grid grid-cols-2 gap-4">
                            <div>
                                <label className="block text-sm font-medium text-slate-700 mb-1.5">
                                    大区 <span className="text-red-500">*</span>
                                </label>
                                <div className="relative">
                                    <MapPin className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
                                    <input
                                        type="text"
                                        value={region}
                                        onChange={e => setRegion(e.target.value)}
                                        className="w-full pl-9 pr-3 py-2.5 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500 outline-none transition-all"
                                    />
                                </div>
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-slate-700 mb-1.5">
                                    服务器 <span className="text-red-500">*</span>
                                </label>
                                <div className="relative">
                                    <Server className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
                                    <input
                                        type="text"
                                        value={server}
                                        onChange={e => setServer(e.target.value)}
                                        className="w-full pl-9 pr-3 py-2.5 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500 outline-none transition-all"
                                    />
                                </div>
                            </div>
                        </div>

                        <div>
                            <label className="block text-sm font-medium text-slate-700 mb-1.5">
                                门派
                            </label>
                            <div className="relative">
                                <Sparkles className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
                                <select
                                    value={sect}
                                    onChange={e => setSect(e.target.value)}
                                    className="w-full pl-9 pr-3 py-2.5 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500 outline-none transition-all appearance-none bg-white"
                                >
                                    <option value="">请选择门派</option>
                                    {SECTS.map(s => (
                                        <option key={s} value={s}>{s}</option>
                                    ))}
                                </select>
                            </div>
                        </div>

                        <div>
                            <label className="block text-sm font-medium text-slate-700 mb-1.5">
                                装分
                            </label>
                            <div className="relative">
                                <Shield className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
                                <input
                                    type="number"
                                    min="0"
                                    value={equipmentScore === undefined ? '' : equipmentScore}
                                    onChange={e => {
                                        const val = e.target.value;
                                        setEquipmentScore(val === '' ? undefined : parseInt(val));
                                    }}
                                    className="w-full pl-9 pr-3 py-2.5 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500 outline-none transition-all"
                                    placeholder="可选"
                                />
                            </div>
                        </div>

                        <div className="flex items-center gap-3 pt-2">
                            <input
                                type="checkbox"
                                id="isClient"
                                checked={isClient}
                                onChange={e => setIsClient(e.target.checked)}
                                className="w-4 h-4 text-emerald-600 rounded border-slate-300 focus:ring-emerald-500"
                            />
                            <label htmlFor="isClient" className="text-sm text-slate-700 select-none cursor-pointer">
                                这是代清角色
                            </label>
                        </div>

                        <div className="flex gap-3 pt-4">
                            <button
                                type="button"
                                onClick={onClose}
                                className="flex-1 px-4 py-2.5 border border-slate-300 text-slate-700 rounded-lg font-medium hover:bg-slate-50 transition-colors"
                            >
                                取消
                            </button>
                            <button
                                type="submit"
                                className="flex-1 px-4 py-2.5 bg-emerald-600 text-white rounded-lg font-medium hover:bg-emerald-700 transition-colors shadow-sm"
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
