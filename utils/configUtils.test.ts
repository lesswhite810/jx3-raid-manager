import { describe, expect, it } from 'vitest';
import { resolveGameRuntimeDirectory } from './configUtils';

describe('resolveGameRuntimeDirectory', () => {
  it('resolves SeasunGame install root to zhcn_hd runtime directory', () => {
    expect(resolveGameRuntimeDirectory(String.raw`E:\Game\SeasunGame`)).toBe(
      String.raw`E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd`
    );
  });

  it('keeps legacy zhcn_hd directory unchanged', () => {
    expect(resolveGameRuntimeDirectory(String.raw`E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd`)).toBe(
      String.raw`E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd`
    );
  });

  it('collapses subdirectories under zhcn_hd back to the runtime directory', () => {
    expect(
      resolveGameRuntimeDirectory(
        String.raw`E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\userdata\test-account`
      )
    ).toBe(String.raw`E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd`);
  });

  it('returns empty string for blank input', () => {
    expect(resolveGameRuntimeDirectory('')).toBe('');
  });
});
