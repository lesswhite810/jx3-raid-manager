import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, test } from 'vitest';

const HOOKS_FILE = path.resolve('src-tauri', 'nsis', 'hooks.nsh');

// 取出 NSIS_HOOK_PREUNINSTALL 宏内真正会执行的指令（去掉注释行），
// 说明文字里出现的路径示例不应影响断言。
function readPreuninstallMacroBody() {
  const source = fs.readFileSync(HOOKS_FILE, 'utf8');
  const lines = source.split(/\r?\n/);

  const start = lines.findIndex((line) =>
    line.trim().startsWith('!macro NSIS_HOOK_PREUNINSTALL'),
  );
  const end = lines.findIndex(
    (line, index) => index > start && line.trim().startsWith('!macroend'),
  );

  expect(start).toBeGreaterThanOrEqual(0);
  expect(end).toBeGreaterThan(start);

  return lines
    .slice(start + 1, end)
    .filter((line) => !line.trim().startsWith(';'))
    .join('\n');
}

describe('NSIS uninstall hook', () => {
  test('never deletes the portable default data directory unconditionally', () => {
    const body = readPreuninstallMacroBody();

    // 卸载安装版时曾无条件 Push 便携版默认目录并清理，会误删
    // ~/.jx3-raid-manager 里的便携版数据库与日志。
    expect(body).not.toContain('Push "$PROFILE\\.jx3-raid-manager"');
  });

  test('still cleans the installer data directories', () => {
    const body = readPreuninstallMacroBody();

    expect(body).toContain('Push "$INSTDIR"');
    expect(body).toContain('Call un.DeleteManagedDataFilesInDir');
    // 读取安装版专属状态文件后，按记录的实际目录清理。
    expect(body).toContain('data-dir-installer.ini');
  });

  test('keeps the two guards that protect the portable data directory', () => {
    const body = readPreuninstallMacroBody();

    const guardForEffective = '${AndIf} $0 != "$PROFILE\\.jx3-raid-manager"';
    const guardForResolved = '${AndIf} $1 != "$PROFILE\\.jx3-raid-manager"';
    expect(body).toContain(guardForEffective);
    expect(body).toContain(guardForResolved);

    // 非注释代码里对该目录的引用应恰好是两处守卫，不应有删除调用。
    const occurrences = body.split('$PROFILE\\.jx3-raid-manager').length - 1;
    expect(occurrences).toBe(2);
  });
});
