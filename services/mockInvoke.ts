// 简单的 mock 拦截器，当由于没有 Tauri 后端导致 API 调用失败时使用。
// 这里硬编码一些初始的基础测试数据。

const MOCK_ACCOUNTS = [
  {
    "id": "mock-acc-1",
    "accountName": "测试账号 (Mock)",
    "type": "OWN",
    "roles": [
      {
        "id": "mock-role-1",
        "name": "欧神再临",
        "server": "梦江南",
        "region": "电信五区",
        "sect": "万花",
        "equipmentScore": 320000
      },
      {
        "id": "mock-role-2",
        "name": "无敌黑手",
        "server": "梦江南",
        "region": "电信五区",
        "sect": "纯阳",
        "equipmentScore": 280000
      }
    ]
  }
];

const MOCK_RECORDS = [
  {
    "id": "mock-record-1",
    "accountId": "mock-acc-1",
    "roleId": "mock-role-1",
    "raidName": "冷龙峰25人普通",
    "date": Date.now(),
    "goldIncome": 54000,
    "hasXuanjing": true,
    "hasMount": true,
    "type": "raid"
  }
];

const MOCK_TRIAL_RECORDS = [
  {
    "id": "mock-trial-1",
    "accountId": "mock-acc-1",
    "roleId": "mock-role-1",
    "roleName": "欧神再临",
    "server": "梦江南",
    "date": Date.now(),
    "layer": 90,
    "bosses": ["九色鹿", "吸血鬼", "推石头"],
    "flippedIndex": 1,
    "type": "trial"
  }
];

const mockCache = new Map<string, [string, string]>();

export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  console.log(`[Mock Invoke] Call: ${cmd}`, args);
  
  return new Promise((resolve) => {
    setTimeout(() => {
      let result: any = null;
      
      switch (cmd) {
        case 'db_init':
          result = null;
          break;
        case 'db_get_version_info':
          result = { schemaVersion: 1, currentVersion: 1, isLatest: true, localStorageMigrated: true };
          break;
        case 'db_is_local_storage_migrated':
          result = true;
          break;
        case 'db_get_accounts_with_roles':
        case 'db_get_accounts_structured':
          result = JSON.stringify(MOCK_ACCOUNTS);
          break;
        case 'db_get_all_roles':
          result = JSON.stringify(MOCK_ACCOUNTS.flatMap(a => a.roles));
          break;
        case 'db_get_records':
          result = MOCK_RECORDS.map(r => JSON.stringify(r));
          break;
        case 'db_get_pending_records':
          result = [
            JSON.stringify({
              id: 'mock-pending-1',
              accountId: 'mock-acc-1',
              roleId: 'mock-role-1',
              roleName: '欧神再临',
              server: '梦江南',
              raidName: '25人普通冷龙峰',
              date: Date.now(),
              goldIncome: 54000,
              goldExpense: 8000,
              source: 'auto',
              status: 'pending',
              bossNames: ['骨犀', '武云飞'],
              drops: ['流漓腰带', '维峰丹', '玛瑙'],
              isScrapsBoss: false,
              scrapsValue: 0,
              scrapsItems: [
                { name: '维峰丹', count: 2, unitPrice: null, category: 'material', priceSource: 'manual' },
                { name: '玛瑙', count: 1, unitPrice: null, category: 'material', priceSource: 'manual' },
                { name: '流漓腰带', count: 1, unitPrice: 25000, category: 'equipment', priceSource: 'npc' },
              ],
            }),
          ];
          break;
        case 'db_get_raids':
          result = [
            JSON.stringify({
              name: '冷龙峰',
              difficulty: '普通',
              playerCount: 25,
              version: '丝路风雨',
              isActive: true,
              static: true,
              bosses: [
                { id: 'leng-long-feng-b1', name: '骨犀', order: 1 },
                { id: 'leng-long-feng-b2', name: '武云飞', order: 2 },
                { id: 'leng-long-feng-b3', name: '月泉淮', order: 3 },
              ],
            }),
            JSON.stringify({
              name: '冷龙峰',
              difficulty: '英雄',
              playerCount: 25,
              version: '丝路风雨',
              isActive: true,
              static: true,
              bosses: [
                { id: 'leng-long-feng-h-b1', name: '骨犀', order: 1 },
                { id: 'leng-long-feng-h-b2', name: '武云飞', order: 2 },
                { id: 'leng-long-feng-h-b3', name: '月泉淮', order: 3 },
              ],
            }),
          ];
          break;
        case 'db_get_raid_versions':
          result = ['丝路风雨'];
          break;
        case 'db_get_current_raid_version_info':
          result = {
            majorVersion: '丝路风雨',
            level: 130,
            versionName: '山海源流',
            startDate: '2025.10.30'
          };
          break;
        case 'db_get_config':
          result = JSON.stringify({ theme: 'system', dbPath: 'mock_path' });
          break;
        case 'db_get_equipments':
          result = '[]';
          break;
        case 'db_get_trial_records':
          result = JSON.stringify(MOCK_TRIAL_RECORDS);
          break;
        case 'db_get_baizhan_records':
          result = '[]';
          break;
        case 'db_get_favorite_raids':
          result = [];
          break;
        case 'db_get_cache':
          result = typeof args?.key === 'string' ? mockCache.get(args.key) ?? null : null;
          break;
        case 'db_save_cache':
          if (typeof args?.key === 'string' && typeof args?.value === 'string') {
            mockCache.set(args.key, [args.value, new Date().toISOString()]);
          }
          result = null;
          break;
        case 'db_get_instance_types':
          result = JSON.stringify([]);
          break;
        case 'db_get_all_role_visibility':
          result = JSON.stringify([]);
          break;
        case 'db_get_raid_role_visibility':
          // 返回空数组表示所有角色默认可见
          result = JSON.stringify([]);
          break;
        case 'db_save_role_visibility':
        case 'db_save_raid_role_visibility':
          result = null;
          break;
        case 'get_app_config':
          result = {
            gameDirectory: 'mock://browser-preview',
            setupCompleted: true,
            lastScanMingyiAt: null,
            autoScanEnabled: false,
            autoRefreshEquipScore: true,
          };
          break;
        case 'complete_setup':
        case 'set_game_directory':
        case 'reset_setup':
        case 'set_auto_scan_enabled':
        case 'set_auto_refresh_equip_score_enabled':
          result = null;
          break;
        // 跨角色设置同步（浏览器环境仅返回空数据，避免 UI 崩溃）
        case 'list_syncable_roles':
          result = [];
          break;
        case 'list_char_sync_backups':
          result = [];
          break;
        case 'sync_character_settings':
          result = {
            success: true,
            copiedFiles: [],
            skippedFiles: [],
            backupDir: null,
            error: null,
          };
          break;
        case 'rollback_char_sync':
          result = null;
          break;
        // 回滚标记 CRUD（V18+）
        case 'list_char_sync_rollback_marks':
          result = {};
          break;
        case 'set_char_sync_rollback_mark':
        case 'clear_char_sync_rollback_marks':
          result = null;
          break;
        default:
          console.warn(`[Mock Invoke] Unhandled command: ${cmd}`);
          result = null;
      }
      
      resolve(result as T);
    }, 100); // 模拟一点网络延迟
  });
}
