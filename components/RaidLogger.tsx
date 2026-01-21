import React, { useState } from 'react';
import { RaidRecord, Account, Raid } from '../types';
import { Search, Filter, Trash2 } from 'lucide-react';
import { generateUUID } from '../utils/uuid';
import { getRaidKey } from '../utils/raidUtils';
import { toast } from '../utils/toastManager';

// 添加全局错误捕获
window.onerror = function (message, source, lineno, colno, error) {
  console.error('全局错误:', { message, source, lineno, colno, error });
  toast.error(`发生错误: ${message}`);
  return false;
};

// 扩展RaidRecord，添加角色信息
interface ExtendedRaidRecord extends RaidRecord {
  roleName?: string;
  server?: string;
}

// 扁平化角色列表，方便选择
const flattenRoles = (accounts: Account[]): { id: string; name: string; server: string; accountId: string }[] => {
  // 确保accounts是数组
  const safeAccounts = Array.isArray(accounts) ? accounts : [];

  return safeAccounts.flatMap(account => {
    // 过滤掉禁用的账号
    if (account.disabled) return [];

    // 确保account.roles是数组
    const safeRoles = Array.isArray(account.roles) ? account.roles : [];

    // 过滤掉禁用的角色
    return safeRoles.filter(role => !role.disabled).map(role => ({
      id: `${account.id}-${role.id}`,
      name: role.name,
      server: `${role.region} ${role.server}`,
      accountId: account.id
    }));
  });
};

interface RaidLoggerProps {
  accounts: Account[];
  records: RaidRecord[];
  setRecords: React.Dispatch<React.SetStateAction<RaidRecord[]>>;
  raids: Raid[];
}

