import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, test } from 'vitest';

function readFile(relativePath) {
  return fs.readFileSync(path.resolve(relativePath), 'utf8');
}

describe('runtime mode detection', () => {
  test('database layer also treats portable release exe names as portable builds', () => {
    const dbSource = readFile('src-tauri/src/db.rs');
    const updaterSource = readFile('src-tauri/src/updater.rs');
    const runtimeModeSource = readFile('src-tauri/src/runtime_mode.rs');

    expect(runtimeModeSource).toContain('value.contains("_v")');
    expect(dbSource).toContain('runtime_mode::detect_current_runtime_mode()');
    expect(updaterSource).toContain('runtime_mode::detect_runtime_mode_for_executable(&current_exe)');
  });
});
