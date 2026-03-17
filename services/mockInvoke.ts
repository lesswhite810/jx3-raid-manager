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
        case 'db_get_raids':
          result = [];
          break;
        case 'db_get_raid_versions':
          result = [];
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
        case 'db_get_instance_types':
          result = JSON.stringify([]);
          break;
        case 'db_get_all_role_visibility':
          result = JSON.stringify([]);
          break;
        default:
          console.warn(`[Mock Invoke] Unhandled command: ${cmd}`);
          result = null;
      }
      
      resolve(result as T);
    }, 100); // 模拟一点网络延迟
  });
}
