import { beforeEach, describe, expect, it, vi } from 'vitest';

const { getEquipmentsMock, fetchMock } = vi.hoisted(() => ({
  getEquipmentsMock: vi.fn(),
  fetchMock: vi.fn(),
}));

vi.mock('./db', () => ({
  db: {
    getEquipments: getEquipmentsMock,
  },
}));

vi.mock('@tauri-apps/plugin-http', () => ({
  fetch: fetchMock,
}));

import { getEquip, getTrialEquipments } from './jx3BoxApi';

const oldCachedEquipment = { ID: 1, Name: '旧赛季无修', Level: 25500 };
const currentSeasonEquipment = { ID: 2, Name: '当前赛季无修', Level: 900 };
interface RemoteResponseOptions {
  total?: number;
  pages?: number;
  page?: number;
  per?: number;
  ok?: boolean;
}

const remoteResponse = (list: unknown[], {
  total = list.length,
  pages = 1,
  page = 1,
  per = 100,
  ok = true,
}: RemoteResponseOptions = {}) => ({
  ok,
  status: ok ? 200 : 500,
  json: async () => ({ list, total, pages, page, per }),
});

const createItems = <T>(count: number, createItem: (index: number) => T): T[] =>
  Array.from({ length: count }, (_, index) => createItem(index));

