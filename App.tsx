import React, { useState, useEffect, useRef, useCallback } from 'react';
import { LayoutDashboard, Users, Download, Shield, Settings, Sun, Moon } from 'lucide-react';
import { useTheme } from './contexts/ThemeContext';
import { Dashboard } from './components/Dashboard';
import { IncomeDetail } from './components/IncomeDetail';
import { AccountManager } from './components/AccountManager';
import { RaidManager } from './components/RaidManager';

import { CrystalDetail } from './components/CrystalDetail';


import { ToastContainer } from './components/ToastContainer';
import { ConfigManager } from './components/ConfigManager';
import { LoadingSpinner } from './components/LoadingSpinner';
import { AddRecordModal } from './components/AddRecordModal';
import { AddBaizhanRecordModal } from './components/AddBaizhanRecordModal';
import { Account, RaidRecord, Raid, Config, TrialPlaceRecord, BaizhanRecord, InstanceType, RoleInstanceVisibility } from './types';
import {
  DEFAULT_CONFIG,
  loadConfigFromStorage,
  saveConfigToStorage,
  validateConfig,
  getConfigSummary
} from './utils/configUtils';
import { saveRaidCache } from './utils/raidUtils';
import { injectDefaultBossesForRaids } from './data/raidBosses';
import { sortAccounts } from './utils/accountUtils';
import { db } from './services/db';
import { checkLocalStorageData, migrateLocalStorageData } from './services/migration';
import { syncEquipment } from './services/jx3BoxApi';

