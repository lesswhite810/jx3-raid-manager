import type { AppConfig, TrialPlaceRecord } from '../types';

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

const MOCK_TRIAL_RECORDS: TrialPlaceRecord[] = [
  {
    "id": "mock-trial-1",
    "accountId": "mock-acc-1",
    "roleId": "mock-role-1",
    "roleName": "欧神再临",
    "server": "梦江南",
    "date": Date.now(),
    "layer": 90,
    "bosses": ["九色鹿", "吸血鬼", "推石头"],
    "flippedIndex": 0,
    "source": "auto",
    "equipmentUnreviewed": true,
    card1: '', card2: '', card3: '', card4: '', card5: '',
    draws: [1, 2].map(sequence => ({
      id: `mock-draw-${sequence}`,
      sourceKey: `mock-receipt-${sequence}`,
      sequence,
      receiptTime: Date.now() - (2 - sequence) * 20000,
      flippedIndex: sequence === 1 ? 1 : 0,
      rewardItems: [{ name: '银叶子·试炼之地', quantity: sequence === 1 ? 31 : 33 }],
      ...(sequence === 2 ? { staminaCost: 1000, staminaCosts: [125, 875] } : {}),
      card1: '', card2: '', card3: '', card4: '', card5: '',
      equipmentUnreviewed: true,
    })),
    "type": "trial"
  }
];

const mockCache = new Map<string, [string, string]>();

const MOCK_APP_CONFIG: AppConfig = {
  gameDirectory: 'mock://browser-preview',
  setupCompleted: true,
  lastScanMingyiAt: null,
  autoScanEnabled: false,
  trialScreenshotRecognitionEnabled: false,
  autoRefreshEquipScore: true,
  closeBehavior: 'ask',
};

