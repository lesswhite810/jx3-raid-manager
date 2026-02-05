import React, { useState, useEffect, useRef } from 'react';
import { LayoutDashboard, Users, Download, Shield, Settings, Sun, Moon } from 'lucide-react';
import { useTheme } from './contexts/ThemeContext';
import { Dashboard } from './components/Dashboard';
import { IncomeDetail } from './components/IncomeDetail';
import { AccountManager } from './components/AccountManager';
import { RaidManager } from './components/RaidManager';
import { RaidDetail } from './components/RaidDetail';
import { CrystalDetail } from './components/CrystalDetail';


import { ToastContainer } from './components/ToastContainer';
import { ConfigManager } from './components/ConfigManager';
import { LoadingSpinner } from './components/LoadingSpinner';
import { AddRecordModal } from './components/AddRecordModal';
import { Account, RaidRecord, Raid, Config, TrialPlaceRecord } from './types';
import {
  DEFAULT_CONFIG,
  loadConfigFromStorage,
  saveConfigToStorage,
  validateConfig,
  getConfigSummary
} from './utils/configUtils';
import { mergeRaids, getRaidKey, saveRaidCache } from './utils/raidUtils';
import { sortAccounts } from './utils/accountUtils';
import { db } from './services/db';
import { checkLocalStorageData, forceMigrate } from './services/migration';
import { syncEquipment } from './services/jx3BoxApi';