function App() {
  const [activeTab, setActiveTab] = useState<'dashboard' | 'accounts' | 'raidManager' | 'config'>('dashboard');
  const [showIncomeDetail, setShowIncomeDetail] = useState(false);
  const [showCrystalDetail, setShowCrystalDetail] = useState(false);

  const [deferredPrompt, setDeferredPrompt] = useState<any>(null);
  const [isTransitioning, setIsTransitioning] = useState(false);
  const [contentKey, setContentKey] = useState(0);
  const [isInitialized, setIsInitialized] = useState(false);
  const previousTabRef = useRef<string>('dashboard');

  const { theme, toggleTheme } = useTheme();

  const [accounts, setAccounts] = useState<Account[]>([]);
  const [records, setRecords] = useState<RaidRecord[]>([]);
  const [trialRecords, setTrialRecords] = useState<TrialPlaceRecord[]>([]);
  const [baizhanRecords, setBaizhanRecords] = useState<BaizhanRecord[]>([]);
  const [raids, setRaids] = useState<Raid[]>([]);
  const [config, setConfig] = useState<Config>(DEFAULT_CONFIG);
  const [editingRecord, setEditingRecord] = useState<RaidRecord | null>(null);
  const [editingBaizhanRecord, setEditingBaizhanRecord] = useState<BaizhanRecord | null>(null);
  const [instanceTypes, setInstanceTypes] = useState<InstanceType[]>([]);

  // 重新从数据库加载记录
  const reloadRecords = useCallback(async () => {
    try {
      const loadedRecords = await db.getRecords();
      console.log(`重新加载记录: ${loadedRecords.length} 条`);
      setRecords(loadedRecords);
    } catch (error) {
      console.error('重新加载记录失败:', error);
    }
  }, []);

  // 重新从数据库加载试炼记录
  const reloadTrialRecords = useCallback(async () => {
    try {
      const loadedTrialRecords = await db.getTrialRecords();
      console.log(`重新加载试炼记录: ${loadedTrialRecords.length} 条`);
      setTrialRecords(loadedTrialRecords);
    } catch (error) {
      console.error('重新加载试炼记录失败:', error);
    }
  }, []);

  // 重新从数据库加载百战记录
  const reloadBaizhanRecords = useCallback(async () => {
    try {
      const loadedBaizhanRecords = await db.getBaizhanRecords();
      console.log(`重新加载百战记录: ${loadedBaizhanRecords.length} 条`);
      setBaizhanRecords(loadedBaizhanRecords);
    } catch (error) {
      console.error('重新加载百战记录失败:', error);
    }
  }, []);

  useEffect(() => {
    const initApp = async () => {
      try {
        console.log('正在初始化数据库...');
        await db.init();
        console.log('数据库初始化完成');

        // 获取版本信息（用于调试）
        const versionInfo = await db.getVersionInfo();
        if (versionInfo) {
          console.log(`数据库版本: V${versionInfo.schemaVersion}, 最新版本: V${versionInfo.currentVersion}`);
          console.log(`localStorage 迁移状态: ${versionInfo.localStorageMigrated ? '已完成' : '未完成'}`);
        }

        // 检查是否需要执行 localStorage 迁移
        const localData = await checkLocalStorageData();
        if (localData.totalItems > 0) {
          console.log(`发现 localStorage 数据，开始迁移...`);
          console.log(`  账号: ${localData.accountsCount}`);
          console.log(`  记录: ${localData.recordsCount}`);
          console.log(`  副本: ${localData.raidsCount}`);

          const result = await migrateLocalStorageData();

          if (result.success) {
            console.log('✓ localStorage 迁移成功！');
            // 输出详细的迁移日志
            if (result.details && result.details.length > 0) {
              result.details.forEach((detail: string) => console.log(`  - ${detail}`));
            }
          } else {
            console.error('✗ localStorage 迁移失败:', result.message);
            if (result.details && result.details.length > 0) {
              result.details.forEach((detail: string) => console.error(`  - ${detail}`));
            }
          }
        } else {
          console.log('无需迁移 localStorage，无本地数据');
        }

        console.log('\n正在加载数据库数据...');
        const [loadedAccounts, loadedRecords, loadedRaids, loadedConfig, loadedTrialRecords, loadedBaizhanRecords, loadedInstanceTypes, loadedRoleVisibility] = await Promise.all([
          db.getAccounts(),
          db.getRecords(),
          db.getRaids(),
          db.getConfig(),
          db.getTrialRecords(),
          db.getBaizhanRecords(),
          db.getInstanceTypes(),
          db.getAllRoleVisibility()
        ]);

        console.log(`加载完成: 账号 ${loadedAccounts.length}, 记录 ${loadedRecords.length}, 副本 ${loadedRaids.length}, 试炼 ${loadedTrialRecords.length}, 百战 ${loadedBaizhanRecords.length}`);

        // 保存副本类型
        setInstanceTypes(loadedInstanceTypes);

        const parsedAccounts = loadedAccounts;
        const parsedRecords = loadedRecords;
        const parsedRaids = loadedRaids;

        if (parsedAccounts.length > 0) {
          // 合并可见性数据到角色
          const accountsWithVisibility = parsedAccounts.map(acc => ({
            ...acc,
            roles: acc.roles.map((role: any) => ({
              ...role,
              visibility: buildVisibilityMap(role.id, loadedRoleVisibility, loadedInstanceTypes)
            }))
          }));
          const sortedAccounts = sortAccounts(accountsWithVisibility);
          setAccounts(sortedAccounts);
          console.log(`设置账号: ${sortedAccounts.length} 个`);
        } else {
          console.log('没有账号数据');
        }

        // 自动为已知副本注入默认 BOSS 配置
        const raidsWithBosses = injectDefaultBossesForRaids(parsedRaids);
        saveRaidCache(raidsWithBosses);
        setRaids(raidsWithBosses);

        if (loadedConfig) {
          // 数据库有配置，使用它
          const configData = typeof loadedConfig === 'string' ? JSON.parse(loadedConfig) : loadedConfig;
          setConfig(configData);
          console.log('使用数据库配置');
        }
        // 如果数据库没有配置，保持使用 DEFAULT_CONFIG（不写入数据库）
        // config 迁移已在 migrateLocalStorageData() 中完成

        if (parsedRecords.length > 0) {
          // Filter out any legacy trial records if they exist in standard records
          // @ts-ignore
          const raidRecords = parsedRecords.filter((r: any) => r.type !== 'trial') as RaidRecord[];
          setRecords(raidRecords);
        }

        // Set trial records from the dedicated table source
        if (loadedTrialRecords && loadedTrialRecords.length > 0) {
          setTrialRecords(loadedTrialRecords as TrialPlaceRecord[]);
        }

        // Set baizhan records from the dedicated table source
        if (loadedBaizhanRecords && loadedBaizhanRecords.length > 0) {
          setBaizhanRecords(loadedBaizhanRecords as BaizhanRecord[]);
        }

        setIsInitialized(true);
        console.log('\n✓ 应用初始化完成');

        // Start background sync
        syncEquipment().catch(console.error);
      } catch (error) {
        console.error('初始化失败:', error);
        setIsInitialized(true);
      }
    };

    initApp();
  }, []);

  useEffect(() => {
    if (!isInitialized) return;

    const saveData = async () => {
      try {
        await db.saveAccounts(accounts);
      } catch (error) {
        console.error('保存账号失败:', error);
      }
    };
    saveData();
  }, [accounts, isInitialized]);

  useEffect(() => {
    if (!isInitialized) return;

    const saveData = async () => {
      try {
        // Only save raid records to the legacy 'records' table
        // Trial records are handled individually via db.addTrialRecord / db.deleteTrialRecord
        await db.saveRecords(records);
      } catch (error) {
        console.error('保存副本记录失败:', error);
      }
    };
    saveData();
  }, [records, isInitialized]); // Removed trialRecords dependency

  useEffect(() => {
    if (!isInitialized) return;

    const saveData = async () => {
      try {
        await db.saveRaids(raids);
        saveRaidCache(raids);
      } catch (error) {
        console.error('保存副本失败:', error);
      }
    };
    saveData();
  }, [raids, isInitialized]);

  useEffect(() => {
    if (!isInitialized) return;

    const saveData = async () => {
      try {
        await db.saveConfig(config);
        saveConfigToStorage(config);
      } catch (error) {
        console.error('保存配置失败:', error);
      }
    };
    saveData();
  }, [config, isInitialized]);

  useEffect(() => {
    const handleStorageChange = async (e: StorageEvent) => {
      if (e.key === 'jx3_config') {
        try {
          const newConfig = e.newValue ? JSON.parse(e.newValue) : DEFAULT_CONFIG;
          const validation = await validateConfig(newConfig);

          if (validation.isValid) {
            setConfig(newConfig);
          }
        } catch (error) {
          console.error('同步配置失败:', error);
        }
      }
    };

    window.addEventListener('storage', handleStorageChange);

    return () => {
      window.removeEventListener('storage', handleStorageChange);
    };
  }, []);

  useEffect(() => {
    const handleVisibilityChange = async () => {
      if (document.visibilityState === 'visible') {
        try {
          const currentConfig = loadConfigFromStorage();
          const validation = await validateConfig(currentConfig);

          if (validation.isValid && JSON.stringify(currentConfig) !== JSON.stringify(config)) {
            console.log('检测到配置变更，同步中...', getConfigSummary(currentConfig));
            setConfig(currentConfig);
          }
        } catch (error) {
          console.error('同步配置失败:', error);
        }
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);

    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, []);

  // 处理标签页切换，添加过渡效果
  const handleTabChange = (tab: typeof activeTab) => {
    if (tab !== activeTab) {
      previousTabRef.current = activeTab;
    }

    setIsTransitioning(true);
    setActiveTab(tab);
    setShowIncomeDetail(false);
    setShowCrystalDetail(false);

    setContentKey(prev => prev + 1);

    setTimeout(() => {
      setIsTransitioning(false);
    }, 350);
  };

  // 仅在浏览器环境中添加PWA相关事件监听
  useEffect(() => {
    // 检查是否是Tauri环境
    const isTauri = typeof window !== 'undefined' && window.__tauri__ !== undefined;

    if (!isTauri) {
      const handler = (e: any) => {
        e.preventDefault();
        setDeferredPrompt(e);
      };
      window.addEventListener('beforeinstallprompt', handler);
      return () => window.removeEventListener('beforeinstallprompt', handler);
    }
    return undefined;
  }, []);

  const handleInstall = async () => {
    if (!deferredPrompt) return;
    deferredPrompt.prompt();
    const { outcome } = await deferredPrompt.userChoice;
    if (outcome === 'accepted') {
      setDeferredPrompt(null);
    }
  };

  const handleDeleteRecord = async (recordId: string, isBaizhan?: boolean, isTrial?: boolean) => {
    if (isTrial) {
      try {
        await db.deleteTrialRecord(recordId);
        setTrialRecords(prev => prev.filter(r => r.id !== recordId));
      } catch (error) {
        console.error('删除试炼记录失败:', error);
      }
    } else if (isBaizhan) {
      try {
        await db.deleteBaizhanRecord(recordId);
        setBaizhanRecords(prev => prev.filter(r => r.id !== recordId));
      } catch (error) {
        console.error('删除百战记录失败:', error);
      }
    } else {
      try {
        await db.deleteRecord(recordId);
        await reloadRecords();
      } catch (error) {
        console.error('删除副本记录失败:', error);
      }
    }
  };

  const handleUpdateRecord = async (updatedRecord: Partial<RaidRecord>) => {
    if (!editingRecord) return;

    const mergedRecord = { ...editingRecord, ...updatedRecord } as RaidRecord;

    try {
      await db.addRecord(mergedRecord);
      await reloadRecords();
    } catch (error) {
      console.error('更新副本记录失败:', error);
    }

    setEditingRecord(null);
  };

  if (!isInitialized) {
    return (
      <div className="min-h-screen bg-slate-50 flex items-center justify-center">
        <LoadingSpinner size="lg" text="正在初始化数据库..." />
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-base text-main pb-20 md:pb-0 transition-colors duration-200">
      {/* Sidebar / Topbar */}
      {/* added app-region-drag to allow moving the window, but we must exclude buttons */}
      {/* Sidebar / Topbar */}
      {/* added app-region-drag to allow moving the window, but we must exclude buttons */}
      <nav className="bg-surface/80 backdrop-blur-md border-b border-border sticky top-0 z-50 px-4 md:px-8 h-16 flex items-center justify-between app-region-drag select-none transition-colors duration-200">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-primary rounded-lg flex items-center justify-center text-primary-foreground font-bold text-lg shadow-sm">
            剑
          </div>
          <h1 className="text-xl font-bold tracking-tight text-main">副本管家 <span className="text-xs font-normal text-muted bg-base px-2 py-0.5 rounded-full border border-border">JX3</span></h1>
        </div>
        <div className="flex items-center gap-3 app-region-no-drag">
          <div className="hidden md:flex gap-1">
            <NavButton active={activeTab === 'dashboard'} onClick={() => handleTabChange('dashboard')} icon={<LayoutDashboard size={18} />} label="概览" />
            <NavButton active={activeTab === 'raidManager'} onClick={() => handleTabChange('raidManager')} icon={<Shield size={18} />} label="副本管理" />
            <NavButton active={activeTab === 'accounts'} onClick={() => handleTabChange('accounts')} icon={<Users size={18} />} label="账号管理" />
            <NavButton active={activeTab === 'config'} onClick={() => handleTabChange('config')} icon={<Settings size={18} />} label="配置" />
          </div>
          <button
            onClick={toggleTheme}
            className="p-2 rounded-lg text-muted hover:text-main hover:bg-base transition-colors app-region-no-drag"
            title={theme === 'dark' ? '切换到亮色模式' : '切换到深色模式'}
          >
            {theme === 'dark' ? <Sun size={20} /> : <Moon size={20} />}
          </button>
          {deferredPrompt && (
            <button
              onClick={handleInstall}
              className="hidden md:flex items-center gap-2 px-3 py-1.5 bg-primary text-primary-text text-sm rounded-lg hover:bg-primary-hover transition-colors shadow-sm"
              title="安装到 Windows"
            >
              <Download size={16} />
              <span className="font-medium">安装应用</span>
            </button>
          )}
        </div>
      </nav>

      {/* Main Content */}
      <main key={contentKey} className="w-full mx-auto p-4 md:p-8 select-text">
        {isTransitioning ? (
          <LoadingSpinner size="lg" text="切换中..." />
        ) : (
          <>
            {activeTab === 'dashboard' && (
              showIncomeDetail ? (
                <IncomeDetail
                  key={`incomeDetail-${contentKey}`}
                  records={records}
                  baizhanRecords={baizhanRecords}
                  accounts={accounts}
                  onBack={() => setShowIncomeDetail(false)}
                  onDeleteRecord={handleDeleteRecord}
                  onEditRecord={setEditingRecord}
                  onEditBaizhanRecord={setEditingBaizhanRecord}
                />
              ) : showCrystalDetail ? (
                <CrystalDetail
                  key={`crystalDetail-${contentKey}`}
                  records={records}
                  trialRecords={trialRecords}
                  accounts={accounts}
                  onBack={() => setShowCrystalDetail(false)}
                />
              ) : (
                <Dashboard
                  key={`dashboard-${contentKey}`}
                  records={records}
                  accounts={accounts}
                  baizhanRecords={baizhanRecords}
                  trialRecords={trialRecords}
                  onShowIncomeDetail={() => setShowIncomeDetail(true)}
                  onShowCrystalDetail={() => setShowCrystalDetail(true)}
                />
              )
            )}
            {activeTab === 'accounts' && (
              <AccountManager key={`accounts-${contentKey}`} accounts={accounts} setAccounts={setAccounts} config={config} instanceTypes={instanceTypes} />
            )}
            {activeTab === 'raidManager' && (
              <RaidManager
                key={`raidManager-${contentKey}`}
                raids={raids}
                setRaids={setRaids}
                records={records}
                setRecords={setRecords}
                onEditRecord={setEditingRecord}
                trialRecords={trialRecords}
                baizhanRecords={baizhanRecords}
                accounts={accounts}
                onRefreshRecords={reloadRecords}
                onRefreshTrialRecords={reloadTrialRecords}
                onRefreshBaizhanRecords={reloadBaizhanRecords}
              />
            )}
            {activeTab === 'config' && (
              <ConfigManager key={`config-${contentKey}`} config={config} setConfig={setConfig} />
            )}
          </>
        )}
      </main>

      {/* Mobile Bottom Nav */}
      {/* Mobile Bottom Nav */}
      <div className="md:hidden fixed bottom-0 left-0 right-0 bg-surface border-t border-base h-16 flex items-center justify-around z-20 shadow-[0_-4px_6px_-1px_rgba(0,0,0,0.05)] pb-safe app-region-no-drag">
        <MobileNavButton active={activeTab === 'dashboard'} onClick={() => handleTabChange('dashboard')} icon={<LayoutDashboard size={20} />} label="概览" />
        <MobileNavButton active={activeTab === 'raidManager'} onClick={() => handleTabChange('raidManager')} icon={<Shield size={20} />} label="副本" />
        <MobileNavButton active={activeTab === 'accounts'} onClick={() => handleTabChange('accounts')} icon={<Users size={20} />} label="账号" />
      </div>



      {/* Edit Record Modal */}
      {editingRecord && (() => {
        // 从 raidName 解析副本信息，格式如 "25人英雄弓月城"
        const raidName = editingRecord.raidName;
        let playerCount: 10 | 25 = 25;
        let difficulty: '普通' | '英雄' | '挑战' = '普通';
        let name = '未知';

        if (raidName.includes('10人')) {
          playerCount = 10;
        } else if (raidName.includes('25人')) {
          playerCount = 25;
        }

        if (raidName.includes('英雄')) {
          difficulty = '英雄';
        } else if (raidName.includes('挑战')) {
          difficulty = '挑战';
        } else if (raidName.includes('普通')) {
          difficulty = '普通';
        }

        // 提取副本名称：移除人数和难度前缀
        name = raidName
          .replace(/10人|25人/g, '')
          .replace(/普通|英雄|挑战/g, '')
          .trim() || '未知';

        // 尝试从raids配置中找到匹配的副本获取boss信息
        const matchedRaid = raids.find(r =>
          r.name === name && r.difficulty === difficulty && r.playerCount === playerCount
        );

        const raidForEdit = matchedRaid || {
          name,
          difficulty,
          playerCount,
          isActive: true
        };

        return (
          <AddRecordModal
            isOpen={true}
            onClose={() => setEditingRecord(null)}
            onSubmit={handleUpdateRecord}
            initialData={editingRecord}
            role={{
              id: editingRecord.roleId,
              name: editingRecord.roleName || '未知角色',
              server: editingRecord.server?.split(' ')[1] || '未知服务器',
              region: editingRecord.server?.split(' ')[0] || '未知大区',
              sect: '未知',
              accountId: editingRecord.accountId,
              accountName: ''
            }}
            raid={raidForEdit}
          />
        );
      })()}

      {/* Edit Baizhan Record Modal */}
      {editingBaizhanRecord && (
        <AddBaizhanRecordModal
          isOpen={true}
          onClose={() => setEditingBaizhanRecord(null)}
          onSubmit={(updatedRecord) => {
            setBaizhanRecords(prev => {
              const exists = prev.some(r => r.id === updatedRecord.id);
              if (exists) {
                return prev.map(r => r.id === updatedRecord.id ? updatedRecord : r);
              }
              return [updatedRecord, ...prev];
            });
            setEditingBaizhanRecord(null);
          }}
          accounts={accounts}
          initialData={editingBaizhanRecord}
        />
      )}

      {/* Toast Container */}
      <ToastContainer />
    </div>
  );
}

const NavButton = ({ active, onClick, icon, label }: { active: boolean, onClick: () => void, icon: React.ReactNode, label: string }) => (
  <button
    onClick={onClick}
    className={`flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition-colors ${active ? 'bg-base text-primary' : 'text-muted hover:text-main hover:bg-base/50'
      }`}
  >
    {icon} {label}
  </button>
);

const MobileNavButton = ({ active, onClick, icon, label }: { active: boolean, onClick: () => void, icon: React.ReactNode, label: string }) => (
  <button
    onClick={onClick}
    className={`flex flex-col items-center justify-center w-full h-full gap-1 transition-colors ${active ? 'text-primary' : 'text-muted'
      }`}
  >
    {icon}
    <span className="text-[10px] font-medium">{label}</span>
  </button>
);

// 构建角色可见性映射
function buildVisibilityMap(
  roleId: string,
  allVisibility: RoleInstanceVisibility[],
  types: InstanceType[]
): Record<string, boolean> {
  const map: Record<string, boolean> = {};

  // 默认全部可见
  types.forEach(type => {
    map[type.type] = true;
  });

  // 覆盖有记录的配置
  allVisibility
    .filter(v => v.roleId === roleId)
    .forEach(v => {
      map[v.instanceType] = v.visible;
    });

  return map;
}

export default App;