export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  console.log(`[Mock Invoke] Call: ${cmd}`, args);
  
  return new Promise((resolve, reject) => {
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
              seasonId: 5,
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
              seasonId: 5,
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
        case 'db_add_trial_record': {
          // 与后端一致：`record` 是 JSON 字符串，整体写入试炼记录列表。
          const raw = args?.record;
          if (typeof raw !== 'string') {
            reject(new Error('新增试炼记录的参数格式无效'));
            return;
          }
          let parsed: TrialPlaceRecord;
          try {
            parsed = JSON.parse(raw);
          } catch {
            reject(new Error('新增试炼记录的数据无法解析'));
            return;
          }
          if (!parsed?.id) {
            reject(new Error('新增试炼记录缺少 id'));
            return;
          }
          if (MOCK_TRIAL_RECORDS.some(row => row.id === parsed.id)) {
            reject(new Error('试炼记录 id 已存在'));
            return;
          }
          MOCK_TRIAL_RECORDS.push(parsed);
          break;
        }
        case 'db_delete_trial_record': {
          const id = args?.id;
          if (typeof id !== 'string' || !id) {
            reject(new Error('删除试炼记录缺少 id'));
            return;
          }
          const index = MOCK_TRIAL_RECORDS.findIndex(row => row.id === id);
          if (index < 0) {
            reject(new Error('未找到要删除的试炼记录'));
            return;
          }
          MOCK_TRIAL_RECORDS.splice(index, 1);
          break;
        }
        case 'db_update_trial_draw_flipped_index': {
          const record = MOCK_TRIAL_RECORDS.find(row => row.id === args?.recordId);
          const draw = record?.draws?.find(row => row.id === args?.drawId);
          const slot = args?.flippedIndex ?? 0;
          if (!draw || typeof slot !== 'number' || !Number.isInteger(slot) || slot < 0 || slot > 5) {
            reject(new Error('未找到翻牌明细或牌位无效'));
            return;
          }
          draw.flippedIndex = slot;
          break;
        }
        case 'db_update_trial_draw_equipment': {
          const drawId = args?.drawId;
          const cardIndex = args?.cardIndex;
          const equipmentId = args?.equipmentId;
          const draw = MOCK_TRIAL_RECORDS.flatMap(record => record.draws ?? [])
            .find(row => row.id === drawId);
          if (!draw || typeof cardIndex !== 'number' || !Number.isInteger(cardIndex) || cardIndex < 1 || cardIndex > 5
            || (equipmentId !== null && typeof equipmentId !== 'string')) {
            reject(new Error('未找到翻牌明细、牌位或装备无效'));
            return;
          }
          const key = `card${cardIndex}` as 'card1' | 'card2' | 'card3' | 'card4' | 'card5';
          draw[key] = equipmentId?.trim() || '';
          break;
        }
        case 'db_set_trial_draw_equipment_reviewed': {
          const draw = MOCK_TRIAL_RECORDS.flatMap(record => record.draws ?? [])
            .find(row => row.id === args?.drawId);
          if (!draw || typeof args?.reviewed !== 'boolean') {
            reject(new Error('未找到翻牌明细或核对状态无效'));
            return;
          }
          const expectedCards = args.expectedCards;
          if (args.reviewed && (args.expectedFlippedIndex !== draw.flippedIndex
            || !Array.isArray(expectedCards) || expectedCards.length !== 5
            || [draw.card1, draw.card2, draw.card3, draw.card4, draw.card5].some((card, index) => card !== expectedCards[index]))) {
            reject(new Error('牌面已更新，请刷新后重新确认'));
            return;
          }
          draw.equipmentUnreviewed = !args.reviewed;
          break;
        }
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
          result = { ...MOCK_APP_CONFIG };
          break;
        case 'complete_setup':
        case 'set_game_directory':
        case 'reset_setup':
        case 'set_auto_scan_enabled':
        case 'set_auto_refresh_equip_score_enabled':
          result = null;
          break;
        case 'set_trial_screenshot_recognition_enabled':
          if (typeof args?.enabled === 'boolean') {
            MOCK_APP_CONFIG.trialScreenshotRecognitionEnabled = args.enabled;
          }
          result = null;
          break;
        case 'set_close_behavior':
          if (args?.behavior === 'ask' || args?.behavior === 'minimize' || args?.behavior === 'exit') {
            MOCK_APP_CONFIG.closeBehavior = args.behavior;
          }
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
        // 游戏目录校验 / 扫描（浏览器环境返回成功空结果，避免预览时抛错）
        case 'validate_game_directory':
          result = {
            isValid: true,
            message: '游戏目录验证成功（浏览器预览环境）',
            details: {
              checkedPath: typeof args?.gameDirectory === 'string' ? args.gameDirectory : 'mock://browser-preview',
              expectedStructure: 'SeasunGame\\Game\\JX3\\bin\\zhcn_hd',
              currentStructure: 'mock://browser-preview',
              missingDirectories: [],
              checkTime: new Date().toISOString(),
            },
          };
          break;
        case 'scan_game_directory':
          result = { success: true, accounts: [], gkpFiles: [] };
          break;
        case 'auto_parse_game_directory':
          result = { success: true, newAccounts: 0, updatedAccounts: 0, newRoles: 0, updatedRoles: 0, error: null };
          break;
        case 'analyze_roles':
          result = { success: true, newAccounts: 0, updatedAccounts: 0, newRoles: 0, updatedRoles: 0, error: null };
          break;
        case 'scan_jx3_clients':
          result = { success: true, clients: [] };
          break;
        // 角色预览 / 选择性导入（浏览器环境返回空列表，UI 显示空状态）
        case 'preview_importable_roles':
          result = { success: true, accounts: [], error: null };
          break;
        case 'import_selected_roles':
          result = { success: true, newAccounts: 0, updatedAccounts: 0, newRoles: 0, updatedRoles: 0, error: null };
          break;
        // 活跃检测 / 运行时状态（浏览器环境视为 JX3 未运行）
        case 'detect_accounts_active':
          result = {
            jx3Running: false,
            jx3StartTime: null,
            jx3EndTime: null,
            jx3ProcessCount: 0,
            multiInstanceDetected: false,
            multiInstanceHint: null,
            roles: [],
            scanDurationMs: 0,
            processSessions: [],
          };
          break;
        case 'get_jx3_runtime_status':
          result = {
            isRunning: false,
            matchedProcess: null,
            allProcesses: [],
            matchedProcessCount: 0,
            multiInstanceDetected: false,
            multiInstanceHint: null,
          };
          break;
        // 副本掉落扫描 / 记录确认（浏览器环境不产生任何扫描结果）
        case 'scan_raid_drops':
          result = 0;
          break;
        case 'scan_all_active_raid_drops':
        case 'scan_raids_in_range':
          result = [];
          break;
        case 'clear_jcl_cache':
          result = 0;
          break;
        case 'confirm_record':
        case 'reject_record':
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