export const RaidLogger: React.FC<RaidLoggerProps> = ({ accounts, records, setRecords, raids }) => {
  // 确保所有传入的数组都是安全的
  const safeAccounts = Array.isArray(accounts) ? accounts : [];
  const safeRecords = Array.isArray(records) ? records : [];
  const safeRaids = Array.isArray(raids) ? raids : [];

  const roles = flattenRoles(safeAccounts);

  // Filter State
  const [filterRole, setFilterRole] = useState('ALL');

  // 自定义资料片排序顺序映射（逆序）
  const versionOrderMap: Record<string, number> = {
    "丝路风雨": 0,
    "横刀断浪": 1,
    "奉天证道": 2,
    "世外蓬莱": 3,
    "重制版": 4,
    "风骨霸刀": 5,
    "剑胆琴心": 6,
    "安史之乱": 7,
    "巴蜀风云": 8,
    "风起稻香": 9
  };

  const activeRaids = safeRaids.filter(r => r.isActive).sort((a, b) => {
    // 1. 按人数排序（25人优先）
    const playerCountOrder = { 25: 0, 10: 1 };
    if (playerCountOrder[a.playerCount] !== playerCountOrder[b.playerCount]) {
      return playerCountOrder[a.playerCount] - playerCountOrder[b.playerCount];
    }

    // 2. 按难度排序（挑战 → 英雄 → 普通）
    const difficultyOrder = { 'CHALLENGE': 0, 'HEROIC': 1, 'NORMAL': 2 };
    if (difficultyOrder[a.difficulty] !== difficultyOrder[b.difficulty]) {
      return difficultyOrder[a.difficulty] - difficultyOrder[b.difficulty];
    }

    // 3. 按资料片排序（保持现有逻辑）
    const orderA = a.version ? versionOrderMap[a.version] || 999 : 999;
    const orderB = b.version ? versionOrderMap[b.version] || 999 : 999;
    if (orderA !== orderB) {
      return orderA - orderB;
    }

    // 4. 按副本名称逆序排列
    return b.name.localeCompare(a.name);
  });

  // New Record State
  const [selectedRole, setSelectedRole] = useState<string>(roles[0]?.id || '');
  const [selectedRaidKey, setSelectedRaidKey] = useState<string>(activeRaids[0] ? getRaidKey(activeRaids[0]) : '');
  const [goldIncome, setGoldIncome] = useState<number>(0);
  const [hasXuanjing, setHasXuanjing] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Batch Deletion State
  const [selectedRecords, setSelectedRecords] = useState<Set<string>>(new Set());

  // Confirmation Dialog State
  const [showBatchDeleteConfirm, setShowBatchDeleteConfirm] = useState(false);
  const [recordToDelete, setRecordToDelete] = useState<string | null>(null);

  // Toggle record selection
  const toggleRecordSelection = (recordId: string) => {
    setSelectedRecords(prev => {
      const newSet = new Set(prev);
      if (newSet.has(recordId)) {
        newSet.delete(recordId);
      } else {
        newSet.add(recordId);
      }
      return newSet;
    });
  };

  // Toggle all records selection
  const toggleSelectAll = () => {
    if (selectedRecords.size === filteredRecords.length) {
      setSelectedRecords(new Set());
    } else {
      setSelectedRecords(new Set(filteredRecords.map(record => record.id)));
    }
  };

  // Handle batch delete click - show confirm dialog
  const handleBatchDeleteClick = () => {
    if (selectedRecords.size === 0) return;
    setShowBatchDeleteConfirm(true);
  };

  // Handle batch delete confirmation - execute delete
  const handleBatchDeleteConfirm = () => {
    setRecords(prev => prev.filter(r => !selectedRecords.has(r.id)));
    setSelectedRecords(new Set());
    setShowBatchDeleteConfirm(false);
  };

  // Handle batch delete cancellation - close dialog
  const handleBatchDeleteCancel = () => {
    setShowBatchDeleteConfirm(false);
  };

  // Handle single delete confirmation - execute delete
  const handleSingleDeleteConfirm = () => {
    if (recordToDelete) {
      setRecords(prev => prev.filter(r => r.id !== recordToDelete));
      setRecordToDelete(null);
    }
  };

  // Handle single delete cancellation - close dialog
  const handleSingleDeleteCancel = () => {
    setRecordToDelete(null);
  };

  // 从扁平化角色ID中解析出账号ID和角色ID
  const parseRoleId = (roleId: string): { accountId: string; roleId: string } => {
    // 角色ID格式: accountId-roleId (accountId是完整UUID，roleId是完整UUID)
    // 找到第二个UUID的开始位置 (跳过第一个UUID的8-4-4-4-12格式)
    const uuidPattern = /^([a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})-(.+)$/;
    const match = roleId.match(uuidPattern);

    if (match) {
      return {
        accountId: match[1],
        roleId: match[2]
      };
    }

    // 兼容处理: 使用最后一个-作为分隔符
    const lastDashIndex = roleId.lastIndexOf('-');
    if (lastDashIndex === -1) {
      return { accountId: roleId, roleId: '' };
    }

    return {
      accountId: roleId.substring(0, lastDashIndex),
      roleId: roleId.substring(lastDashIndex + 1)
    };
  };

  // 构建副本名称
  const constructRaidName = (raid: Raid): string => {
    const difficultyLabel = raid.difficulty === 'NORMAL' ? '普通' :
      raid.difficulty === 'HEROIC' ? '英雄' :
        '挑战'; // 只有普通、英雄、挑战三种难度
    return `${raid.playerCount}人${difficultyLabel}${raid.name}`;
  };

  const handleAddRecord = (e: React.FormEvent) => {
    e.preventDefault();

    console.log('=== 提交记录调试信息 ===');
    console.log('selectedRole:', selectedRole);
    console.log('selectedRaidKey:', selectedRaidKey);
    console.log('goldIncome:', goldIncome);
    console.log('safeAccounts 数量:', safeAccounts.length);
    console.log('safeAccounts:', safeAccounts.map(a => ({ id: a.id, name: a.accountName, rolesCount: a.roles?.length || 0 })));
    console.log('roles 数量:', roles.length);
    console.log('roles:', roles);

    // 表单验证
    if (!selectedRole) {
      alert('请先选择角色');
      return;
    }

    if (!selectedRaidKey) {
      alert('请先选择副本');
      return;
    }

    if (!goldIncome || goldIncome <= 0) {
      alert('请输入有效的工资收入');
      return;
    }

    setIsSubmitting(true);

    try {
      const raid = safeRaids.find(r => getRaidKey(r) === selectedRaidKey);
      if (!raid) {
        alert('未找到选择的副本');
        return;
      }

      const { accountId, roleId } = parseRoleId(selectedRole);
      console.log('解析结果 - accountId:', accountId, 'roleId:', roleId);
      console.log('accountId 类型:', typeof accountId, '长度:', accountId.length);

      // 提前构建副本名称，确保两个路径都能使用
      const raidName = constructRaidName(raid);

      const account = safeAccounts.find(a => {
        console.log('比较:', a.id, '===', accountId, '结果:', a.id === accountId);
        return a.id === accountId;
      });

      if (!account) {
        console.log('使用 roles 数组查找账号');
        const selectedRoleObj = roles.find(r => r.id === selectedRole);
        console.log('找到的角色对象:', selectedRoleObj);
        if (selectedRoleObj) {
          const accountFromRoles = safeAccounts.find(a => a.id === selectedRoleObj.accountId);
          console.log('从 roles 找到的账号:', accountFromRoles ? '找到' : '未找到');

          if (accountFromRoles) {
            const record: RaidRecord = {
              id: generateUUID(),
              accountId: accountFromRoles.id,
              roleId: roleId,
              raidName: raidName,
              date: new Date().toISOString(),
              goldIncome: Number(goldIncome),
              hasXuanjing
            };

            setRecords(prev => [record, ...prev]);
            setGoldIncome(0);
            setHasXuanjing(false);

            toast.success(`成功添加记录: ${raidName}`);
            return;
          }
        }
      }

      console.log('查找账号结果:', account ? '找到' : '未找到');
      console.log('所有账号ID:', safeAccounts.map(a => a.id));

      if (!account) {
        alert(`未找到选择的账号，账号ID: ${accountId}\n\n所有可用账号ID:\n${safeAccounts.map(a => a.id).join('\n')}`);
        return;
      }

      // 获取角色信息，用于保存到记录中（确保历史记录不可变）
      const safeRoles = Array.isArray(account.roles) ? account.roles : [];
      const role = safeRoles.find(r => r.id === roleId);
      const roleName = role?.name || '未知';
      const server = role ? `${role.region} ${role.server}` : '未知';

      const record: RaidRecord = {
        id: generateUUID(),
        accountId: accountId,
        roleId: roleId,
        raidName,
        date: new Date().toISOString(),
        goldIncome: Number(goldIncome),
        hasXuanjing,
        roleName, // 保存角色名称（不可变）
        server // 保存服务器信息（不可变）
      };

      setRecords(prev => [record, ...prev]);
      setGoldIncome(0);
      setHasXuanjing(false);

      toast.success(`成功添加记录: ${raidName}`);

    } catch (error) {
      console.error('添加记录失败:', error);
      alert(`添加记录失败: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setIsSubmitting(false);
    }
  };

  // 获取角色名称和门派
  const getRoleInfo = (accountId: string, roleId: string) => {
    console.log('=== 获取角色信息 ===');
    console.log('accountId:', accountId, 'roleId:', roleId);

    const account = safeAccounts.find(a => a.id === accountId);
    console.log('找到的账号:', account ? '找到' : '未找到');

    if (!account) return { name: '未知', server: '未知' };

    const safeRoles = Array.isArray(account.roles) ? account.roles : [];
    console.log('账号角色数量:', safeRoles.length);
    console.log('账号角色列表:', safeRoles.map(r => ({ id: r.id, name: r.name })));

    let role = safeRoles.find(r => r.id === roleId);

    if (!role) {
      console.log('未找到匹配的角色，使用第一个角色');
      role = safeRoles[0];
    }

    // 注意：这里不再检查 role.disabled，因为历史记录应该显示创建时的原始信息
    // 如果记录中保存了 roleName 和 server，会优先使用记录中的信息
    // 只有当记录中没有这些信息时，才会从当前账号中查找

    console.log('最终使用的角色:', role ? role.name : '未知');

    return {
      roleName: role?.name || '未知',
      server: role ? `${role.region} ${role.server}` : '未知'
    };
  };

  // 扩展记录，添加角色信息
  const extendedRecords: ExtendedRaidRecord[] = safeRecords.map(record => {
    // 优先使用记录中保存的角色名称和服务器信息（确保历史记录不可变）
    if (record.roleName && record.server) {
      return {
        ...record,
        roleName: record.roleName,
        server: record.server
      };
    }
    // 如果记录中没有保存角色信息，则从当前账号中查找
    return {
      ...record,
      ...getRoleInfo(record.accountId, record.roleId)
    };
  });

  const filteredRecords = extendedRecords.filter(record => {
    if (filterRole === 'ALL') return true;
    // 筛选器的值是完整角色ID，需要提取账号ID和角色ID进行比较
    const { accountId: filterAccountId, roleId: filterRoleId } = parseRoleId(filterRole);
    return record.accountId === filterAccountId && record.roleId === filterRoleId;
  });

  return (
    <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
      {/* Input Form */}
      <div className="lg:col-span-1">
        <div className="bg-surface p-6 rounded-xl shadow-sm border border-base sticky top-6">
          <h2 className="text-xl font-bold text-main mb-4">记录副本</h2>
          <form onSubmit={handleAddRecord} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-main mb-1">选择角色</label>
              <select
                className="w-full p-2 bg-surface text-main border border-base rounded-lg"
                value={selectedRole}
                onChange={e => setSelectedRole(e.target.value)}
              >
                {roles.length === 0 ? (
                  <option value="">暂无角色，请先添加角色</option>
                ) : (
                  roles.map(role => (
                    <option key={role.id} value={role.id}>{role.name} ({role.server})</option>
                  ))
                )}
              </select>
            </div>

            <div>
              <label className="block text-sm font-medium text-main mb-1">副本名称</label>
              <select
                className="w-full p-2 bg-surface text-main border border-base rounded-lg max-h-60 overflow-y-auto"
                value={selectedRaidKey}
                onChange={e => setSelectedRaidKey(e.target.value)}
              >
                {activeRaids.map(r => {
                  // 所有副本使用统一的命名格式
                  const difficultyLabel = r.difficulty === 'NORMAL' ? '普通' :
                    r.difficulty === 'HEROIC' ? '英雄' :
                      '挑战'; // 只有普通、英雄、挑战三种难度
                  const displayText = `${r.playerCount}人${difficultyLabel}${r.name}`;
                  const raidKey = getRaidKey(r);
                  return (
                    <option key={raidKey} value={raidKey}>{displayText}</option>
                  );
                })}
              </select>
            </div>

            <div>
              <label className="block text-sm font-medium text-main mb-1">工资收入 (金)</label>
              <input
                type="number"
                min="0"
                className="w-full p-2 bg-surface text-main border border-base rounded-lg font-mono placeholder:text-muted"
                value={goldIncome}
                onChange={e => setGoldIncome(Number(e.target.value))}
                placeholder="例如: 20000"
              />
            </div>

            <div className="flex items-center gap-4 py-2">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={hasXuanjing}
                  onChange={e => setHasXuanjing(e.target.checked)}
                  className="w-4 h-4 text-orange-500 rounded border-slate-300 focus:ring-orange-500"
                />
                <span className="text-sm text-bold text-orange-600">出玄晶了!</span>
              </label>
            </div>

            <button
              type="submit"
              className="w-full py-2.5 bg-primary text-white font-medium rounded-lg hover:bg-primary-hover shadow-md shadow-primary/20 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
              disabled={isSubmitting || !goldIncome || goldIncome <= 0}
            >
              {isSubmitting ? '提交中...' : '提交记录'}
            </button>
          </form>
        </div>
      </div>

      {/* History List */}
      <div className="lg:col-span-2 space-y-4">
        <div className="flex justify-between items-center bg-surface p-4 rounded-xl shadow-sm border border-base">
          <h3 className="font-semibold text-main flex items-center gap-2">
            <Filter size={18} /> 历史记录
          </h3>
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={selectedRecords.size === filteredRecords.length && filteredRecords.length > 0}
                onChange={toggleSelectAll}
                className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
              />
              <span className="text-sm text-muted">全选</span>
            </div>
            <button
              onClick={handleBatchDeleteClick}
              className="px-3 py-1.5 bg-red-600 text-white text-sm rounded-lg hover:bg-red-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              disabled={selectedRecords.size === 0}
            >
              批量删除 ({selectedRecords.size})
            </button>
            <div className="flex items-center gap-2">
              <span className="text-sm text-muted hidden sm:inline">筛选:</span>
              <select
                className="p-1.5 border border-base rounded text-sm bg-base text-main"
                value={filterRole}
                onChange={e => setFilterRole(e.target.value)}
              >
                <option value="ALL">所有角色</option>
                {roles.map(role => (
                  <option key={role.id} value={role.id}>{role.name} ({role.server})</option>
                ))}
              </select>
            </div>
          </div>
        </div>

        <div className="space-y-3">
          {filteredRecords.length === 0 ? (
            <div className="text-center py-12 text-muted">
              <Search className="w-12 h-12 mx-auto mb-2 opacity-20" />
              <p>暂无记录，快去打本吧！</p>
            </div>
          ) : (
            filteredRecords.map(record => {
              const handleDeleteClick = () => {
                setRecordToDelete(record.id);
              };
              return (
                <div key={record.id} className={`bg-surface p-4 rounded-xl shadow-sm border-l-4 flex items-start justify-between ${record.hasXuanjing ? 'border-l-amber-500 bg-amber-50/30 dark:bg-amber-900/10' : 'border-l-emerald-500'}`}>
                  <div className="flex gap-3">
                    <input
                      type="checkbox"
                      checked={selectedRecords.has(record.id)}
                      onChange={() => toggleRecordSelection(record.id)}
                      className="w-4 h-4 text-primary rounded border-base focus:ring-primary mt-1 flex-shrink-0"
                    />
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="font-bold text-main">{record.roleName} ({record.server})</span>
                        <span className="text-xs text-muted">
                          {new Date(record.date).toLocaleDateString()}
                        </span>
                        {record.hasXuanjing && (
                          <span className="px-2 py-0.5 bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-400 text-[10px] font-bold rounded-full border border-amber-200 dark:border-amber-700">
                            玄晶
                          </span>
                        )}
                      </div>
                      <div className="text-sm text-muted mt-1">
                        {record.raidName}
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-4">
                    <div className="text-right">
                      <div className="text-lg font-bold text-emerald-600 dark:text-emerald-400 font-mono">
                        +{record.goldIncome.toLocaleString()}
                      </div>
                      <div className="text-xs text-muted">金币</div>
                    </div>
                    <button
                      onClick={handleDeleteClick}
                      className="text-muted hover:text-amber-500 transition-colors p-1.5 rounded-full hover:bg-amber-50 dark:hover:bg-amber-900/20"
                      title="删除记录"
                    >
                      <Trash2 size={16} />
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* Batch Delete Confirmation Dialog */}
      {showBatchDeleteConfirm && (
        <div className="fixed inset-0 bg-slate-900/60 flex items-center justify-center z-50">
          <div className="bg-surface p-6 rounded-xl shadow-lg max-w-sm w-full mx-4">
            <h3 className="text-lg font-semibold text-main mb-4">确认删除</h3>
            <p className="text-muted mb-6">确定要删除选中的 {selectedRecords.size} 条记录吗？</p>
            <div className="flex justify-end gap-3">
              <button
                onClick={handleBatchDeleteCancel}
                className="px-4 py-2 text-main border border-base rounded-lg hover:bg-base transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleBatchDeleteConfirm}
                className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors"
              >
                确定
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Single Delete Confirmation Dialog */}
      {recordToDelete && (
        <div className="fixed inset-0 bg-slate-900/60 flex items-center justify-center z-50">
          <div className="bg-surface p-6 rounded-xl shadow-lg max-w-sm w-full mx-4">
            <h3 className="text-lg font-semibold text-main mb-4">确认删除</h3>
            <p className="text-muted mb-6">确定要删除这条记录吗？</p>
            <div className="flex justify-end gap-3">
              <button
                onClick={handleSingleDeleteCancel}
                className="px-4 py-2 text-main border border-base rounded-lg hover:bg-base transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleSingleDeleteConfirm}
                className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors"
              >
                确定
              </button>
            </div>
          </div>
        </div>
      )}

    </div>
  );
};