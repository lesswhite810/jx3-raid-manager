import { describe, expect, test } from 'vitest';
import {
  buildTauriCliArgs,
  buildTauriCliEnv,
  describeLocalBuildOptimization,
} from './tauriCliUtils.mjs';

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

describe('buildTauriCliEnv', () => {
  test('本地 tauri build 启用快速 release 配置', () => {
    const env = buildTauriCliEnv(['build'], {}, {
      targetTriple: 'x86_64-pc-windows-msvc',
      rustLldPath: 'C:/Rust/rust-lld.exe',
    });

    expect(env.CARGO_PROFILE_RELEASE_INCREMENTAL).toBe('true');
    expect(env.CARGO_PROFILE_RELEASE_CODEGEN_UNITS).toBe('256');
    expect(env.CARGO_PROFILE_RELEASE_OPT_LEVEL).toBe('2');
    expect(env.CARGO_PROFILE_RELEASE_DEBUG).toBe('0');
    expect(env.CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER).toBe('C:/Rust/rust-lld.exe');
  });

  test('GitHub Release 构建不注入本地加速配置', () => {
    const sourceEnv = { GITHUB_ACTIONS: 'true' };
    expect(buildTauriCliEnv(['build'], sourceEnv)).toBe(sourceEnv);
  });

  test('显式关闭本地快速构建时保留原环境', () => {
    const sourceEnv = { JX3_TAURI_FULL_LOCAL_BUILD: '1' };
    expect(buildTauriCliEnv(['build'], sourceEnv)).toBe(sourceEnv);
  });

  test('debug 构建不注入 release 加速配置', () => {
    const sourceEnv = {};
    expect(buildTauriCliEnv(['build', '--debug'], sourceEnv)).toBe(sourceEnv);
  });
});

describe('describeLocalBuildOptimization', () => {
  test('本地构建会输出优化说明', () => {
    expect(
      describeLocalBuildOptimization(['build'], {}, {
        targetTriple: 'x86_64-pc-windows-msvc',
        rustLldPath: 'C:/Rust/rust-lld.exe',
      })
    ).toContain('rust-lld');
  });
});
