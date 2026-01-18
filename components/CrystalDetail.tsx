import React, { useMemo, useState } from 'react';
import { Trophy, Users, ArrowLeft } from 'lucide-react';
import { RaidRecord, Account } from '../types';

interface CrystalDetailProps {
  records: RaidRecord[];
  accounts: Account[];
  onBack: () => void;
}

interface CrystalRoleStats {
  roleId: string;
  roleName: string;
  server: string;
  totalCount: number;
  records: RaidRecord[];
}

export const CrystalDetail: React.FC<CrystalDetailProps> = ({ records, accounts, onBack }) => {
  console.log('[CrystalDetail] Component rendered with records:', records?.length, 'accounts:', accounts?.length);
  
  const [expandedRoleId, setExpandedRoleId] = useState<string | null>(null);

  const safeRecords = Array.isArray(records) ? records : [];
  const safeAccounts = Array.isArray(accounts) ? accounts : [];

  const findRoleInfo = (accountId: string, roleId: string): { roleName: string; server: string } => {
    console.log('[CrystalDetail] findRoleInfo called with accountId:', accountId, 'roleId:', roleId);
    
    for (const account of safeAccounts) {
      if (account.id === accountId) {
        const roles = account.roles || [];
        const role = roles.find((r: any) => r.id === roleId);
        if (role) {
          console.log('[CrystalDetail] Role found:', role);
          return {
            roleName: role.name,
            server: `${role.region} ${role.server}`
          };
        }
      }
    }
    console.log('[CrystalDetail] Role not found, returning defaults');
    return { roleName: '', server: '' };
  };

  const crystalRecords = useMemo(() => {
    const filtered = safeRecords.filter(r => r.hasXuanjing === true);
    console.log('[CrystalDetail] crystalRecords filtered:', filtered.length, 'out of', safeRecords.length);
    return filtered;
  }, [safeRecords]);

  const roleStats = useMemo<CrystalRoleStats[]>(() => {
    console.log('[CrystalDetail] Computing roleStats from', crystalRecords.length, 'crystal records');
    const roleMap = new Map<string, CrystalRoleStats>();

    crystalRecords.forEach(record => {
      const roleId = record.roleId || record.accountId;
      console.log('[CrystalDetail] Processing record:', record.id, 'roleId:', roleId, 'accountId:', record.accountId);
      
      if (!roleMap.has(roleId)) {
        const roleInfo = findRoleInfo(record.accountId, record.roleId);
        console.log('[CrystalDetail] roleInfo for record:', roleInfo);
        roleMap.set(roleId, {
          roleId,
          roleName: record.roleName || roleInfo.roleName || '未知角色',
          server: record.server || roleInfo.server || '未知服务器',
          totalCount: 0,
          records: []
        });
      }

      const stats = roleMap.get(roleId)!;
      stats.totalCount++;
      stats.records.push(record);
    });

    const result = Array.from(roleMap.values()).sort((a, b) => b.totalCount - a.totalCount);
    console.log('[CrystalDetail] Final roleStats:', result);
    return result;
  }, [crystalRecords]);

  const toggleExpand = (roleId: string) => {
    console.log('[CrystalDetail] toggleExpand called with roleId:', roleId, 'current expandedRoleId:', expandedRoleId);
    setExpandedRoleId(expandedRoleId === roleId ? null : roleId);
  };

  const totalCrystals = useMemo(() => {
    return crystalRecords.length;
  }, [crystalRecords]);

  const totalRoles = roleStats.length;

  console.log('[CrystalDetail] Render state - totalCrystals:', totalCrystals, 'totalRoles:', totalRoles, 'roleStats.length:', roleStats.length);

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <button
          onClick={onBack}
          className="p-2 hover:bg-slate-100 rounded-lg transition-colors"
        >
          <ArrowLeft className="w-5 h-5 text-slate-600" />
        </button>
        <div>
          <h2 className="text-2xl font-bold text-slate-800">玄晶记录</h2>
          <p className="text-sm text-slate-500 mt-1">
            共获取 {totalCrystals} 次玄晶，来自 {totalRoles} 个角色
          </p>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div className="bg-white rounded-xl p-5 shadow-sm border border-slate-200">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-2 bg-slate-100 rounded-lg">
              <Trophy className="w-5 h-5 text-slate-600" />
            </div>
            <span className="text-slate-600 font-medium">总获取次数</span>
          </div>
          <p className="text-3xl font-bold text-slate-800">{totalCrystals}</p>
        </div>
        <div className="bg-white rounded-xl p-5 shadow-sm border border-slate-200">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-2 bg-slate-100 rounded-lg">
              <Users className="w-5 h-5 text-slate-600" />
            </div>
            <span className="text-slate-600 font-medium">涉及角色</span>
          </div>
          <p className="text-3xl font-bold text-slate-800">{totalRoles}</p>
        </div>
      </div>

      <div className="bg-white rounded-xl shadow-sm border border-slate-200">
        {roleStats.length === 0 ? (
          <div className="text-center py-12">
            <Trophy className="w-12 h-12 text-slate-300 mx-auto mb-3" />
            <p className="text-slate-500">暂无玄晶记录</p>
            <p className="text-sm text-slate-400 mt-1">通关有玄晶奖励的副本后会自动记录</p>
            {safeRecords.length > 0 && (
              <p className="text-xs text-slate-300 mt-2">共有 {safeRecords.length} 条记录，其中 {crystalRecords.length} 条有玄晶标记</p>
            )}
          </div>
        ) : (
          <div className="space-y-1">
            {roleStats.map((stat, index) => {
              console.log('[CrystalDetail] Rendering role stat:', stat.roleName, 'count:', stat.totalCount, 'records:', stat.records.length);
              return (
                <div key={stat.roleId} className="border-b last:border-b-0">
                  <button
                    onClick={() => toggleExpand(stat.roleId)}
                    className="w-full flex items-center gap-4 p-4 hover:bg-slate-50 transition-colors text-left"
                  >
                    <div className="flex items-center justify-center w-8 h-8 bg-slate-100 rounded-lg font-bold text-sm text-slate-700">
                      {index + 1}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <h4 className="font-semibold text-slate-800 truncate">{stat.roleName}</h4>
                        {index === 0 && stat.totalCount > 0 && (
                          <span className="px-2 py-0.5 bg-slate-100 text-slate-700 text-xs rounded-full">榜首</span>
                        )}
                      </div>
                      <p className="text-sm text-slate-500 truncate">{stat.server}</p>
                    </div>
                    <div className="flex items-center gap-2">
                      <Trophy className="w-4 h-4 text-slate-500" />
                      <span className="text-xl font-bold text-slate-800">{stat.totalCount}</span>
                      <span className="text-sm text-slate-400">次</span>
                      <svg
                        className={`w-4 h-4 text-slate-400 transition-transform duration-200 ${expandedRoleId === stat.roleId ? 'rotate-180' : ''}`}
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                      </svg>
                    </div>
                  </button>
                  
                  {expandedRoleId === stat.roleId && (
                    <div className="bg-white p-4 border-t border-slate-200">
                      <div className="space-y-3">
                        {stat.records.map((record, recordIndex) => (
                          <div key={`${record.id || record.date}-${recordIndex}`} className="flex items-center gap-4 p-3 border border-slate-200 rounded-lg">
                            <div className="flex-shrink-0 text-sm text-slate-500">
                              {new Date(record.date).toLocaleString('zh-CN', {
                                year: 'numeric',
                                month: '2-digit',
                                day: '2-digit',
                                hour: '2-digit',
                                minute: '2-digit'
                              })}
                            </div>
                            <div className="flex-1">
                              <p className="font-medium text-slate-800">{record.raidName}</p>
                              <p className="text-xs text-slate-500">{record.notes || ''}</p>
                            </div>
                            <div className="flex-shrink-0 bg-slate-100 text-slate-700 px-2 py-0.5 rounded text-xs">
                              {record.hasXuanjing ? '玄晶' : '普通'}
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};
