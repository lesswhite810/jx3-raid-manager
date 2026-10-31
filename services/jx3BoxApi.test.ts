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

import { getEquip } from './jx3BoxApi';

const oldCachedEquipment = { ID: 1, Name: '旧赛季无修', Level: 25500 };
const currentSeasonEquipment = { ID: 2, Name: '当前赛季无修', Level: 900 };
const remoteResponse = (list: unknown[], pages = 1, ok = true) => ({
  ok,
  status: ok ? 200 : 500,
  json: async () => ({ list, pages }),
});

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

  it('preferRemote 抓取三类装备的完整分页，不会混回高于 900 的旧数据库条目', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockImplementation((url: string) => {
      const request = new URL(url);
      const equipType = request.pathname.split('/').pop();
      const page = request.searchParams.get('page');
      const responses: Record<string, ReturnType<typeof remoteResponse>> = {
        'armor-1': remoteResponse([{ ...currentSeasonEquipment, ID: 2, SubType: 3 }], 2),
        'armor-2': remoteResponse([{ ...currentSeasonEquipment, ID: 3, SubType: 2 }]),
        'trinket-1': remoteResponse([{ ...currentSeasonEquipment, ID: 4, SubType: 4 }]),
        'weapon-1': remoteResponse([{ ...currentSeasonEquipment, ID: 5, SubType: 1 }], 2),
        'weapon-2': remoteResponse([{ ...currentSeasonEquipment, ID: 6, DetailType: 6 }]),
      };
      return Promise.resolve(responses[`${equipType}-${page}`]);
    });

    await expect(getEquip('无修', { preferRemote: true, minLevel: 900 })).resolves.toEqual([
      { ...currentSeasonEquipment, ID: 2, SubType: 3, TypeLabel: '帽子' },
      { ...currentSeasonEquipment, ID: 3, SubType: 2, TypeLabel: '上衣' },
      { ...currentSeasonEquipment, ID: 4, SubType: 4, TypeLabel: '项链' },
      { ...currentSeasonEquipment, ID: 5, SubType: 1, TypeLabel: '武器' },
      { ...currentSeasonEquipment, ID: 6, DetailType: 6, TypeLabel: '暗器' },
    ]);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
    expect(fetchMock).toHaveBeenCalledTimes(5);
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
        return Promise.resolve(remoteResponse([currentSeasonEquipment], 2));
      }
      if (equipType === 'armor' && page === '2') {
        return Promise.resolve(remoteResponse([], 1, false));
      }
      return Promise.resolve(remoteResponse([currentSeasonEquipment]));
    });

    await expect(getEquip('无修', { preferRemote: true })).resolves.toEqual([]);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
  });

  it('preferRemote 遇到超过十页的类别时明确失败，不静默截断', async () => {
    getEquipmentsMock.mockResolvedValue([oldCachedEquipment]);
    fetchMock.mockResolvedValue(remoteResponse([currentSeasonEquipment], 11));

    await expect(getEquip('无修', { preferRemote: true })).resolves.toEqual([]);
    expect(fetchMock).toHaveBeenCalledTimes(3);
    expect(getEquipmentsMock).not.toHaveBeenCalled();
  });
});