describe('getEquip 本地缓存与远端优先语义', () => {
  beforeEach(() => {
    getEquipmentsMock.mockReset();
    fetchMock.mockReset();
  });

  it('默认调用保持读取本地缓存的既有行为', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);

    await expect(getEquip('无修')).resolves.toEqual([oldCachedEquipment]);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('preferRemote 抓取三类装备，不会混回高于 900 的旧数据库条目', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockImplementation((url: string) => {
      const request = new URL(url);
      const equipType = request.pathname.split('/').pop();
      const page = request.searchParams.get('page');
      const responses: Record<string, ReturnType<typeof remoteResponse>> = {
        'armor-1': remoteResponse([{ ...currentSeasonEquipment, ID: 2, SubType: 3 }]),
        'trinket-1': remoteResponse([{ ...currentSeasonEquipment, ID: 4, SubType: 4 }]),
        'weapon-1': remoteResponse([{ ...currentSeasonEquipment, ID: 5, SubType: 1, DetailType: 6 }]),
      };
      return Promise.resolve(responses[`${equipType}-${page}`]);
    });

    await expect(getEquip('无修', { preferRemote: true, minLevel: 900 })).resolves.toEqual([
      { ...currentSeasonEquipment, ID: 2, SubType: 3, TypeLabel: '帽子' },
      { ...currentSeasonEquipment, ID: 4, SubType: 4, TypeLabel: '项链' },
      { ...currentSeasonEquipment, ID: 5, SubType: 1, DetailType: 6, TypeLabel: '暗器' },
    ]);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
    expect(fetchMock).toHaveBeenCalledTimes(3);
    for (const [url] of fetchMock.mock.calls) {
      const request = new URL(url as string);
      expect(request.searchParams.get('per')).toBe('100');
      expect(request.searchParams.get('min_level')).toBe('900');
      expect(request.searchParams.has('max_level')).toBe(false);
    }
  });

  it('preferRemote 的任一分页失败都不返回部分装备，也不回退旧数据库缓存', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockImplementation((url: string) => {
      const request = new URL(url);
      const equipType = request.pathname.split('/').pop();
      const page = request.searchParams.get('page');
      if (equipType === 'armor' && page === '1') {
        return Promise.resolve(remoteResponse(
          createItems(100, index => ({ ...currentSeasonEquipment, ID: index + 1000 })),
          { total: 101, pages: 2 }
        ));
      }
      if (equipType === 'armor' && page === '2') {
        return Promise.resolve(remoteResponse([], { total: 101, pages: 2, page: 2, ok: false }));
      }
      return Promise.resolve(remoteResponse([currentSeasonEquipment]));
    });

    await expect(getEquip('无修', { preferRemote: true })).resolves.toEqual([]);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
  });

  it('preferRemote 遇到超过十页的类别时明确失败，不静默截断', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockResolvedValue(remoteResponse(
      createItems(100, index => ({ ...currentSeasonEquipment, ID: index + 2000 })),
      { total: 1100, pages: 11 }
    ));

    await expect(getEquip('无修', { preferRemote: true })).resolves.toEqual([]);
    expect(fetchMock).toHaveBeenCalledTimes(3);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
  });

  it('苍生铸世只请求无观项链和暗器，并过滤同名制作装备与普通武器', async () => {
    const necklacePage = createItems(100, index => ({
      ID: 51407 + index,
      Name: '无观项链·内·荒',
      Level: 900,
      SubType: 4,
    }));
    const pouchPage = createItems(100, index => ({
      ID: 47942 + index,
      Name: '无观囊·内·荒',
      Level: 1280,
      SubType: 1,
      DetailType: 6,
    }));
    necklacePage[1] = { ID: 51408, Name: '无观戒指·内·荒', Level: 900, SubType: 5 };
    necklacePage[2] = { ID: 51409, Name: '无观项链·内·荒', Level: 899, SubType: 4 };
    pouchPage[1] = { ID: 47943, Name: '无观剑·内·荒', Level: 1280, SubType: 1, DetailType: 0 };
    pouchPage[2] = { ID: 47944, Name: '无观囊·内·荒', Level: 1281, SubType: 1, DetailType: 6 };
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockImplementation((url: string) => {
      const request = new URL(url);
      const equipType = request.pathname.split('/').pop();
      const page = request.searchParams.get('page');
      if (equipType === 'trinket' && page === '1') {
        return Promise.resolve(remoteResponse(necklacePage, { total: 101, pages: 2 }));
      }
      if (equipType === 'trinket' && page === '2') {
        return Promise.resolve(remoteResponse([
          { ID: 51607, Name: '无观项链·内·荒', Level: 940, SubType: 4 },
        ], { total: 101, pages: 2, page: 2 }));
      }
      if (equipType === 'weapon' && page === '1') {
        return Promise.resolve(remoteResponse(pouchPage, { total: 101, pages: 2 }));
      }
      return Promise.resolve(remoteResponse([
        { ID: 48042, Name: '无观囊·内·荒', Level: 1195, SubType: 1, DetailType: 6 },
      ], { total: 101, pages: 2, page: 2 }));
    });

    const items = await getTrialEquipments(
      { name: '苍生铸世' },
      { minLevel: 900, maxLevel: 1280 }
    );
    expect(items).toHaveLength(198);
    expect(items).toContainEqual(expect.objectContaining({ ID: 51407, TypeLabel: '项链' }));
    expect(items).toContainEqual(expect.objectContaining({ ID: 51607, TypeLabel: '项链' }));
    expect(items).toContainEqual(expect.objectContaining({ ID: 47942, TypeLabel: '暗器' }));
    expect(items).toContainEqual(expect.objectContaining({ ID: 48042, TypeLabel: '暗器' }));
    expect(items.map(item => item.ID)).not.toContain(51408);
    expect(items.map(item => item.ID)).not.toContain(47943);
    expect(items.map(item => item.ID)).not.toContain(51409);
    expect(items.map(item => item.ID)).not.toContain(47944);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
    expect(fetchMock).toHaveBeenCalledTimes(4);
    for (const [url] of fetchMock.mock.calls) {
      const request = new URL(url as string);
      expect(request.searchParams.get('name')).toBe('无观');
      expect(request.searchParams.get('min_level')).toBe('900');
      expect(request.searchParams.get('max_level')).toBe('1280');
      expect(['trinket', 'weapon']).toContain(request.pathname.split('/').pop());
    }
  });

  it('旧赛季未配置品级范围时保持读取本地无修缓存', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);

    await expect(getTrialEquipments({ name: '暗影千机' })).resolves.toEqual([oldCachedEquipment]);
    expect(getEquipmentsMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('新赛季任一分页失败时不返回部分无观装备，也不回退旧缓存', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockImplementation((url: string) => {
      const request = new URL(url);
      const equipType = request.pathname.split('/').pop();
      const page = request.searchParams.get('page');
      if (equipType === 'trinket' && page === '1') {
        return Promise.resolve(remoteResponse(
          createItems(100, index => ({ ID: 51407 + index, Name: '无观项链·内·荒', Level: 900, SubType: 4 })),
          { total: 101, pages: 2 }
        ));
      }
      if (equipType === 'trinket' && page === '2') {
        return Promise.resolve(remoteResponse([], { total: 101, pages: 2, page: 2, ok: false }));
      }
      return Promise.resolve(remoteResponse(
        createItems(100, index => ({ ID: 47942 + index, Name: '无观囊·内·荒', Level: 900, SubType: 1, DetailType: 6 }))
      ));
    });

    await expect(getTrialEquipments(
      { name: '苍生铸世' },
      { minLevel: 900, maxLevel: 1280 }
    )).resolves.toEqual([]);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
  });

  it.each([
    { label: '空页', total: 101, lastPage: [] as unknown[] },
    {
      label: '短页',
      total: 102,
      lastPage: [{ ID: 51607, Name: '无观项链·内·荒', Level: 940, SubType: 4 }],
    },
  ])('HTTP 200 但试炼装备分页出现$label时不返回部分数据', async ({ total, lastPage }) => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockImplementation((url: string) => {
      const request = new URL(url);
      const equipType = request.pathname.split('/').pop();
      const page = request.searchParams.get('page');
      if (equipType === 'trinket' && page === '1') {
        return Promise.resolve(remoteResponse(
          createItems(100, index => ({ ID: 51407 + index, Name: '无观项链·内·荒', Level: 900, SubType: 4 })),
          { total, pages: 2 }
        ));
      }
      if (equipType === 'trinket' && page === '2') {
        return Promise.resolve(remoteResponse(lastPage, { total, pages: 2, page: 2 }));
      }
      return Promise.resolve(remoteResponse(
        createItems(100, index => ({ ID: 47942 + index, Name: '无观囊·内·荒', Level: 900, SubType: 1, DetailType: 6 }))
      ));
    });

    await expect(getTrialEquipments({ name: '苍生铸世' }, { minLevel: 900 })).resolves.toEqual([]);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
  });

  it('HTTP 200 但页码不匹配时不返回部分试炼装备', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockImplementation((url: string) => {
      const equipType = new URL(url).pathname.split('/').pop();
      const items = equipType === 'trinket'
        ? createItems(100, index => ({ ID: 51407 + index, Name: '无观项链·内·荒', Level: 900, SubType: 4 }))
        : createItems(100, index => ({ ID: 47942 + index, Name: '无观囊·内·荒', Level: 900, SubType: 1, DetailType: 6 }));
      return Promise.resolve(remoteResponse(items, { page: equipType === 'trinket' ? 2 : 1 }));
    });

    await expect(getTrialEquipments({ name: '苍生铸世' }, { minLevel: 900 })).resolves.toEqual([]);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
  });

  it('限制部位的任一类别筛选为空时不展示另一类别的部分数据', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockImplementation((url: string) => {
      const equipType = new URL(url).pathname.split('/').pop();
      const items = equipType === 'trinket'
        ? createItems(100, index => ({ ID: 51407 + index, Name: '无观戒指·内·荒', Level: 900, SubType: 5 }))
        : createItems(100, index => ({ ID: 47942 + index, Name: '无观囊·内·荒', Level: 900, SubType: 1, DetailType: 6 }));
      return Promise.resolve(remoteResponse(items));
    });

    await expect(getTrialEquipments({ name: '苍生铸世' }, { minLevel: 900 })).resolves.toEqual([]);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
  });

  it('限制部位的任一类别全部不在品级范围时不展示另一类别的部分数据', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockImplementation((url: string) => {
      const equipType = new URL(url).pathname.split('/').pop();
      const items = equipType === 'trinket'
        ? createItems(100, index => ({ ID: 51407 + index, Name: '无观项链·内·荒', Level: 899, SubType: 4 }))
        : createItems(100, index => ({ ID: 47942 + index, Name: '无观囊·内·荒', Level: 900, SubType: 1, DetailType: 6 }));
      return Promise.resolve(remoteResponse(items));
    });

    await expect(getTrialEquipments({ name: '苍生铸世' }, { minLevel: 900 })).resolves.toEqual([]);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
  });
});
