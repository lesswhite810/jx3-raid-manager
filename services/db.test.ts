import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// db.ts 的失败路径测试。
//
// 关注点不是「正常返回」，而是 IPC 抛错时每个方法的降级行为：
// 读操作应返回空值而不是把异常漏给 UI，写操作应 rethrow 让调用方能提示用户。
//
// 这里 mock 掉 services/invoke，直接控制 invokeCommand 的成功/失败，
// 从而在纯 Node 环境下覆盖 db.ts 的错误分支（无需 Tauri 宿主）。

const invokeCommandMock = vi.fn();

vi.mock('./invoke', () => ({
  invokeCommand: (...args: unknown[]) => invokeCommandMock(...args),
  isTauriRuntime: () => true,
}));

// 每个测试都要拿到干净的 DatabaseService 实例：
// db.ts 导出的是单例，但 initialized 状态会跨测试泄漏，故按需重置模块。
type DbModule = typeof import('./db');

const loadDb = async (): Promise<DbModule> => {
  vi.resetModules();
  return import('./db');
};

const makeError = (message: string): Error => new Error(message);

describe('DatabaseService 失败路径', () => {
  beforeEach(() => {
    invokeCommandMock.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('读操作：失败降级为空值', () => {
    it('getAccounts 在 IPC 失败时返回空数组且不抛出', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_get_accounts_with_roles') return Promise.reject(makeError('IPC 断开'));
        return Promise.resolve(undefined);
      });

      await expect(db.getAccounts()).resolves.toEqual([]);
    });

    it('getRaidVersions 在 IPC 失败时返回空数组', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_get_raid_versions') return Promise.reject(makeError('IPC 断开'));
        return Promise.resolve(undefined);
      });

      await expect(db.getRaidVersions()).resolves.toEqual([]);
    });

    it('getCurrentSeason 在 IPC 失败时返回 null', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_get_current_season') return Promise.reject(makeError('IPC 断开'));
        return Promise.resolve(undefined);
      });

      await expect(db.getCurrentSeason()).resolves.toBeNull();
    });

    it('getFavoriteRaids 在 IPC 失败时返回空数组', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_get_favorite_raids') return Promise.reject(makeError('IPC 断开'));
        return Promise.resolve(undefined);
      });

      await expect(db.getFavoriteRaids()).resolves.toEqual([]);
    });
  });

  describe('写操作：失败必须向上抛出', () => {
    it('saveAccounts 在 IPC 失败时 rethrow，供调用方提示用户', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_save_accounts') return Promise.reject(makeError('磁盘只读'));
        return Promise.resolve(undefined);
      });

      await expect(db.saveAccounts([])).rejects.toThrow('磁盘只读');
    });

    it('saveRaids 在 IPC 失败时 rethrow', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_save_raids') return Promise.reject(makeError('磁盘只读'));
        return Promise.resolve(undefined);
      });

      await expect(db.saveRaids([])).rejects.toThrow('磁盘只读');
    });

    it('saveRecords 在 IPC 失败时 rethrow', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_save_records') return Promise.reject(makeError('写库失败'));
        return Promise.resolve(undefined);
      });

      await expect(db.saveRecords([])).rejects.toThrow('写库失败');
    });

    it('deleteDirectory 在 IPC 失败时 rethrow，避免静默忽略删除失败', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_delete_directory') return Promise.reject(makeError('路径不在 userdata 内'));
        return Promise.resolve(undefined);
      });

      await expect(db.deleteDirectory('C:/x', 'account')).rejects.toThrow('路径不在 userdata 内');
    });

    it('deleteRaidStructured 在 IPC 失败时 rethrow', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_delete_raid_structured') return Promise.reject(makeError('预制副本不能删除'));
        return Promise.resolve(undefined);
      });

      await expect(db.deleteRaidStructured('25人普通洛阳之战')).rejects.toThrow('预制副本不能删除');
    });
  });

  describe('保存护栏参数', () => {
    it('saveAccounts 默认传 allowDelete=false', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation(() => Promise.resolve(undefined));

      await db.saveAccounts([]);

      const saveCall = invokeCommandMock.mock.calls.find(([cmd]) => cmd === 'db_save_accounts');
      expect(saveCall).toBeDefined();
      expect(saveCall?.[1]).toMatchObject({ allowDelete: false });
    });

    it('saveAccounts 显式传 true 时透传 allowDelete=true', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation(() => Promise.resolve(undefined));

      await db.saveAccounts([], true);

      const saveCall = invokeCommandMock.mock.calls.find(([cmd]) => cmd === 'db_save_accounts');
      expect(saveCall?.[1]).toMatchObject({ allowDelete: true });
    });

    it('saveRaids 默认传 allowDelete=false', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation(() => Promise.resolve(undefined));

      await db.saveRaids([]);

      const saveCall = invokeCommandMock.mock.calls.find(([cmd]) => cmd === 'db_save_raids');
      expect(saveCall).toBeDefined();
      expect(saveCall?.[1]).toMatchObject({ allowDelete: false });
    });
  });

  describe('试炼记录：语义反转与并发冲突必须显式暴露', () => {
    // getTrialRecords 曾是「失败返回 []」，会让「读取失败」伪装成「没有记录」，
    // 用户会以为自己的试炼数据丢了。这里锁定新语义：失败必须抛出。
    it('getTrialRecords 在 IPC 失败时必须抛出，不能降级为空数组', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_get_trial_records') return Promise.reject(makeError('查询试炼记录失败'));
        return Promise.resolve(undefined);
      });

      await expect(db.getTrialRecords()).rejects.toThrow('查询试炼记录失败');
    });

    it('getTrialRecords 在返回非法 JSON 时必须抛出，不能吞掉解析错误', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_get_trial_records') return Promise.resolve('{ 这不是合法 JSON');
        return Promise.resolve(undefined);
      });

      await expect(db.getTrialRecords()).rejects.toThrow();
    });

    it('getTrialRecords 成功时返回解析后的数组', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_get_trial_records') {
          return Promise.resolve(JSON.stringify([{ id: 'trial-1', type: 'trial' }]));
        }
        return Promise.resolve(undefined);
      });

      await expect(db.getTrialRecords()).resolves.toEqual([{ id: 'trial-1', type: 'trial' }]);
    });

    it('deleteTrialRecord 在 IPC 失败时 rethrow', async () => {
      const { db } = await loadDb();
      invokeCommandMock.mockImplementation((cmd: string) => {
        if (cmd === 'db_init') return Promise.resolve(undefined);
        if (cmd === 'db_delete_trial_record') return Promise.reject(makeError('记录不存在'));
        return Promise.resolve(undefined);
      });

      await expect(db.deleteTrialRecord('trial-1')).rejects.toThrow('记录不存在');
    });
  });
});
