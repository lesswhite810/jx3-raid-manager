import { describe, expect, it } from 'vitest';
import { getCurrentVersionLevelCap, shouldShowRoleByLevel } from './roleLevelFilter';
import type { GameVersion } from '../types';

describe('shouldShowRoleByLevel', () => {
  it('开关关闭时始终显示（即使等级不匹配）', () => {
    expect(shouldShowRoleByLevel({ level: 100 }, 130, false)).toBe(true);
    expect(shouldShowRoleByLevel({ level: 130 }, 130, false)).toBe(true);
    expect(shouldShowRoleByLevel({}, 130, false)).toBe(true);
  });

  it('当前版本未配置等级上限（null/undefined）时不过滤', () => {
    expect(shouldShowRoleByLevel({ level: 100 }, null, true)).toBe(true);
    expect(shouldShowRoleByLevel({ level: 100 }, undefined, true)).toBe(true);
    expect(shouldShowRoleByLevel({ level: 130 }, null, true)).toBe(true);
  });

  it('角色等级未知（null/undefined/缺失）时默认显示（老数据）', () => {
    expect(shouldShowRoleByLevel({ level: null }, 130, true)).toBe(true);
    expect(shouldShowRoleByLevel({ level: undefined }, 130, true)).toBe(true);
    expect(shouldShowRoleByLevel({}, 130, true)).toBe(true);
  });

  it('开关开启且等级匹配上限时显示', () => {
    expect(shouldShowRoleByLevel({ level: 130 }, 130, true)).toBe(true);
    expect(shouldShowRoleByLevel({ level: 50 }, 50, true)).toBe(true);
  });

  it('开关开启且等级不匹配上限时隐藏', () => {
    expect(shouldShowRoleByLevel({ level: 120 }, 130, true)).toBe(false);
    expect(shouldShowRoleByLevel({ level: 130 }, 50, true)).toBe(false);
    expect(shouldShowRoleByLevel({ level: 0 }, 130, true)).toBe(false);
  });
});

describe('getCurrentVersionLevelCap', () => {
  const makeVersion = (name: string, sortOrder: number, levelCap?: number | null): GameVersion => ({
    name,
    sortOrder,
    ...(levelCap !== undefined ? { levelCap } : {}),
  });

  it('空列表返回 null（不过滤）', () => {
    expect(getCurrentVersionLevelCap([])).toBeNull();
  });

  it('取 sortOrder 最大的版本的 levelCap', () => {
    const versions = [
      makeVersion('苍生铸世', 1, 50),
      makeVersion('丝路风雨', 2, 130),
    ];
    expect(getCurrentVersionLevelCap(versions)).toBe(130);
  });

  it('sortOrder 最大但未配置 levelCap 时返回 null', () => {
    const versions = [
      makeVersion('苍生铸世', 1, 50),
      makeVersion('未知版本', 2, null),
    ];
    expect(getCurrentVersionLevelCap(versions)).toBeNull();
  });

  it('最新版本缺失 levelCap 字段时返回 null', () => {
    const versions = [
      makeVersion('苍生铸世', 1, 50),
      makeVersion('未知版本', 2),
    ];
    expect(getCurrentVersionLevelCap(versions)).toBeNull();
  });

  it('列表全为 null levelCap 时返回 null', () => {
    const versions = [
      makeVersion('A', 1, null),
      makeVersion('B', 2, null),
    ];
    expect(getCurrentVersionLevelCap(versions)).toBeNull();
  });
});