function App() {
  const [activeTab, setActiveTab] = useState<'dashboard' | 'accounts' | 'raidManager' | 'config'>('dashboard');
  const [showIncomeDetail, setShowIncomeDetail] = useState(false);
  const [showCrystalDetail, setShowCrystalDetail] = useState(false);
  const [selectedRaid, setSelectedRaid] = useState<Raid | null>(null);
  const [deferredPrompt, setDeferredPrompt] = useState<any>(null);
  const [isTransitioning, setIsTransitioning] = useState(false);
  const [contentKey, setContentKey] = useState(0);
  const [isInitialized, setIsInitialized] = useState(false);
  const previousTabRef = useRef<string>('dashboard');

  const { theme, toggleTheme } = useTheme();

  const [accounts, setAccounts] = useState<Account[]>([]);
  const [records, setRecords] = useState<RaidRecord[]>([]);
  const [trialRecords, setTrialRecords] = useState<TrialPlaceRecord[]>([]);
  const [raids, setRaids] = useState<Raid[]>([]);
  const [config, setConfig] = useState<Config>(DEFAULT_CONFIG);
  const [editingRecord, setEditingRecord] = useState<RaidRecord | null>(null);

  useEffect(() => {
    const initApp = async () => {
      try {
        console.log('正在初始化数据库...');
        await db.init();
        console.log('数据库初始化完成');

        console.log('正在检查本地存储数据...');
        const localData = await checkLocalStorageData();
        console.log('本地数据状态:', localData);

        if (localData.totalItems > 0) {
          console.log(`发现本地存储数据，开始迁移...`);
          console.log(`  账号: ${localData.accountsCount}`);
          console.log(`  记录: ${localData.recordsCount}`);
          console.log(`  副本: ${localData.raidsCount}`);

          const result = await forceMigrate();

          if (result.success) {
            console.log('✓ 迁移成功！');
            if (result.migrated.accounts > 0 || result.migrated.records > 0 || result.migrated.raids > 0) {
              console.log(`  新增账号: +${result.migrated.accounts}`);
              console.log(`  新增记录: +${result.migrated.records}`);
              console.log(`  新增副本: +${result.migrated.raids}`);
            } else {
              console.log('  数据已是最新的，无需新增');
            }
          } else {
            console.error('✗ 迁移失败:', result.message);
          }
        } else {
          console.log('无需迁移，本地无数据');
        }

        console.log('\n正在加载数据库数据...');
        const [loadedAccounts, loadedRecords, loadedRaids, loadedConfig, loadedTrialRecords] = await Promise.all([
          db.getAccounts(),
          db.getRecords(),
          db.getRaids(),
          db.getConfig(),
          db.getTrialRecords()
        ]);

        console.log(`加载完成: 账号 ${loadedAccounts.length}, 记录 ${loadedRecords.length}, 副本 ${loadedRaids.length}, 试炼 ${loadedTrialRecords.length}`);

        const parsedAccounts = loadedAccounts;
        const parsedRecords = loadedRecords;
        const parsedRaids = loadedRaids;

        if (parsedAccounts.length > 0) {
          const sortedAccounts = sortAccounts(parsedAccounts);
          setAccounts(sortedAccounts);
          console.log(`设置账号: ${sortedAccounts.length} 个`);
        } else {
          console.log('没有账号数据');
        }

        const mergedRaids = parsedRaids.length > 0 ? mergeRaids(parsedRaids) : mergeRaids([]);
        saveRaidCache(mergedRaids);
        setRaids(mergedRaids);

        if (loadedConfig) {
          const configData = typeof loadedConfig === 'string' ? JSON.parse(loadedConfig) : loadedConfig;
          setConfig(configData);
        } else {
          const localConfig = loadConfigFromStorage();
          if (localConfig) {
            setConfig(localConfig);
            await db.saveConfig(localConfig);
          }
        }

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
    setSelectedRaid(null);
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

  const handleDeleteRecord = (recordId: string) => {
    setRecords(prev => prev.filter(r => r.id !== recordId));
  };

  const handleUpdateRecord = (updatedRecord: Partial<RaidRecord>) => {
    if (!editingRecord) return;

    setRecords(prev => prev.map(r => {
      if (r.id === editingRecord.id) {
        return { ...r, ...updatedRecord } as RaidRecord;
      }
      return r;
    }));
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
                  accounts={accounts}
                  onBack={() => setShowIncomeDetail(false)}
                  onDeleteRecord={handleDeleteRecord}
                  onEditRecord={setEditingRecord}
                />
              ) : showCrystalDetail ? (
                <CrystalDetail
                  key={`crystalDetail-${contentKey}`}
                  records={records}
                  accounts={accounts}
                  onBack={() => setShowCrystalDetail(false)}
                />
              ) : (
                <Dashboard
                  key={`dashboard-${contentKey}`}
                  records={records}
                  accounts={accounts}
                  onShowIncomeDetail={() => setShowIncomeDetail(true)}
                  onShowCrystalDetail={() => setShowCrystalDetail(true)}
                />
              )
            )}
            {activeTab === 'accounts' && (
              <AccountManager key={`accounts-${contentKey}`} accounts={accounts} setAccounts={setAccounts} config={config} />
            )}
            {activeTab === 'raidManager' && (
              selectedRaid ? (
                <RaidDetail
                  key={`raidDetail-${getRaidKey(selectedRaid)}-${contentKey}`}
                  raid={selectedRaid}
                  accounts={accounts}
                  records={records}
                  onBack={() => setSelectedRaid(null)}
                  setRecords={setRecords}
                  onEditRecord={setEditingRecord}
                />
              ) : (

                // @ts-ignore - passing extra props that aren't defined in interface yet but will be
                <RaidManager
                  key={`raidManager-${contentKey}`}
                  raids={raids}
                  setRaids={setRaids}
                  onRaidClick={setSelectedRaid}
                  trialRecords={trialRecords}
                  setTrialRecords={setTrialRecords}
                  accounts={accounts}
                />
              )
            )
            }
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
      {editingRecord && (
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
            sect: '未知', // Editing doesn't need sect strictly for logic
            accountId: editingRecord.accountId,
            accountName: '' // Not critical for edit display
          }}
          raid={{
            name: '未知',
            difficulty: 'NORMAL',
            playerCount: 25,
            isActive: true
          }}
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

export default App;