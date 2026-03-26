import { describe, expect, it } from 'vitest';
import {
  buildGitHubReleaseUrl,
  detectUpdateChannel,
  formatUpdatePubDate,
  normalizeReleaseNotes,
  pickInstallerAsset,
  type GitHubReleaseAsset
} from './updaterUtils';

describe('detectUpdateChannel', () => {
  it('treats install directory with uninstall executable as installer channel', () => {
    expect(
      detectUpdateChannel(String.raw`C:\Users\lessw\AppData\Local\Programs\JX3RaidManager\JX3RaidManager.exe`, true)
    ).toEqual({
      isPortable: false,
      channel: 'installer',
      willInstallInPlace: true
    });
  });

  it('falls back to portable channel when uninstall executable is missing', () => {
    expect(
      detectUpdateChannel(String.raw`E:\Tools\JX3RaidManager_v2.1.8.exe`, false)
    ).toEqual({
      isPortable: true,
      channel: 'portable',
      willInstallInPlace: false
    });
  });
});

describe('normalizeReleaseNotes', () => {
  it('keeps existing markdown content when notes are present', () => {
    expect(normalizeReleaseNotes('## 新增需求\n\n- 支持自动更新')).toBe('## 新增需求\n\n- 支持自动更新');
  });

  it('returns fallback text when release notes are empty', () => {
    expect(normalizeReleaseNotes('   ')).toBe('本次版本未提供更新说明。');
  });
});

describe('formatUpdatePubDate', () => {
  it('formats legacy OffsetDateTime strings that JS cannot parse directly', () => {
    const legacyTimeString = '2026-03-27 00:10:20.123456789 +00:00:00';
    const isoTimeString = '2026-03-27T00:10:20.123456789Z';

    expect(formatUpdatePubDate(legacyTimeString)).toBe(formatUpdatePubDate(isoTimeString));
    expect(formatUpdatePubDate(legacyTimeString)).not.toBeNull();
  });

  it('returns null for empty or invalid release timestamps', () => {
    expect(formatUpdatePubDate('')).toBeNull();
    expect(formatUpdatePubDate('not-a-date')).toBeNull();
    expect(formatUpdatePubDate(undefined)).toBeNull();
  });
});

describe('pickInstallerAsset', () => {
  it('prefers the NSIS setup package from GitHub release assets', () => {
    const assets: GitHubReleaseAsset[] = [
      {
        name: 'JX3RaidManager_v2.1.9.exe',
        browser_download_url: 'https://example.com/portable.exe'
      },
      {
        name: 'JX3RaidManager_2.1.9_x64-setup.exe',
        browser_download_url: 'https://example.com/setup.exe'
      }
    ];

    expect(pickInstallerAsset(assets)?.browser_download_url).toBe('https://example.com/setup.exe');
  });

  it('returns null when release does not provide an installer asset', () => {
    const assets: GitHubReleaseAsset[] = [
      {
        name: 'JX3RaidManager_v2.1.9.exe',
        browser_download_url: 'https://example.com/portable.exe'
      }
    ];

    expect(pickInstallerAsset(assets)).toBeNull();
  });
});

describe('buildGitHubReleaseUrl', () => {
  it('builds the tag page URL for manual download fallback', () => {
    expect(buildGitHubReleaseUrl('lesswhite810/jx3-raid-manager', 'v2.1.9')).toBe(
      'https://github.com/lesswhite810/jx3-raid-manager/releases/tag/v2.1.9'
    );
  });
});
