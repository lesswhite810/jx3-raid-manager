import fs from 'node:fs';
import { describe, expect, test } from 'vitest';

describe('Tauri dependency versions', () => {
  test('resolved Rust and JavaScript core versions share a major and minor version', () => {
    const cargoLock = fs.readFileSync('src-tauri/Cargo.lock', 'utf8');
    const packageLock = JSON.parse(fs.readFileSync('package-lock.json', 'utf8'));
    const tauriPackages = cargoLock.split('[[package]]').filter((section) =>
      /^name = "tauri"$/m.test(section),
    );

    expect(tauriPackages).toHaveLength(1);
    const rustVersion = tauriPackages[0].match(/^version = "(\d+\.\d+\.\d+)"$/m)?.[1];
    const apiVersion = packageLock.packages?.['node_modules/@tauri-apps/api']?.version;
    expect(rustVersion).toMatch(/^\d+\.\d+\.\d+$/);
    expect(apiVersion).toMatch(/^\d+\.\d+\.\d+$/);
    expect(apiVersion.split('.').slice(0, 2)).toEqual(rustVersion.split('.').slice(0, 2));
  });
});
