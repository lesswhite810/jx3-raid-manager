import { describe, expect, test } from 'vitest';
import { buildTauriCliArgs } from './tauriCliUtils.mjs';

describe('buildTauriCliArgs', () => {
  test('本地执行 tauri build 时默认追加 --no-bundle', () => {
    expect(buildTauriCliArgs(['build'], {})).toEqual(['build', '--no-bundle']);
  });

  test('GitHub Release 构建保留 bundling', () => {
    expect(buildTauriCliArgs(['build'], { GITHUB_ACTIONS: 'true' })).toEqual(['build']);
  });

  test('显式指定 bundles 时不覆盖用户参数', () => {
    expect(buildTauriCliArgs(['build', '--bundles', 'nsis'], {})).toEqual(['build', '--bundles', 'nsis']);
  });

  test('非 build 命令不追加参数', () => {
    expect(buildTauriCliArgs(['dev'], {})).toEqual(['dev']);
  });
});
