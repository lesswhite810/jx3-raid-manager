import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, test } from 'vitest';

const REQUIRED_SIMPLIFIED_CHINESE_KEYS = [
  'addOrReinstall',
  'alreadyInstalled',
  'alreadyInstalledLong',
  'appRunning',
  'appRunningOkKill',
  'chooseMaintenanceOption',
  'choowHowToInstall',
  'createDesktop',
  'dontUninstall',
  'dontUninstallDowngrade',
  'failedToKillApp',
  'installingWebview2',
  'newerVersionInstalled',
  'older',
  'olderOrUnknownVersionInstalled',
  'silentDowngrades',
  'unableToUninstall',
  'uninstallApp',
  'uninstallBeforeInstalling',
  'unknown',
  'webview2AbortError',
  'webview2DownloadError',
  'webview2DownloadSuccess',
  'webview2Downloading',
  'webview2InstallError',
  'webview2InstallSuccess',
  'deleteAppData',
];

function readSimplifiedChineseLanguageFile() {
  const languageFile = path.resolve('src-tauri', 'nsis', 'SimpChinese.nsh');
  return fs.readFileSync(languageFile, 'utf8');
}

describe('SimpChinese NSIS language file', () => {
  test('covers all installer custom language keys', () => {
    const content = readSimplifiedChineseLanguageFile();

    for (const key of REQUIRED_SIMPLIFIED_CHINESE_KEYS) {
      expect(content).toContain(`LangString ${key} \${LANG_SIMPCHINESE} `);
    }
  });

  test('uses direct Chinese copy for app-running prompts', () => {
    const content = readSimplifiedChineseLanguageFile();

    expect(content).toContain(
      'LangString appRunning ${LANG_SIMPCHINESE} "${PRODUCTNAME} 正在运行，请先关闭后再重试。"',
    );
    expect(content).toContain(
      'LangString appRunningOkKill ${LANG_SIMPCHINESE} "${PRODUCTNAME} 正在运行。点击确定可结束它并继续卸载。"',
    );
    expect(content).toContain(
      'LangString failedToKillApp ${LANG_SIMPCHINESE} "无法结束 ${PRODUCTNAME}。请先关闭后再重试。"',
    );
    expect(content).not.toContain('{{product_name}}');
  });
});
