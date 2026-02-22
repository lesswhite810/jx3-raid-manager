import React, { useMemo, useState, useEffect } from 'react';
import { ArrowLeft } from 'lucide-react';
import { RaidRecord, Account, TrialPlaceRecord } from '../types';
import { db } from '../services/db';

interface CrystalDetailProps {
  records: RaidRecord[];
  trialRecords: TrialPlaceRecord[];
  accounts: Account[];
  onBack: () => void;
}

interface CrystalRoleStats {
  roleId: string;
  roleName: string;
  server: string;
  totalCount: number;
  records: { id: string; date: string; raidName: string; notes?: string; type: string; equip?: any }[];
}

export const CrystalDetail: React.FC<CrystalDetailProps> = ({ records, trialRecords, accounts, onBack }) => {
  const [expandedRoleId, setExpandedRoleId] = useState<string | null>(null);
  const [equipments, setEquipments] = useState<any[]>([]);

  useEffect(() => {
    db.getEquipments().then((data: any[]) => {
      setEquipments(data.map(d => typeof d === 'string' ? JSON.parse(d) : d));
    }).catch(console.error);
  }, []);

  const findEquipmentById = (id: string | undefined) => {
    if (!id || !id.trim()) return null;
    return equipments.find(e => e.ID?.toString() === id) || null;
  };

  const getFormattedAttributes = (equip: any) => {
    const attrs: { label: string; color?: string }[] = [];
    if (equip.attributes && Array.isArray(equip.attributes)) {
      equip.attributes.forEach((attr: any) => {
        if (attr.type === 'atSkillEventHandler') {
          attrs.push({ label: '特效', color: '#ffcc00' });
          return;
        }

        let name = equip.AttributeTypes?.[attr.type];
        if (!name && attr.label) {
          const match = attr.label.match(/^([^\s0-9]+)/);
          if (match) name = match[1];
        }
        if (name) {
          name = name.replace(/等级$|值$/, '').replace(/^外功|^内功/, '');
          if (name === '会心效果') name = '会效';
          if (name === '治疗成效') name = '治疗';
          attrs.push({
            label: name,
            color: (attr.color && attr.color.toLowerCase() !== '#ffffff' && attr.color.toLowerCase() !== 'white')
              ? attr.color : '#00bcd4'
          });
        }
      });
    }
    return attrs.slice(0, 4);
  };

  const safeRecords = Array.isArray(records) ? records : [];
  const safeTrialRecords = Array.isArray(trialRecords) ? trialRecords : [];
  const safeAccounts = Array.isArray(accounts) ? accounts : [];

  const findRoleInfo = (accountId: string, roleId: string): { roleName: string; server: string } => {
    for (const account of safeAccounts) {
      if (account.id === accountId) {
        const roles = account.roles || [];
        const role = roles.find((r: any) => r.id === roleId);
        if (role) {
          return {
            roleName: role.name,
            server: `${role.region} ${role.server}`
          };
        }
      }
    }
    return { roleName: '', server: '' };
  };

  // 合并所有稀有掉落记录：副本玄晶 + 试炼可交易装备
  const allDropRecords = useMemo(() => {
    const raidDrops = safeRecords
      .filter(r => r.hasXuanjing === true)
      .map(r => ({ id: r.id, date: r.date, raidName: r.raidName, notes: r.notes, type: '玄晶', roleId: r.roleId || r.accountId, accountId: r.accountId, roleName: r.roleName, server: r.server }));

    const trialDrops = safeTrialRecords
      .filter(r => {
        const equipId = (r as any)[`card${r.flippedIndex}`];
        if (!equipId) return false;
        const equip = findEquipmentById(equipId);
        return equip && (equip.BindType === 1 || equip.BindType === 2); // 1 = 不绑定, 2 = 装绑
      })
      .map(r => {
        const equip = findEquipmentById((r as any)[`card${r.flippedIndex}`]);
        return {
          id: r.id,
          date: r.date,
          raidName: `试炼之地 - 第 ${r.layer} 层`,
          notes: r.notes,
          equip: equip,
          type: equip ? (equip.BindType === 1 ? '不绑定' : '装绑') : '装备',
          roleId: r.roleId || r.accountId,
          accountId: r.accountId,
          roleName: r.roleName,
          server: r.server
        };
      });

    return [...raidDrops, ...trialDrops];
  }, [safeRecords, safeTrialRecords, equipments]);

  const roleStats = useMemo<CrystalRoleStats[]>(() => {
    const roleMap = new Map<string, CrystalRoleStats>();

    allDropRecords.forEach(record => {
      const roleId = record.roleId;

      if (!roleMap.has(roleId)) {
        const roleInfo = findRoleInfo(record.accountId, roleId);
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

    return Array.from(roleMap.values()).sort((a, b) => b.totalCount - a.totalCount);
  }, [allDropRecords]);

  const toggleExpand = (roleId: string) => {
    setExpandedRoleId(expandedRoleId === roleId ? null : roleId);
  };

  const totalDrops = allDropRecords.length;
  const xuanjingTotal = allDropRecords.filter(r => r.type === '玄晶').length;
  const equipTotal = allDropRecords.filter(r => r.type !== '玄晶').length;
  const totalRoles = roleStats.length;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <button
          onClick={onBack}
          className="p-2 hover:bg-base rounded-lg transition-colors"
        >
          <ArrowLeft className="w-5 h-5 text-muted" />
        </button>
        <div>
          <h2 className="text-2xl font-bold text-main">稀有掉落统计</h2>
          <p className="text-sm text-muted mt-1">
            共获取 {totalDrops} 次稀有掉落，来自 {totalRoles} 个角色
          </p>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-gradient-to-br from-violet-50/50 to-amber-50/50 dark:from-violet-900/10 dark:to-amber-900/10 rounded-xl p-5 shadow-sm border border-violet-100/50 dark:border-violet-800/20">
          <div className="flex items-center gap-3 mb-3">
            <span className="text-violet-700 dark:text-violet-300 font-medium text-sm">玄晶总数</span>
          </div>
          <p className="text-3xl font-bold text-violet-900 dark:text-violet-100">{xuanjingTotal}</p>
        </div>
        <div className="bg-gradient-to-br from-emerald-50/50 to-teal-50/50 dark:from-emerald-900/10 dark:to-teal-900/10 rounded-xl p-5 shadow-sm border border-emerald-100/50 dark:border-emerald-800/20">
          <div className="flex items-center gap-3 mb-3">
            <span className="text-emerald-700 dark:text-emerald-300 font-medium text-sm">试炼可交易装备数</span>
          </div>
          <p className="text-3xl font-bold text-emerald-900 dark:text-emerald-100">{equipTotal}</p>
        </div>
        <div className="bg-surface rounded-xl p-5 shadow-sm border border-base">
          <div className="flex items-center gap-3 mb-3">
            <span className="text-muted font-medium text-sm">涉及角色</span>
          </div>
          <p className="text-3xl font-bold text-main">{totalRoles}</p>
        </div>
      </div>

      <div className="bg-surface rounded-xl shadow-sm border border-base">
        {roleStats.length === 0 ? (
          <div className="text-center py-12">
            <div className="w-12 h-12 rounded-full bg-slate-100 dark:bg-slate-800 flex items-center justify-center mx-auto mb-3">
              <span className="text-slate-400 dark:text-slate-500 text-xl font-bold">0</span>
            </div>
            <p className="text-muted">暂无稀有掉落记录</p>
            <p className="text-sm text-muted/70 mt-1">通关副本有玄晶奖励或试炼翻牌出可交易装备后会自动记录</p>
            {allDropRecords.length === 0 && safeRecords.length > 0 && (
              <p className="text-xs text-muted/50 mt-2">共有 {safeRecords.length} 条副本记录，暂无稀有掉落</p>
            )}
          </div>
        ) : (
          <div className="space-y-1">
            {roleStats.map((stat, index) => {
              console.log('[CrystalDetail] Rendering role stat:', stat.roleName, 'count:', stat.totalCount, 'records:', stat.records.length);
              return (
                <div key={stat.roleId} className="border-b border-base last:border-b-0">
                  <button
                    onClick={() => toggleExpand(stat.roleId)}
                    className="w-full flex items-center gap-4 p-4 hover:bg-base/50 transition-colors text-left"
                  >
                    <div className="flex items-center justify-center w-8 h-8 bg-base rounded-lg font-bold text-sm text-main">
                      {index + 1}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <h4 className="font-semibold text-main truncate">{stat.roleName}</h4>
                        {index === 0 && stat.totalCount > 0 && (
                          <span className="px-2 py-0.5 bg-primary/10 text-primary text-xs rounded-full">榜首</span>
                        )}
                      </div>
                      <p className="text-sm text-muted truncate">{stat.server}</p>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-sm text-muted mr-1">掉落</span>
                      <span className="text-lg font-bold text-main">{stat.totalCount}</span>
                      <span className="text-xs text-muted">次</span>
                      <svg
                        className={`w-4 h-4 text-muted transition-transform duration-200 ${expandedRoleId === stat.roleId ? 'rotate-180' : ''}`}
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                      </svg>
                    </div>
                  </button>

                  {expandedRoleId === stat.roleId && (
                    <div className="bg-surface p-4 border-t border-base">
                      <div className="space-y-3">
                        {stat.records.map((record, recordIndex) => (
                          <div key={`${record.id || record.date}-${recordIndex}`} className="flex items-center gap-4 p-3 border border-base rounded-lg">
                            <div className="flex-shrink-0 text-sm text-muted">
                              {new Date(record.date).toLocaleString('zh-CN', {
                                year: 'numeric',
                                month: '2-digit',
                                day: '2-digit',
                                hour: '2-digit',
                                minute: '2-digit'
                              })}
                            </div>
                            <div className="flex-1">
                              <p className="font-medium text-main">{record.raidName}</p>
                              {record.equip ? (
                                <div className="flex flex-wrap items-center gap-x-2 gap-y-1 mt-0.5 text-xs">
                                  <span className="text-muted">{record.equip.Level}品</span>
                                  <span className="font-bold text-[#fe2dfe]" style={{ textShadow: '0 0 1px rgba(254, 45, 254, 0.2)' }}>
                                    {record.equip.Name}
                                  </span>
                                  <div className="flex items-center gap-x-1.5 ml-1">
                                    {getFormattedAttributes(record.equip).map((attr, idx) => (
                                      <span key={idx} style={{ color: attr.color }}>{attr.label}</span>
                                    ))}
                                  </div>
                                </div>
                              ) : (
                                <p className="text-xs text-muted">{record.notes || ''}</p>
                              )}
                            </div>
                            <div className={`flex-shrink-0 px-2 py-0.5 rounded text-[11px] font-medium border ${record.type === '玄晶'
                              ? 'bg-violet-50 text-violet-600 border-violet-200 dark:bg-violet-900/20 dark:text-violet-300 dark:border-violet-800/30'
                              : 'bg-emerald-50 text-emerald-600 border-emerald-200 dark:bg-emerald-900/20 dark:text-emerald-300 dark:border-emerald-800/30'
                              }`}>
                              {record.type}
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
